//! Pure bounded layout, canonical certificate packing and ordered result decoding.

use super::{TightGpuCheck, TightGpuLimits, poll};
use crate::{GpuError, GpuErrorKind};
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, Node, Theory, TightPlan, TightVerdict};

pub(super) const PARAM_BYTES: u64 = 32;
pub(super) const RESULT_WORDS: usize = 6;
pub(super) const RESULT_MAGIC: u32 = 0x5453_5031;

// Wire tags mirror check.wgsl. Producer presence has its own explicit field;
// an absent body never borrows a node identifier as a sentinel.
const NODE_FALSE: u32 = 0;
const NODE_ATOM: u32 = 1;
const NODE_AND: u32 = 2;
const NODE_OR: u32 = 3;
const NODE_IMPLIES: u32 = 4;
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

pub(super) struct Packed {
    pub(super) nodes: Vec<u32>,
    pub(super) roots: Vec<u32>,
    pub(super) producers: Vec<u32>,
}

pub(super) struct Graph {
    pub(super) theory: Theory,
    pub(super) atoms: u32,
    pub(super) nodes: u32,
    pub(super) roots: u32,
    pub(super) producers: u32,
    pub(super) words: u32,
    pub(super) node_bytes: u64,
    pub(super) root_bytes: u64,
    pub(super) producer_bytes: u64,
    pub(super) bytes: u64,
    pub(super) work: u32,
}
impl Graph {
    pub(super) fn new(certificate: &TightPlan, device: &wgpu::Limits) -> Result<Self, GpuError> {
        let theory = certificate.theory();
        let atoms = address(theory.atom_count())?;
        let nodes = address(theory.nodes().len())?;
        let roots = address(theory.roots().len())?;
        let producers = address(certificate.producers().len())?;
        if [atoms, nodes, roots, producers]
            .into_iter()
            .any(|n| n > u32::MAX - 64)
        {
            return Err(capacity("tight strided loop increment exceeds u32"));
        }
        let work = atoms
            .checked_mul(2)
            .and_then(|n| n.checked_add(nodes))
            .and_then(|n| n.checked_add(roots))
            .and_then(|n| n.checked_add(producers))
            .ok_or_else(|| capacity("tight complete-scan work exceeds u32"))?;
        let node_bytes = u64::from(nodes.max(1)) * 16;
        let root_bytes = u64::from(roots.max(1)) * 4;
        let producer_bytes = u64::from(producers.max(1)) * 16;
        for bytes in [node_bytes, root_bytes, producer_bytes] {
            buffer(bytes, device)?;
        }
        Ok(Self {
            theory: theory.clone(),
            atoms,
            nodes,
            roots,
            producers,
            words: atoms.div_ceil(32),
            node_bytes,
            root_bytes,
            producer_bytes,
            bytes: sum(&[node_bytes, root_bytes, producer_bytes])?,
            work,
        })
    }

    pub(super) fn pack(
        &self,
        certificate: &TightPlan,
        control: &Control,
    ) -> Result<Packed, GpuError> {
        if !self.theory.same_instance(certificate.theory()) {
            return Err(GpuError::new(
                GpuErrorKind::Seed,
                "certificate belongs to another Theory",
            ));
        }
        let mut nodes = words(self.node_bytes)?;
        for node in self.theory.nodes() {
            poll(control)?;
            nodes.extend(match *node {
                Node::False => [NODE_FALSE, 0, 0, 0],
                Node::Atom(atom) => [NODE_ATOM, address(atom)?, 0, 0],
                Node::And(a, b) => [NODE_AND, address(a)?, address(b)?, 0],
                Node::Or(a, b) => [NODE_OR, address(a)?, address(b)?, 0],
                Node::Implies(a, b) => [NODE_IMPLIES, address(a)?, address(b)?, 0],
            });
        }
        if nodes.is_empty() {
            nodes.extend([0; 4]);
        }
        let mut roots = words(self.root_bytes)?;
        for &root in self.theory.roots() {
            poll(control)?;
            roots.push(address(root)?);
        }
        if roots.is_empty() {
            roots.push(0);
        }
        let mut producers = words(self.producer_bytes)?;
        for producer in certificate.producers() {
            poll(control)?;
            let (body, present) = match producer.body() {
                Some(body) => (address(body)?, 1),
                None => (0, 0),
            };
            producers.extend([address(producer.head())?, body, present, 0]);
        }
        if producers.is_empty() {
            producers.extend([0; 4]);
        }
        Ok(Packed {
            nodes,
            roots,
            producers,
        })
    }
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
        if worlds > limits.max_candidates || count > device.max_compute_workgroups_per_dimension {
            return Err(capacity("tight batch exceeds candidate/dispatch ceiling"));
        }
        if limits.max_work_per_candidate < u64::from(graph.work) || epoch == 0 {
            return Err(capacity("tight full-scan work or epoch exceeds limits"));
        }
        let array = |length: u32| -> Result<u64, GpuError> {
            let elements = u64::from(length) * u64::from(count);
            if elements > u64::from(u32::MAX) {
                return Err(capacity("tight world offset exceeds u32"));
            }
            Ok(elements.max(1) * 4)
        };
        let seeds = array(graph.words)?;
        let truth = array(graph.nodes)?;
        let support = array(graph.atoms)?;
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
            if fresh { graph.bytes } else { 0 },
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
            work: graph.work,
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
            self.worlds,
            self.work,
            self.epoch,
        ]
    }

    pub(super) fn pack(
        &self,
        graph: &Graph,
        candidates: &[Interpretation],
        control: &Control,
    ) -> Result<Vec<u32>, GpuError> {
        if candidates.len() != self.worlds as usize {
            return Err(capacity("tight candidate count differs from plan"));
        }
        let mut output = words(self.seeds)?;
        output.resize(
            usize::try_from(self.seeds / 4).map_err(|_| capacity("tight seeds exceed host"))?,
            0,
        );
        for (world, candidate) in candidates.iter().enumerate() {
            poll(control)?;
            if !graph.theory.same_instance(candidate.theory()) {
                return Err(GpuError::new(
                    GpuErrorKind::Seed,
                    "candidate belongs to another Theory",
                ));
            }
            for atom in candidate.atoms() {
                poll(control)?;
                output[world * graph.words as usize + atom / 32] |= 1 << (atom % 32);
            }
        }
        Ok(output)
    }
}

pub(super) fn decode(
    words: &[u32],
    graph: &Graph,
    plan: &Plan,
    seeds: &[u32],
    control: &Control,
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
        poll(control)?;
        if row[0] != plan.epoch
            || row[1] as usize != world
            || row[4] != plan.work
            || row[5] != RESULT_MAGIC
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
#[path = "../../tests/tight/packing.rs"]
mod tests;
