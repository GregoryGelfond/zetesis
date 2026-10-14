//! Pure bounded layout, canonical certificate packing and ordered result decoding.

use super::{TightGpuCheck, TightGpuLimits, TightSupport, poll};
use crate::formula_graph::dependency;
use crate::{GpuError, GpuErrorKind};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Theory, TightPlan, TightVerdict};

pub(super) const PARAM_BYTES: u64 = 32;
pub(super) const RESULT_WORDS: usize = 6;
pub(super) const RESULT_MAGIC: u32 = 0x5453_5031;
pub(super) const RESULT_GROUPED_MAGIC: u32 = 0x5453_4731;

const STATUS_STABLE: u32 = 0;
const STATUS_NOT_MODEL: u32 = 1;
const STATUS_RESIDUAL: u32 = 2;

fn capacity(detail: &str) -> GpuError {
    GpuError::new(GpuErrorKind::Capacity, detail)
}
fn address(value: usize) -> Result<u32, GpuError> {
    u32::try_from(value).map_err(|_| capacity("tight dimension exceeds u32"))
}
fn sum(values: &[u64]) -> Result<u64, GpuError> {
    values.iter().try_fold(0u64, |total, next| {
        total
            .checked_add(*next)
            .ok_or_else(|| capacity("tight byte sum overflow"))
    })
}
fn buffer(bytes: u64, device: &wgpu::Limits) -> Result<(), GpuError> {
    if bytes > device.max_buffer_size || bytes > device.max_storage_buffer_binding_size {
        return Err(capacity("tight storage exceeds granted device limits"));
    }
    Ok(())
}
fn vector<T>(length: usize) -> Result<Vec<T>, GpuError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| GpuError::new(GpuErrorKind::Allocation, "tight host reservation failed"))?;
    Ok(output)
}
fn words(bytes: u64) -> Result<Vec<u32>, GpuError> {
    vector(usize::try_from(bytes / 4).map_err(|_| capacity("tight buffer exceeds host"))?)
}

fn producer_storage(producers: u32, words: u32, support: TightSupport) -> Result<u64, GpuError> {
    let producer_words = u64::from(producers) * 4
        + if support == TightSupport::Grouped {
            u64::from(words) + 1
        } else {
            0
        };
    // The shader addresses records and appended offsets with u32 indices,
    // independently of a device's hypothetical byte capacity.
    if producer_words > u64::from(u32::MAX) {
        return Err(capacity("tight producer indexing exceeds u32"));
    }
    Ok(producer_words.max(4) * 4)
}

pub(super) struct Packed {
    pub(super) nodes: Vec<u32>,
    pub(super) roots: Vec<u32>,
    pub(super) producers: Vec<u32>,
}

#[derive(Clone, Copy)]
pub(super) struct Packing<'a> {
    pub(super) device: &'a wgpu::Limits,
    pub(super) support: TightSupport,
}

pub(super) struct Graph {
    pub(super) theory: Theory,
    pub(super) atoms: u32,
    pub(super) nodes: u32,
    pub(super) roots: u32,
    pub(super) edges: u32,
    pub(super) wide_words: u32,
    pub(super) levels: u32,
    pub(super) rows: crate::formula_graph::Rows,
    pub(super) producers: u32,
    pub(super) words: u32,
    pub(super) node_bytes: u64,
    pub(super) root_bytes: u64,
    pub(super) producer_bytes: u64,
    pub(super) bytes: u64,
    /// Simultaneous host packing, including temporary grouped-write cursors.
    pub(super) packing_bytes: u64,
    pub(super) work: u32,
    support: TightSupport,
}
impl Graph {
    const fn result_marker(&self) -> u32 {
        match self.support {
            TightSupport::Atomic => RESULT_MAGIC,
            TightSupport::Grouped => RESULT_GROUPED_MAGIC,
        }
    }

    #[cfg(test)]
    pub(super) fn new(certificate: &TightPlan, packing: Packing<'_>) -> Result<Self, GpuError> {
        Self::with_control(certificate, packing, u64::MAX, &Cancellation::default())
    }

    pub(super) fn with_control(
        certificate: &TightPlan,
        packing: Packing<'_>,
        max_staging_bytes: u64,
        cancellation: &Cancellation,
    ) -> Result<Self, GpuError> {
        let theory = certificate.theory();
        let atoms = address(theory.atom_count())?;
        let graph = crate::formula_graph::Shape::with_control(
            theory,
            packing.device,
            max_staging_bytes,
            cancellation,
        )?;
        let nodes = graph.nodes;
        let roots = address(theory.roots().len())?;
        let producers = address(certificate.producers().len())?;
        if [atoms, nodes, roots, producers]
            .into_iter()
            .any(|n| n > u32::MAX - 64)
        {
            return Err(capacity("tight strided loop increment exceeds u32"));
        }
        let words = atoms.div_ceil(32);
        let node_bytes = graph.bytes;
        let root_bytes = u64::from(roots.max(1)) * 4;
        let grouped = packing.support == TightSupport::Grouped;
        let producer_bytes = producer_storage(producers, words, packing.support)?;
        for bytes in [node_bytes, root_bytes, producer_bytes] {
            buffer(bytes, packing.device)?;
        }
        let bytes = sum(&[node_bytes, root_bytes, producer_bytes])?;
        let packing_bytes = sum(&[
            bytes,
            graph.rows.bytes()?,
            if grouped { u64::from(words) * 4 } else { 0 },
        ])?;
        let mut shape = Self {
            theory: theory.clone(),
            atoms,
            nodes,
            roots,
            edges: graph.edges,
            wide_words: graph.wide_words,
            levels: 0,
            rows: graph.rows,
            producers,
            words,
            node_bytes,
            root_bytes,
            producer_bytes,
            bytes,
            packing_bytes,
            work: 0,
            support: packing.support,
        };
        shape.work = words
            .checked_mul(2)
            .and_then(|n| n.checked_add(nodes))
            .and_then(|n| n.checked_add(shape.edges))
            .and_then(|n| n.checked_add(roots))
            .and_then(|n| n.checked_add(producers))
            .and_then(|n| n.checked_add(graph.numeric_work))
            .ok_or_else(|| capacity("tight complete-scan work exceeds u32"))?;
        Ok(shape)
    }

    fn pack_nodes(
        &self,
        certificate: &TightPlan,
        cancellation: &Cancellation,
    ) -> Result<Vec<u32>, GpuError> {
        if !self.theory.same_instance(certificate.theory()) {
            return Err(GpuError::new(
                GpuErrorKind::Seed,
                "certificate belongs to another Theory",
            ));
        }
        let mut nodes = words(self.node_bytes)?;
        crate::formula_graph::pack(&self.theory, &self.rows, &mut nodes, cancellation)?;
        Ok(nodes)
    }

    /// Complete an already-admitted serial shape. Optional level storage is
    /// selected only when its complete cold layout and work also fit.
    pub(super) fn prepare(
        &mut self,
        certificate: &TightPlan,
        limits: TightGpuLimits,
        device: &wgpu::Limits,
        plan: &mut Plan,
        cancellation: &Cancellation,
    ) -> Result<Packed, GpuError> {
        let mut nodes = self.pack_nodes(certificate, cancellation)?;
        let depths =
            dependency::label_depths(self.nodes, self.wide_words, &mut nodes, cancellation)?;
        let levels = dependency::parallel_levels(self.nodes, depths);
        if levels != 0 {
            let serial = (self.root_bytes, self.bytes, self.packing_bytes);
            let scheduled = (|| {
                let root_bytes =
                    u64::from(dependency::root_words(self.nodes, self.roots, levels)?) * 4;
                buffer(root_bytes, device)?;
                let added = root_bytes - self.root_bytes;
                self.bytes = sum(&[self.bytes, added])?;
                self.packing_bytes = sum(&[self.packing_bytes, added])?;
                self.root_bytes = root_bytes;
                self.levels = levels;
                Plan::new(self, plan.worlds as usize, limits, device, true, plan.epoch)
            })();
            match scheduled {
                Ok(selected) if selected.levels != 0 => *plan = selected,
                Ok(_) => {
                    (self.root_bytes, self.bytes, self.packing_bytes) = serial;
                    self.levels = 0;
                }
                Err(error) if error.kind() == GpuErrorKind::Capacity => {
                    (self.root_bytes, self.bytes, self.packing_bytes) = serial;
                    self.levels = 0;
                }
                Err(error) => return Err(error),
            }
        }
        self.pack_columns(certificate, nodes, cancellation)
    }

    #[cfg(test)]
    pub(super) fn pack(
        &self,
        certificate: &TightPlan,
        cancellation: &Cancellation,
    ) -> Result<Packed, GpuError> {
        self.pack_columns(
            certificate,
            self.pack_nodes(certificate, cancellation)?,
            cancellation,
        )
    }

    fn pack_columns(
        &self,
        certificate: &TightPlan,
        mut nodes: Vec<u32>,
        cancellation: &Cancellation,
    ) -> Result<Packed, GpuError> {
        let mut roots = words(self.root_bytes)?;
        dependency::pack(
            &self.theory,
            self.nodes,
            self.levels,
            &nodes,
            &mut roots,
            cancellation,
        )?;
        // Depths are preparation scratch, never resident node state.
        for header in nodes.chunks_exact_mut(4).take(self.nodes as usize) {
            poll(cancellation)?;
            header[3] = 0;
        }
        let producers = self.pack_producers(certificate, cancellation)?;
        Ok(Packed {
            nodes,
            roots,
            producers,
        })
    }

    fn pack_producers(
        &self,
        certificate: &TightPlan,
        cancellation: &Cancellation,
    ) -> Result<Vec<u32>, GpuError> {
        let mut output = words(self.producer_bytes)?;
        if self.support == TightSupport::Atomic {
            for producer in certificate.producers() {
                poll(cancellation)?;
                output.extend(producer_record(producer)?);
            }
            if output.is_empty() {
                output.extend([0; 4]);
            }
            return Ok(output);
        }
        let length = usize::try_from(self.producer_bytes / 4)
            .map_err(|_| capacity("tight producers exceed host"))?;
        output.resize(length, 0);
        let offsets = self.producers as usize * 4;
        // Counts occupy the next group's slot. Prefix summation produces W+1
        // half-open offsets, including zero-length groups and the final bound P.
        for producer in certificate.producers() {
            poll(cancellation)?;
            output[offsets + producer.head() / 32 + 1] += 1;
        }
        for word in 0..self.words as usize {
            poll(cancellation)?;
            let prefix = output[offsets + word];
            output[offsets + word + 1] += prefix;
        }
        let mut cursors = vector(self.words as usize)?;
        cursors.extend_from_slice(&output[offsets..offsets + self.words as usize]);
        // Each canonical occurrence advances precisely its word's cursor once.
        // Thus all occurrences are retained, in original order within a group.
        for producer in certificate.producers() {
            poll(cancellation)?;
            let cursor = &mut cursors[producer.head() / 32];
            let start = *cursor as usize * 4;
            output[start..start + 4].copy_from_slice(&producer_record(producer)?);
            *cursor += 1;
        }
        Ok(output)
    }
}

fn producer_record(producer: &zetesis_ferraris::TightProducer) -> Result<[u32; 4], GpuError> {
    let (body, present) = match producer.body() {
        Some(body) => (address(body)?, 1),
        None => (0, 0),
    };
    Ok([address(producer.head())?, body, present, 0])
}

pub(super) struct Plan {
    pub(super) worlds: u32,
    pub(super) seeds: u64,
    pub(super) truth: u64,
    pub(super) support: u64,
    pub(super) results: u64,
    pub(super) transport: u64,
    pub(super) accounted: u64,
    pub(super) work: u32,
    pub(super) levels: u32,
    pub(super) epoch: u32,
}
impl Plan {
    pub(super) fn new(
        graph: &Graph,
        worlds: usize,
        limits: TightGpuLimits,
        device: &wgpu::Limits,
        fresh: bool,
        epoch: u32,
    ) -> Result<Self, GpuError> {
        let count = address(worlds)?;
        crate::runtime::positive_timeout(limits.timeout)?;
        if worlds > limits.max_candidates || count > device.max_compute_workgroups_per_dimension {
            return Err(capacity("tight batch exceeds candidate/dispatch ceiling"));
        }
        if limits.max_work_per_candidate < u64::from(graph.work) || epoch == 0 {
            return Err(capacity("tight full-scan work or epoch exceeds limits"));
        }
        // A resident schedule may be bypassed under a tighter work allowance.
        // Its real retained storage is still charged below.
        let (levels, work) = graph
            .work
            .checked_add(graph.levels)
            .filter(|work| u64::from(*work) <= limits.max_work_per_candidate)
            .map_or((0, graph.work), |work| (graph.levels, work));
        let array = |length: u32| -> Result<u64, GpuError> {
            let elements = u64::from(length) * u64::from(count);
            if elements > u64::from(u32::MAX) {
                return Err(capacity("tight world offset exceeds u32"));
            }
            Ok(elements.max(1) * 4)
        };
        let seeds = array(graph.words)?;
        let truth = array(graph.nodes)?;
        let support = array(graph.words)?;
        let results = array(address(RESULT_WORDS)?)?;
        for bytes in [seeds, truth, support, results] {
            buffer(bytes, device)?;
        }
        if device.max_uniform_buffer_binding_size < PARAM_BYTES
            || device.max_buffer_size < PARAM_BYTES
        {
            return Err(capacity(
                "tight uniform buffer exceeds granted device limits",
            ));
        }
        let transport = sum(&[PARAM_BYTES, seeds, truth, support, results, results])?;
        let host_results = u64::from(count) * std::mem::size_of::<TightGpuCheck>() as u64;
        let accounted = sum(&[
            graph.bytes,
            transport,
            PARAM_BYTES,
            seeds,
            host_results,
            if fresh { graph.packing_bytes } else { 0 },
        ])?;
        if worlds != 0 && accounted > limits.max_batch_bytes {
            return Err(capacity("tight batch exceeds authored byte ceiling"));
        }
        Ok(Self {
            worlds: count,
            seeds,
            truth,
            support,
            results,
            transport,
            accounted,
            work,
            levels,
            epoch,
        })
    }

    pub(super) fn params(&self, graph: &Graph) -> [u32; 8] {
        [
            graph.atoms,
            graph.nodes,
            graph.roots,
            graph.producers,
            graph.words,
            self.levels,
            self.work,
            self.epoch,
        ]
    }

    pub(super) fn pack(
        &self,
        graph: &Graph,
        candidates: &[Interpretation],
        cancellation: &Cancellation,
    ) -> Result<Vec<u32>, GpuError> {
        if candidates.len() != self.worlds as usize {
            return Err(capacity("tight candidate count differs from plan"));
        }
        let mut output = words(self.seeds)?;
        output.resize(
            usize::try_from(self.seeds / 4).map_err(|_| capacity("tight seeds exceed host"))?,
            0,
        );
        crate::candidates::pack(&graph.theory, candidates, &mut output, cancellation)?;
        Ok(output)
    }
}

pub(super) fn decode(
    words: &[u32],
    graph: &Graph,
    plan: &Plan,
    seeds: &[u32],
    cancellation: &Cancellation,
) -> Result<Vec<TightGpuCheck>, GpuError> {
    let fail = || GpuError::new(GpuErrorKind::Readback, "invalid tight result record");
    if words.len() != plan.worlds as usize * RESULT_WORDS
        || u64::try_from(seeds.len())
            .ok()
            .and_then(|n| n.checked_mul(4))
            != Some(plan.seeds)
    {
        return Err(fail());
    }
    let mut output = vector(plan.worlds as usize)?;
    for (world, row) in words.chunks_exact(RESULT_WORDS).enumerate() {
        poll(cancellation)?;
        if row[0] != plan.epoch
            || row[1] as usize != world
            || row[4] != plan.work
            || row[5] != graph.result_marker()
        {
            return Err(fail());
        }
        let verdict = match (row[2], row[3]) {
            (STATUS_STABLE, 0) => TightVerdict::Stable,
            (STATUS_NOT_MODEL, ordinal) if ordinal < graph.roots => TightVerdict::NotModel {
                root: graph.theory.roots()[ordinal as usize],
            },
            (STATUS_RESIDUAL, atom)
                if atom < graph.atoms
                    && seeds[world * graph.words as usize + atom as usize / 32]
                        & (1 << (atom % 32))
                        != 0 =>
            {
                TightVerdict::Residual {
                    unsupported_atom: atom as usize,
                }
            }
            _ => return Err(fail()),
        };
        output.push(TightGpuCheck {
            verdict,
            work: u64::from(plan.work),
        });
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
