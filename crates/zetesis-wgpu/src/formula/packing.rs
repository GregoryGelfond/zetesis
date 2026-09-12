//! Checked integer layout and strict result decoding; no hardware access.

use super::{FormulaCheck, FormulaLimits, FormulaStatistics, FormulaVerdict, ResidualReason};
use crate::{GpuError, GpuErrorKind};
use zetesis_ferraris::{Interpretation, Node, Theory};

pub(super) const PARAM_BYTES: u64 = 48;
pub(super) const RESULT_WORDS: usize = 6;
pub(super) const MAGIC: u32 = 0x4652_5031;

fn capacity(detail: &str) -> GpuError {
    GpuError::new(GpuErrorKind::Capacity, detail)
}
fn address(value: usize) -> Result<u32, GpuError> {
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

pub(super) struct Graph {
    pub(super) theory: Theory,
    pub(super) atoms: u32,
    pub(super) nodes: u32,
    pub(super) roots: u32,
    pub(super) variables: u32,
    pub(super) words: u32,
    pub(super) node_bytes: u64,
    pub(super) root_bytes: u64,
    pub(super) bytes: u64,
    pub(super) setup_work: u32,
    pub(super) sweep_work: u32,
}
impl Graph {
    pub(super) fn new(theory: &Theory, device: &wgpu::Limits) -> Result<Self, GpuError> {
        let atoms = address(theory.atom_count())?;
        let nodes = address(theory.nodes().len())?;
        let roots = address(theory.roots().len())?;
        let auxiliary = address(
            theory
                .nodes()
                .iter()
                .filter(|node| !matches!(node, Node::Atom(_)))
                .count(),
        )?;
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
        let node_bytes = mul(u64::from(nodes.max(1)), 16)?;
        let root_bytes = mul(u64::from(roots.max(1)), 4)?;
        buffer(node_bytes, device)?;
        buffer(root_bytes, device)?;
        let setup_work = nodes
            .checked_mul(2)
            .and_then(|n| n.checked_add(atoms))
            .and_then(|n| n.checked_add(roots))
            .ok_or_else(|| capacity("formula setup work overflows"))?;
        let sweep_work = nodes
            .checked_mul(9)
            .and_then(|n| n.checked_add(atoms))
            .and_then(|n| n.checked_add(65))
            .ok_or_else(|| capacity("formula sweep work overflows"))?;
        Ok(Self {
            theory: theory.clone(),
            atoms,
            nodes,
            roots,
            variables,
            words,
            node_bytes,
            root_bytes,
            bytes: sum(&[node_bytes, root_bytes])?,
            setup_work,
            sweep_work,
        })
    }
    pub(super) fn pack(&self) -> Result<(Vec<u32>, Vec<u32>), GpuError> {
        let mut nodes = vector(
            usize::try_from(self.node_bytes / 4)
                .map_err(|_| capacity("node length exceeds host"))?,
        )?;
        // Original node indices remain topological references. Only non-Atom
        // nodes need auxiliary domain slots; every leaf aliases its semantic atom.
        let mut output = self.atoms;
        for node in self.theory.nodes() {
            let words = match *node {
                Node::False => [0, 0, 0, output],
                Node::Atom(atom) => [1, address(atom)?, 0, address(atom)?],
                Node::And(a, b) => [2, address(a)?, address(b)?, output],
                Node::Or(a, b) => [3, address(a)?, address(b)?, output],
                Node::Implies(a, b) => [4, address(a)?, address(b)?, output],
            };
            nodes.extend(words);
            if !matches!(node, Node::Atom(_)) {
                output = output
                    .checked_add(1)
                    .ok_or_else(|| capacity("formula output address overflow"))?;
            }
        }
        if nodes.is_empty() {
            nodes.extend([0; 4]);
        }
        let mut roots = vector(
            usize::try_from(self.root_bytes / 4)
                .map_err(|_| capacity("root length exceeds host"))?,
        )?;
        for root in self.theory.roots() {
            roots.push(address(*root)?);
        }
        if roots.is_empty() {
            roots.push(0);
        }
        Ok((nodes, roots))
    }
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
        let count = address(worlds)?;
        if worlds > limits.max_candidates || count > device.max_compute_workgroups_per_dimension {
            return Err(capacity("formula batch exceeds candidate/dispatch ceiling"));
        }
        if limits.max_work_per_candidate < graph.setup_work || epoch == 0 {
            return Err(capacity("mandatory formula setup or epoch exceeds limits"));
        }
        let array = |length: u32| -> Result<u64, GpuError> {
            let elements = u64::from(length) * u64::from(count);
            if elements > u64::from(u32::MAX) {
                return Err(capacity("formula world offset exceeds u32"));
            }
            mul(elements.max(1), 4)
        };
        let seeds = array(graph.words)?;
        let masks = array(graph.nodes)?;
        let domains = array(graph.variables)?;
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
            graph.bytes,
            transport,
            PARAM_BYTES,
            seeds,
            host_results,
            if fresh { graph.bytes } else { 0 },
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
            setup: graph.setup_work,
            sweep: graph.sweep_work,
            max_rounds: limits.max_rounds,
            max_work: limits.max_work_per_candidate,
            epoch,
        })
    }
    pub(super) fn params(&self, graph: &Graph) -> [u32; 12] {
        [
            graph.atoms,
            graph.nodes,
            graph.roots,
            graph.variables,
            graph.words,
            self.worlds,
            self.max_rounds,
            self.max_work,
            self.setup,
            self.sweep,
            self.epoch,
            0,
        ]
    }
    pub(super) fn pack(
        &self,
        graph: &Graph,
        candidates: &[Interpretation],
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
        for (world, candidate) in candidates.iter().enumerate() {
            if !graph.theory.same_instance(candidate.theory()) {
                return Err(GpuError::new(
                    GpuErrorKind::Seed,
                    "candidate belongs to another Theory",
                ));
            }
            for atom in candidate.atoms() {
                words[world * graph.words as usize + atom / 32] |= 1 << (atom % 32);
            }
        }
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
#[path = "../../tests/formula/packing.rs"]
mod tests;
