//! Checked integer layout and strict result decoding; no hardware access.

use super::{FormulaCheck, FormulaLimits, FormulaStatistics, FormulaVerdict, ResidualReason};
use crate::{GpuError, GpuErrorKind};
use zetesis_algebra::region::Region;
use zetesis_ferraris::{Interpretation, Theory};

pub(super) const PARAM_BYTES: u64 = 64;
pub(super) const HEADER_WORDS: usize = 6;
pub(super) const MAGIC: u32 = 0x4652_5031;

// Decode constructs one Region at a time. Its mask payload stays with the
// output row, while the same allowance for construction headers serves every row.
const ROOT_STAGING_BYTES: u64 = (size_of::<Region>() + size_of::<Vec<u64>>()) as u64;

fn root_mask_bytes(atoms: u32) -> u64 {
    u64::from(atoms.div_ceil(u64::BITS)) * size_of::<[u64; 2]>() as u64
}

pub(super) fn capacity(detail: &str) -> GpuError {
    GpuError::new(GpuErrorKind::Capacity, detail)
}
pub(super) fn address(value: usize) -> Result<u32, GpuError> {
    u32::try_from(value).map_err(|_| capacity("formula dimension exceeds u32"))
}
fn mul(a: u64, b: u64) -> Result<u64, GpuError> {
    a.checked_mul(b)
        .ok_or_else(|| capacity("formula byte product overflow"))
}
fn sum(values: &[u64]) -> Result<u64, GpuError> {
    values.iter().try_fold(0u64, |total, next| {
        total
            .checked_add(*next)
            .ok_or_else(|| capacity("formula byte sum overflow"))
    })
}
fn buffer(bytes: u64, device: &wgpu::Limits) -> Result<(), GpuError> {
    if bytes > device.max_buffer_size || bytes > device.max_storage_buffer_binding_size {
        return Err(capacity(
            "formula storage buffer exceeds granted device limits",
        ));
    }
    Ok(())
}
pub(super) fn vector<T>(length: usize) -> Result<Vec<T>, GpuError> {
    let mut out = Vec::new();
    out.try_reserve_exact(length)
        .map_err(|_| GpuError::new(GpuErrorKind::Allocation, "formula host reservation failed"))?;
    Ok(out)
}

pub(super) struct Shape {
    pub(super) theory: Theory,
    pub(super) atoms: u32,
    pub(super) nodes: u32,
    pub(super) roots: u32,
    pub(super) edges: u32,
    pub(super) wide_words: u32,
    pub(super) rows: crate::formula_graph::Rows,
    pub(super) temporary_bytes: u64,
    pub(super) variables: u32,
    pub(super) words: u32,
    pub(super) node_bytes: u64,
    pub(super) setup_work: u32,
    pub(super) sweep_work: u32,
}
impl Shape {
    #[cfg(test)]
    pub(super) fn new(theory: &Theory, device: &wgpu::Limits) -> Result<Self, GpuError> {
        Self::with_control(
            theory,
            device,
            u64::MAX,
            &zetesis_algebra::Cancellation::default(),
        )
    }

    pub(super) fn with_control(
        theory: &Theory,
        device: &wgpu::Limits,
        max_staging_bytes: u64,
        cancellation: &zetesis_algebra::Cancellation,
    ) -> Result<Self, GpuError> {
        let atoms = address(theory.atom_count())?;
        let roots = address(theory.roots().len())?;
        let graph = crate::formula_graph::Shape::with_control(
            theory,
            device,
            max_staging_bytes,
            cancellation,
        )?;
        let nodes = graph.nodes;
        let auxiliary = graph.auxiliary;
        let variables = atoms
            .checked_add(auxiliary)
            .ok_or_else(|| capacity("formula variable addresses overflow"))?;
        // Strided GPU loops add at most the widest kernel stride after their last
        // valid address.
        if [atoms, nodes, roots, variables]
            .into_iter()
            .any(|n| !crate::kernel::fits_strided_loop(n))
        {
            return Err(capacity("formula loop increment overflows u32"));
        }
        let words = atoms.div_ceil(32);
        let node_bytes = graph.bytes;
        let mut shape = Self {
            theory: theory.clone(),
            atoms,
            nodes,
            roots,
            edges: graph.edges,
            wide_words: graph.wide_words,
            temporary_bytes: graph.rows.bytes()?,
            rows: graph.rows,
            variables,
            words,
            node_bytes,
            setup_work: 0,
            sweep_work: 0,
        };
        // Reserve the single final projection before admitting any sweep. A
        // residual classifies each atom and writes two masks. Terminal records
        // share the bound and initialize both payloads to zero.
        let projection = words
            .checked_mul(2)
            .and_then(|n| n.checked_add(atoms))
            .ok_or_else(|| capacity("formula residual projection work overflows"))?;
        shape.setup_work = nodes
            .checked_mul(2)
            .and_then(|n| n.checked_add(shape.edges))
            .and_then(|n| n.checked_add(atoms))
            .and_then(|n| n.checked_add(roots))
            .and_then(|n| n.checked_add(graph.numeric_work))
            .and_then(|n| n.checked_add(projection))
            .ok_or_else(|| capacity("formula setup work overflows"))?;
        shape.sweep_work = nodes
            .checked_mul(9)
            .and_then(|n| shape.edges.checked_mul(2).and_then(|e| n.checked_add(e)))
            .and_then(|n| n.checked_add(atoms))
            .and_then(|n| n.checked_add(65))
            .and_then(|n| n.checked_add(graph.numeric_work))
            .and_then(|n| n.checked_add(graph.threshold_guard_work))
            .ok_or_else(|| capacity("formula sweep work overflows"))?;
        Ok(shape)
    }
}

/// Checked wire layout. Zero levels deliberately selects the serial evaluator;
/// positive levels address a complete dependency schedule after the root prefix.
pub(super) struct Schedule {
    pub(super) levels: u32,
    pub(super) root_bytes: u64,
    pub(super) bytes: u64,
    pub(super) setup_work: u32,
    pub(super) incidence: Option<crate::formula_graph::incidence::Layout>,
}
impl Schedule {
    pub(super) fn new(shape: &Shape, levels: u32, device: &wgpu::Limits) -> Result<Self, GpuError> {
        let root_words =
            crate::formula_graph::dependency::root_words(shape.nodes, shape.roots, levels)?;
        let root_bytes = mul(u64::from(root_words), 4)?;
        buffer(root_bytes, device)?;
        Ok(Self {
            levels,
            root_bytes,
            bytes: sum(&[shape.node_bytes, root_bytes])?,
            setup_work: shape
                .setup_work
                .checked_add(levels)
                .ok_or_else(|| capacity("formula level work overflows"))?,
            incidence: None,
        })
    }

    pub(super) fn with_incidence(
        mut self,
        incidence: crate::formula_graph::incidence::Layout,
        device: &wgpu::Limits,
    ) -> Result<Self, GpuError> {
        if u64::from(incidence.offset) * 4 != self.root_bytes {
            return Err(capacity(
                "formula incidence offset differs from root prefix",
            ));
        }
        let extra = mul(u64::from(incidence.words), 4)?;
        self.root_bytes = sum(&[self.root_bytes, extra])?;
        buffer(self.root_bytes, device)?;
        self.bytes = sum(&[self.bytes, extra])?;
        self.setup_work = self
            .setup_work
            .checked_add(incidence.setup_work)
            .ok_or_else(|| capacity("formula dirty setup work overflows"))?;
        self.incidence = Some(incidence);
        Ok(self)
    }

    pub(super) fn propagation(&self) -> crate::GpuPropagation {
        match &self.incidence {
            Some(incidence) if incidence.group_map != 0 => crate::GpuPropagation::IncrementalGroups,
            Some(_) => crate::GpuPropagation::ChangedDependencies,
            None => crate::GpuPropagation::Dense,
        }
    }
}

/// Final shape and schedule only; temporary depth words never belong to Graph.
pub(super) struct Graph {
    pub(super) shape: Shape,
    pub(super) schedule: Schedule,
    /// The workgroup size this graph dispatches: the fixed size, or the size
    /// its packed schedule fits.
    pub(super) workgroup_size: crate::GpuWorkgroupSize,
}

pub(super) struct Plan {
    pub(super) atoms: u32,
    pub(super) max_batch_bytes: u64,
    pub(super) worlds: u32,
    pub(super) seeds: u64,
    pub(super) masks: u64,
    pub(super) domains: u64,
    pub(super) results: u64,
    pub(super) transport: u64,
    pub(super) accounted: u64,
    pub(super) setup: u32,
    pub(super) sweep: u32,
    pub(super) max_rounds: u32,
    pub(super) max_work: u32,
    pub(super) epoch: u32,
}
impl Plan {
    pub(super) fn new(
        graph: &Graph,
        worlds: usize,
        limits: FormulaLimits,
        device: &wgpu::Limits,
        fresh: bool,
        epoch: u32,
    ) -> Result<Self, GpuError> {
        Self::layout(
            &graph.shape,
            &graph.schedule,
            worlds,
            limits,
            device,
            fresh,
            epoch,
        )
    }
    pub(super) fn layout(
        shape: &Shape,
        schedule: &Schedule,
        worlds: usize,
        limits: FormulaLimits,
        device: &wgpu::Limits,
        fresh: bool,
        epoch: u32,
    ) -> Result<Self, GpuError> {
        let count = address(worlds)?;
        crate::runtime::positive_timeout(limits.timeout)?;
        if worlds > limits.max_candidates || count > device.max_compute_workgroups_per_dimension {
            return Err(capacity("formula batch exceeds candidate/dispatch ceiling"));
        }
        if limits.max_work_per_candidate < schedule.setup_work || epoch == 0 {
            return Err(capacity("mandatory formula setup or epoch exceeds limits"));
        }
        let array = |length: u32| -> Result<u64, GpuError> {
            let elements = u64::from(length) * u64::from(count);
            if elements > u64::from(u32::MAX) {
                return Err(capacity("formula world offset exceeds u32"));
            }
            mul(elements.max(1), 4)
        };
        let seeds = array(shape.words)?;
        let masks = array(shape.nodes)?;
        let domain_words = schedule.incidence.as_ref().map_or(
            Ok(shape.variables),
            crate::formula_graph::incidence::Layout::domain_words,
        )?;
        let domains = array(domain_words)?;
        let result_stride = sum(&[HEADER_WORDS as u64, mul(2, u64::from(shape.words))?])?;
        let result_words = mul(u64::from(count), result_stride)?;
        if result_words > u64::from(u32::MAX) {
            return Err(capacity("formula result offset exceeds u32"));
        }
        let results = mul(result_words.max(1), 4)?;
        for bytes in [seeds, masks, domains, results] {
            buffer(bytes, device)?;
        }
        if device.max_uniform_buffer_binding_size < PARAM_BYTES {
            return Err(capacity("formula uniform buffer exceeds device limit"));
        }
        let transport = sum(&[PARAM_BYTES, seeds, masks, domains, results, results])?;
        let host_results = mul(u64::from(count), std::mem::size_of::<FormulaCheck>() as u64)?;
        // All worlds may return a residual simultaneously. The output Vec
        // carries each Region header; each such Region owns two boxed u64 masks.
        let root_payload = mul(u64::from(count), root_mask_bytes(shape.atoms))?;
        let root_staging = if count == 0 { 0 } else { ROOT_STAGING_BYTES };
        let accounted = sum(&[
            schedule.bytes,
            transport,
            PARAM_BYTES,
            seeds,
            host_results,
            root_payload,
            root_staging,
            if fresh { schedule.bytes } else { 0 },
            if fresh { shape.temporary_bytes } else { 0 },
        ])?;
        if worlds != 0 && accounted > limits.max_batch_bytes {
            return Err(capacity("formula batch exceeds authored byte ceiling"));
        }
        Ok(Self {
            worlds: count,
            atoms: shape.atoms,
            max_batch_bytes: limits.max_batch_bytes,
            seeds,
            masks,
            domains,
            results,
            transport,
            accounted,
            setup: schedule.setup_work,
            // Bound one gate visit per node plus the dependency-level walk.
            // Active-word execution retains this allowance even when it skips
            // gates and levels; it is not a count of instructions or barriers.
            sweep: shape
                .sweep_work
                .checked_add(schedule.levels)
                .and_then(|n| {
                    n.checked_add(schedule.incidence.as_ref().map_or(0, |i| i.sweep_work))
                })
                .ok_or_else(|| capacity("formula sweep level work overflows"))?,
            max_rounds: limits.max_rounds,
            max_work: limits.max_work_per_candidate,
            epoch,
        })
    }
    // Replace the requested cold upload staging with its actual retained element
    // capacities. No work or device effect uses a vector before this admission.
    pub(super) fn staging(
        &mut self,
        requested: u64,
        nodes: u64,
        roots: u64,
        ceiling: u64,
    ) -> Result<(), GpuError> {
        self.accounted = self
            .accounted
            .checked_sub(requested)
            .and_then(|n| n.checked_add(nodes))
            .and_then(|n| n.checked_add(roots))
            .ok_or_else(|| capacity("formula staging capacity sum overflows"))?;
        if self.accounted > ceiling {
            return Err(capacity(
                "formula staging capacity exceeds authored byte ceiling",
            ));
        }
        Ok(())
    }
    pub(super) fn params(&self, graph: &Graph) -> [u32; 16] {
        [
            graph.shape.atoms,
            graph.shape.nodes,
            graph.shape.roots,
            graph.shape.variables,
            graph.shape.words,
            graph.schedule.incidence.as_ref().map_or(0, |i| i.offset),
            self.max_rounds,
            self.max_work,
            self.setup,
            self.sweep,
            self.epoch,
            graph.schedule.levels,
            graph.schedule.incidence.as_ref().map_or(0, |i| i.group_map),
            0,
            0,
            0,
        ]
    }
    pub(super) fn pack(
        &self,
        graph: &Graph,
        candidates: &[Interpretation],
        staging: &mut crate::candidates::Staging,
        ceiling: u64,
        cancellation: &zetesis_algebra::Cancellation,
    ) -> Result<u64, GpuError> {
        if candidates.len() != self.worlds as usize {
            staging.release();
            return Err(capacity(
                "formula candidate count differs from admitted plan",
            ));
        }
        staging.pack(
            &graph.shape.theory,
            candidates,
            self.seeds,
            self.accounted,
            ceiling,
            cancellation,
        )
    }
}

fn invalid() -> GpuError {
    GpuError::new(GpuErrorKind::Readback, "invalid formula result record")
}

/// Decoded rows are reserved before submission, so a concurrently admitted
/// lookahead can use the actual capacity rather than an allocation estimate.
pub(super) struct Output {
    rows: Vec<FormulaCheck>,
    pub(super) accounted: u64,
}

impl Output {
    pub(super) fn reserve(plan: &Plan) -> Result<Self, GpuError> {
        let worlds = plan.worlds as usize;
        let rows = vector(worlds)?;
        let excess = mul(
            (rows.capacity() - worlds) as u64,
            size_of::<FormulaCheck>() as u64,
        )?;
        let accounted = sum(&[plan.accounted, excess])?;
        if accounted > plan.max_batch_bytes {
            return Err(capacity(
                "decoded formula checks exceed the batch allowance",
            ));
        }
        Ok(Self { rows, accounted })
    }
}

/// Each metadata row and its two masks share one world stride. The retained
/// seed slice is the exact logical upload for this submission.
pub(super) struct Readback<'a> {
    pub(super) plan: &'a Plan,
    pub(super) seeds: &'a [u32],
    pub(super) output: Output,
}

#[cfg(test)]
pub(super) fn decode(
    words: &[u32],
    plan: &Plan,
    seeds: &[u32],
    cancellation: &zetesis_algebra::Cancellation,
) -> Result<Vec<FormulaCheck>, GpuError> {
    Readback {
        plan,
        seeds,
        output: Output::reserve(plan)?,
    }
    .decode(words, cancellation)
}

impl Readback<'_> {
    pub(super) fn decode(
        self,
        words: &[u32],
        cancellation: &zetesis_algebra::Cancellation,
    ) -> Result<Vec<FormulaCheck>, GpuError> {
        let Self {
            plan,
            seeds,
            output:
                Output {
                    rows: mut output,
                    accounted: actual,
                },
        } = self;
        let fail = invalid;
        let width = plan.atoms.div_ceil(32) as usize;
        let stride = HEADER_WORDS + 2 * width;
        let count = plan.worlds as usize;
        if count.checked_mul(stride) != Some(words.len())
            || count.checked_mul(width).map(|n| n.max(1)) != Some(seeds.len())
            || (width == 0 && seeds[0] != 0)
        {
            return Err(fail());
        }
        for (world, row) in words.chunks_exact(stride).enumerate() {
            cancellation.poll().map_err(GpuError::interrupted)?;
            let candidate = &seeds[world * width..(world + 1) * width];
            let (held, cut) = row[HEADER_WORDS..].split_at(width);
            let rounds = row[3];
            let work = row[4];
            // Admit complete sweeps against the full reservation. The two terminal
            // verdicts skip the atom-domain scan and report only performed stores.
            let reserved = rounds
                .checked_mul(plan.sweep)
                .and_then(|n| n.checked_add(plan.setup))
                .ok_or_else(invalid)?;
            let expected = if matches!(row[2], 1 | 2) {
                reserved.checked_sub(plan.atoms).ok_or_else(invalid)?
            } else {
                reserved
            };
            if row[0] != plan.epoch
                || row[1] as usize != world
                || row[5] != MAGIC
                || rounds > plan.max_rounds
                || reserved > plan.max_work
                || work != expected
            {
                return Err(fail());
            }
            let reason = match row[2] {
                1 if rounds == 0 => None,
                2 if rounds > 0 => None,
                3 if rounds > 0 => Some(ResidualReason::FixedPoint),
                4 if rounds == plan.max_rounds => Some(ResidualReason::RoundLimit),
                5 if rounds < plan.max_rounds && plan.max_work - work < plan.sweep => {
                    Some(ResidualReason::WorkLimit)
                }
                _ => return Err(fail()),
            };
            let verdict = if let Some(reason) = reason {
                FormulaVerdict::Residual(
                    reason,
                    decode_root(plan, actual, candidate, held, cut, cancellation)?,
                )
            } else {
                if held.iter().zip(cut).any(|(&h, &c)| h | c != 0) {
                    return Err(fail());
                }
                if row[2] == 1 {
                    FormulaVerdict::NotModel
                } else {
                    FormulaVerdict::NoProperSubset
                }
            };
            output.push(FormulaCheck {
                verdict,
                statistics: FormulaStatistics { work, rounds },
            });
        }
        Ok(output)
    }
}

/// Keep root construction inside the announced peak. Lookahead may own the rest.
fn admit_decode_storage(
    other: u64,
    requested: u128,
    prepared: u64,
) -> Result<(), zetesis_algebra::Stop> {
    let total = u128::from(other)
        .checked_add(requested)
        .ok_or(zetesis_algebra::Stop::StorageLimit)?;
    zetesis_algebra::MeterBytes::check(total, u128::from(prepared))
        .map_err(|_| zetesis_algebra::Stop::StorageLimit)
}

/// Materialize decisions for a subset of the submitted candidate. Every atom
/// outside the candidate must be cut.
fn decode_root(
    plan: &Plan,
    actual: u64,
    candidate: &[u32],
    held: &[u32],
    cut: &[u32],
    cancellation: &zetesis_algebra::Cancellation,
) -> Result<Region, GpuError> {
    use zetesis_algebra::Stop;
    let atoms = plan.atoms as usize;
    for (word, ((&present, &yes), &no)) in candidate.iter().zip(held).zip(cut).enumerate() {
        cancellation.poll().map_err(GpuError::interrupted)?;
        let bits = (atoms - word * 32).min(32);
        let valid = u32::MAX >> (32 - bits);
        if (present | yes | no) & !valid != 0
            || yes & no != 0
            || yes & !present != 0
            || !present & valid & !no != 0
        {
            return Err(invalid());
        }
    }
    let other = actual
        .checked_sub(root_mask_bytes(plan.atoms) + ROOT_STAGING_BYTES)
        .ok_or_else(invalid)?;
    let mut region = Region::try_all_open_with(
        atoms,
        &mut (),
        &mut |(), _| cancellation.poll(),
        &mut |(), bytes| admit_decode_storage(other, bytes, actual),
    )
    .map_err(|stop| match stop {
        Stop::Allocation => GpuError::new(
            GpuErrorKind::Allocation,
            "formula residual root allocation failed",
        ),
        Stop::StorageLimit => capacity("formula residual root exceeds the batch allowance"),
        Stop::Cancelled | Stop::Deadline => GpuError::interrupted(stop),
        _ => invalid(),
    })?;
    for (word, (&yes, &no)) in held.iter().zip(cut).enumerate() {
        cancellation.poll().map_err(GpuError::interrupted)?;
        for bit in zetesis_algebra::words::Words::<u32>::bits(yes) {
            if !region.hold(word * 32 + bit) {
                return Err(invalid());
            }
        }
        for bit in zetesis_algebra::words::Words::<u32>::bits(no) {
            if !region.cut(word * 32 + bit) {
                return Err(invalid());
            }
        }
    }
    verify_materialized(&region, held, cut, cancellation)?;
    Ok(region)
}

// Verify operation polarity and the 32-bit wire → 64-bit host representation
// independently of the decision publication loop.
fn verify_materialized(
    region: &Region,
    held: &[u32],
    cut: &[u32],
    cancellation: &zetesis_algebra::Cancellation,
) -> Result<(), GpuError> {
    let (stored_held, stored_cut) = region.decision_words();
    for (i, (&yes, &no)) in held.iter().zip(cut).enumerate() {
        cancellation.poll().map_err(GpuError::interrupted)?;
        let shift = u32::BITS as usize * (i % 2);
        let half_word = |packed: u64| (packed >> shift) & u64::from(u32::MAX);
        if half_word(stored_held[i / 2]) != u64::from(yes)
            || half_word(stored_cut[i / 2]) != u64::from(no)
        {
            return Err(invalid());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
