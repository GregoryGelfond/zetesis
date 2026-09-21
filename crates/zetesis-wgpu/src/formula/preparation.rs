//! Cold graph preparation. Depth words exist only between the two named packing
//! phases; only finalized buffers and Graph can cross into device residency.

use super::FormulaLimits;
use super::packing::{Graph, Plan, Schedule, Shape, address, capacity, vector};
use crate::{GpuError, GpuErrorKind};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Node, Theory};

pub(super) struct Preparation {
    shape: Shape,
    minimum: Plan,
    limits: FormulaLimits,
}

pub(super) struct PreparedGraph {
    pub(super) graph: Graph,
    pub(super) nodes: Vec<u32>,
    pub(super) roots: Vec<u32>,
}

impl Preparation {
    pub(super) fn new(
        theory: &Theory,
        worlds: usize,
        limits: FormulaLimits,
        device: &wgpu::Limits,
        epoch: u32,
    ) -> Result<Self, GpuError> {
        let shape = Shape::new(theory, device)?;
        let serial = Schedule::new(&shape, 0, device)?;
        let minimum = Plan::layout(&shape, &serial, worlds, limits, device, true, epoch)?;
        Ok(Self {
            shape,
            minimum,
            limits,
        })
    }

    pub(super) fn finish(
        self,
        device: &wgpu::Limits,
        cancellation: &Cancellation,
    ) -> Result<(PreparedGraph, Plan), GpuError> {
        self.finish_with(device, cancellation, vector)
    }

    // The allocator seam is the real production reservation path. It returns
    // empty storage and permits tests to retain actual non-ZST spare capacity.
    fn finish_with(
        mut self,
        device: &wgpu::Limits,
        cancellation: &Cancellation,
        mut reserve: impl FnMut(usize) -> Result<Vec<u32>, GpuError>,
    ) -> Result<(PreparedGraph, Plan), GpuError> {
        poll(cancellation)?;
        let mut nodes = reserve_words(&mut reserve, self.shape.node_bytes)?;
        let node_capacity = retained_bytes(&nodes)?;
        self.minimum.staging(
            self.shape.node_bytes,
            node_capacity,
            0,
            self.limits.max_batch_bytes,
        )?;
        let depth_count = pack_depths(&self.shape, &mut nodes, cancellation)?;
        // A pure chain has no independent node work. This structural choice is
        // not a measured crossover policy for other skinny/unbalanced DAGs.
        let levels = if depth_count == self.shape.nodes {
            0
        } else {
            depth_count
        };
        let schedule = Schedule::new(&self.shape, levels, device)?;
        let mut plan = Plan::layout(
            &self.shape,
            &schedule,
            self.minimum.worlds as usize,
            self.limits,
            device,
            true,
            self.minimum.epoch,
        )?;
        plan.staging(
            self.shape.node_bytes,
            node_capacity,
            0,
            self.limits.max_batch_bytes,
        )?;
        poll(cancellation)?;
        let mut roots = reserve_words(&mut reserve, schedule.root_bytes)?;
        plan.staging(
            schedule.root_bytes,
            0,
            retained_bytes(&roots)?,
            self.limits.max_batch_bytes,
        )?;
        pack_schedule(&self.shape, &schedule, &nodes, &mut roots, cancellation)?;
        finalize_outputs(&self.shape, &mut nodes, cancellation)?;
        poll(cancellation)?;
        Ok((
            PreparedGraph {
                graph: Graph {
                    shape: self.shape,
                    schedule,
                },
                nodes,
                roots,
            },
            plan,
        ))
    }
}

fn poll(cancellation: &Cancellation) -> Result<(), GpuError> {
    cancellation.poll().map_err(GpuError::interrupted)
}

fn retained_bytes(words: &Vec<u32>) -> Result<u64, GpuError> {
    u64::try_from(words.capacity())
        .ok()
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| capacity("formula staging capacity overflows"))
}

fn reserve_words(
    reserve: &mut impl FnMut(usize) -> Result<Vec<u32>, GpuError>,
    bytes: u64,
) -> Result<Vec<u32>, GpuError> {
    let length =
        usize::try_from(bytes / 4).map_err(|_| capacity("formula staging exceeds host"))?;
    let words = reserve(length)?;
    if !words.is_empty() || words.capacity() < length {
        return Err(GpuError::new(
            GpuErrorKind::Allocation,
            "invalid formula staging reservation",
        ));
    }
    Ok(words)
}

fn pack_depths(
    shape: &Shape,
    nodes: &mut Vec<u32>,
    cancellation: &Cancellation,
) -> Result<u32, GpuError> {
    let mut levels = 0;
    for node in shape.theory.nodes() {
        poll(cancellation)?;
        let (tag, left, right) = match *node {
            Node::False => (0, 0, 0),
            Node::Atom(atom) => (1, address(atom)?, 0),
            Node::And(a, b) => (2, address(a)?, address(b)?),
            Node::Or(a, b) => (3, address(a)?, address(b)?),
            Node::Implies(a, b) => (4, address(a)?, address(b)?),
        };
        let depth = if tag < 2 {
            0
        } else {
            let child = |index: u32| -> Result<u32, GpuError> {
                nodes
                    .get(index as usize * 4 + 3)
                    .copied()
                    .ok_or_else(|| capacity("formula depth child is not a prior node"))
            };
            child(left)?
                .max(child(right)?)
                .checked_add(1)
                .ok_or_else(|| capacity("formula dependency depth overflows"))?
        };
        levels = levels.max(
            depth
                .checked_add(1)
                .ok_or_else(|| capacity("formula levels overflow"))?,
        );
        nodes.extend([tag, left, right, depth]);
    }
    if nodes.is_empty() {
        nodes.extend([0; 4]);
    }
    Ok(levels)
}

fn pack_schedule(
    shape: &Shape,
    schedule: &Schedule,
    nodes: &[u32],
    roots: &mut Vec<u32>,
    cancellation: &Cancellation,
) -> Result<(), GpuError> {
    poll(cancellation)?;
    roots.resize(
        usize::try_from(schedule.root_bytes / 4)
            .map_err(|_| capacity("formula roots exceed host"))?,
        0,
    );
    for (index, root) in shape.theory.roots().iter().enumerate() {
        poll(cancellation)?;
        roots[index] = address(*root)?;
    }
    if schedule.levels == 0 {
        return Ok(());
    }
    let prefix = shape.roots as usize;
    let levels = schedule.levels as usize;
    let order = prefix + levels + 1;
    // The offset segment changes from counts to prefix ends, then to starts.
    for node in nodes.chunks_exact(4).take(shape.nodes as usize) {
        poll(cancellation)?;
        let count = roots
            .get_mut(prefix + node[3] as usize + 1)
            .filter(|_| node[3] < schedule.levels)
            .ok_or_else(|| capacity("formula depth exceeds schedule"))?;
        *count = count
            .checked_add(1)
            .ok_or_else(|| capacity("formula level count overflows"))?;
    }
    for level in 1..=levels {
        poll(cancellation)?;
        roots[prefix + level] = roots[prefix + level]
            .checked_add(roots[prefix + level - 1])
            .ok_or_else(|| capacity("formula level prefix overflows"))?;
    }
    if roots[prefix + levels] != shape.nodes {
        return Err(capacity("formula level coverage differs"));
    }
    for index in (0..shape.nodes as usize).rev() {
        poll(cancellation)?;
        let end = &mut roots[prefix + nodes[index * 4 + 3] as usize + 1];
        let position = insert_position(end, shape.nodes)?;
        roots[order + position] = address(index)?;
    }
    roots[prefix..order].rotate_left(1);
    roots[order - 1] = shape.nodes;
    Ok(())
}

fn insert_position(end: &mut u32, nodes: u32) -> Result<usize, GpuError> {
    let position = end
        .checked_sub(1)
        .filter(|position| *position < nodes)
        .ok_or_else(|| capacity("formula level insertion exceeds its node prefix"))?;
    *end = position;
    Ok(position as usize)
}

fn finalize_outputs(
    shape: &Shape,
    nodes: &mut [u32],
    cancellation: &Cancellation,
) -> Result<(), GpuError> {
    let mut output = shape.atoms;
    for node in nodes.chunks_exact_mut(4).take(shape.nodes as usize) {
        poll(cancellation)?;
        if node[0] == 1 {
            node[3] = node[1];
        } else {
            node[3] = output;
            output = output
                .checked_add(1)
                .ok_or_else(|| capacity("formula output address overflows"))?;
        }
    }
    if output != shape.variables {
        return Err(capacity("formula output coverage differs"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/formula/preparation.rs"]
mod tests;
