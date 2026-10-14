//! Cold graph preparation. Depth words exist only between the two named packing
//! phases; only finalized buffers and Graph can cross into device residency.

use super::FormulaLimits;
use super::packing::{Graph, Plan, Schedule, Shape, capacity, vector};
use crate::formula_graph::dependency;
#[cfg(test)]
use crate::formula_graph::dependency::insert_position;
use crate::{GpuError, GpuErrorKind};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::Theory;

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
    #[cfg(test)]
    pub(super) fn new(
        theory: &Theory,
        worlds: usize,
        limits: FormulaLimits,
        device: &wgpu::Limits,
        epoch: u32,
    ) -> Result<Self, GpuError> {
        Self::with_control(
            theory,
            worlds,
            limits,
            device,
            epoch,
            &Cancellation::default(),
        )
    }

    pub(super) fn with_control(
        theory: &Theory,
        worlds: usize,
        limits: FormulaLimits,
        device: &wgpu::Limits,
        epoch: u32,
        cancellation: &Cancellation,
    ) -> Result<Self, GpuError> {
        let shape = Shape::with_control(theory, device, limits.max_batch_bytes, cancellation)?;
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
        crate::formula_graph::pack(
            &self.shape.theory,
            &self.shape.rows,
            &mut nodes,
            cancellation,
        )?;
        self.shape.rows.release();
        let depth_count = dependency::label_depths(
            self.shape.nodes,
            self.shape.wide_words,
            &mut nodes,
            cancellation,
        )?;
        let levels = dependency::parallel_levels(self.shape.nodes, depth_count);
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
        dependency::pack(
            &self.shape.theory,
            self.shape.nodes,
            schedule.levels,
            &nodes,
            &mut roots,
            cancellation,
        )?;
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
mod tests;
