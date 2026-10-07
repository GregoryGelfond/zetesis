//! Checked integer layout and strict result decoding; no hardware access.

use super::{FormulaCheck, FormulaLimits, FormulaStatistics, FormulaVerdict, ResidualReason};
use crate::{GpuError, GpuErrorKind};
use zetesis_ferraris::{Interpretation, Theory};

pub(super) const PARAM_BYTES: u64 = 48;
pub(super) const RESULT_WORDS: usize = 6;
pub(super) const MAGIC: u32 = 0x4652_5031;

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
    pub(super) variables: u32,
    pub(super) words: u32,
    pub(super) node_bytes: u64,
    pub(super) setup_work: u32,
    pub(super) sweep_work: u32,
}
impl Shape {
    pub(super) fn new(theory: &Theory, device: &wgpu::Limits) -> Result<Self, GpuError> {
        let atoms = address(theory.atom_count())?;
        let nodes = address(theory.nodes().len())?;
        let roots = address(theory.roots().len())?;
        let graph = crate::formula_graph::Shape::new(theory, device)?;
        let auxiliary = graph.auxiliary;
        let variables = atoms
            .checked_add(auxiliary)
            .ok_or_else(|| capacity("formula variable addresses overflow"))?;
        // Strided GPU loops increment by 64 after their last valid address.
        if [atoms, nodes, roots, variables]
            .into_iter()
            .any(|n| n > u32::MAX - 64)
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
            variables,
            words,
            node_bytes,
            setup_work: 0,
            sweep_work: 0,
        };
        shape.setup_work = nodes
            .checked_mul(2)
            .and_then(|n| n.checked_add(shape.edges))
            .and_then(|n| n.checked_add(atoms))
            .and_then(|n| n.checked_add(roots))
            .ok_or_else(|| capacity("formula setup work overflows"))?;
        shape.sweep_work = nodes
            .checked_mul(9)
            .and_then(|n| shape.edges.checked_mul(2).and_then(|e| n.checked_add(e)))
            .and_then(|n| n.checked_add(atoms))
            .and_then(|n| n.checked_add(65))
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
}
impl Schedule {
    pub(super) fn new(shape: &Shape, levels: u32, device: &wgpu::Limits) -> Result<Self, GpuError> {
        let root_words = if levels == 0 {
            shape.roots.max(1)
        } else {
            if levels > shape.nodes {
                return Err(capacity("formula levels exceed node count"));
            }
            shape
                .roots
                .checked_add(levels)
                .and_then(|n| n.checked_add(1))
                .and_then(|n| n.checked_add(shape.nodes))
                .filter(|n| *n <= u32::MAX - 64)
                .ok_or_else(|| capacity("formula schedule addresses overflow"))?
        };
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
        })
    }
}

/// Final shape and schedule only; temporary depth words never belong to Graph.
pub(super) struct Graph {
    pub(super) shape: Shape,
    pub(super) schedule: Schedule,
}

pub(super) struct Plan {
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
        let domains = array(shape.variables)?;
        let results = mul(u64::from(count.max(1)), 24)?;
        if u64::from(count) * 6 > u64::from(u32::MAX) {
            return Err(capacity("formula result offset exceeds u32"));
        }
        for bytes in [seeds, masks, domains, results] {
            buffer(bytes, device)?;
        }
        if device.max_uniform_buffer_binding_size < PARAM_BYTES {
            return Err(capacity("formula uniform buffer exceeds device limit"));
        }
        let transport = sum(&[PARAM_BYTES, seeds, masks, domains, results, results])?;
        let host_results = mul(u64::from(count), std::mem::size_of::<FormulaCheck>() as u64)?;
        let accounted = sum(&[
            schedule.bytes,
            transport,
            PARAM_BYTES,
            seeds,
            host_results,
            if fresh { schedule.bytes } else { 0 },
        ])?;
        if worlds != 0 && accounted > limits.max_batch_bytes {
            return Err(capacity("formula batch exceeds authored byte ceiling"));
        }
        Ok(Self {
            worlds: count,
            seeds,
            masks,
            domains,
            results,
            transport,
            accounted,
            setup: schedule.setup_work,
            sweep: shape.sweep_work,
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
    pub(super) fn params(&self, graph: &Graph) -> [u32; 12] {
        [
            graph.shape.atoms,
            graph.shape.nodes,
            graph.shape.roots,
            graph.shape.variables,
            graph.shape.words,
            self.worlds,
            self.max_rounds,
            self.max_work,
            self.setup,
            self.sweep,
            self.epoch,
            graph.schedule.levels,
        ]
    }
    pub(super) fn pack(
        &self,
        graph: &Graph,
        candidates: &[Interpretation],
        cancellation: &zetesis_cpu::Cancellation,
    ) -> Result<Vec<u32>, GpuError> {
        if candidates.len() != self.worlds as usize {
            return Err(capacity(
                "formula candidate count differs from admitted plan",
            ));
        }
        let length = usize::try_from(self.seeds / 4)
            .map_err(|_| capacity("formula candidate bytes exceed host"))?;
        let mut words = vector(length)?;
        words.resize(length, 0u32);
        crate::candidates::pack(&graph.shape.theory, candidates, &mut words, cancellation)?;
        Ok(words)
    }
}

pub(super) fn decode(words: &[u32], plan: &Plan) -> Result<Vec<FormulaCheck>, GpuError> {
    let fail = || GpuError::new(GpuErrorKind::Readback, "invalid formula result record");
    if words.len() != plan.worlds as usize * RESULT_WORDS {
        return Err(fail());
    }
    let mut output = vector(plan.worlds as usize)?;
    for (world, row) in words.chunks_exact(RESULT_WORDS).enumerate() {
        let rounds = row[3];
        let work = row[4];
        if row[0] != plan.epoch
            || row[1] as usize != world
            || row[5] != MAGIC
            || rounds > plan.max_rounds
            || work > plan.max_work
            || rounds
                .checked_mul(plan.sweep)
                .and_then(|n| n.checked_add(plan.setup))
                != Some(work)
        {
            return Err(fail());
        }
        let verdict = match row[2] {
            1 if rounds == 0 => FormulaVerdict::NotModel,
            2 if rounds > 0 => FormulaVerdict::NoProperSubset,
            3 if rounds > 0 => FormulaVerdict::Residual(ResidualReason::FixedPoint),
            4 if rounds == plan.max_rounds => FormulaVerdict::Residual(ResidualReason::RoundLimit),
            5 if rounds < plan.max_rounds && plan.max_work - work < plan.sweep => {
                FormulaVerdict::Residual(ResidualReason::WorkLimit)
            }
            _ => return Err(fail()),
        };
        output.push(FormulaCheck {
            verdict,
            statistics: FormulaStatistics { work, rounds },
        });
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
