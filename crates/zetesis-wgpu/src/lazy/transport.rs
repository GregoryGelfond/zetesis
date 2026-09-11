//! Batch-owned buffers: reuse capacity, replace every active input and delta.

use zetesis_cpu::lazy;

use crate::runtime::{self, Runtime};

use super::plan::{Retention, Selection};
use super::{Capacity, Plan, UNIFORM_BYTES};

pub(super) struct Transport {
    pub(super) capacity: Capacity,
    buffers: Buffers<wgpu::Buffer>,
    group: wgpu::BindGroup,
}

/// Seven owned handles. The generic parameter permits allocation/drop ordering
/// controls without a device; production stores only wgpu buffers.
struct Buffers<T> {
    uniform: T,
    inputs: [T; 4],
    output: T,
    readback: T,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Uniform,
    Input(usize),
    Output,
    Readback,
}

impl<T> Buffers<T> {
    fn retain(self, retention: Retention) -> Buffers<Option<T>> {
        let mut indices = 0..4;
        Buffers {
            uniform: retention.uniform.then_some(self.uniform),
            inputs: self.inputs.map(|buffer| {
                retention.inputs[indices.next().expect("four input positions")].then_some(buffer)
            }),
            output: retention.result.then_some(self.output),
            readback: retention.result.then_some(self.readback),
        }
    }
}

impl<T> Buffers<Option<T>> {
    fn empty() -> Self {
        Self {
            uniform: None,
            inputs: std::array::from_fn(|_| None),
            output: None,
            readback: None,
        }
    }

    fn complete(self, mut allocate: impl FnMut(Slot) -> T) -> Buffers<T> {
        let mut indices = 0..4;
        Buffers {
            uniform: self.uniform.unwrap_or_else(|| allocate(Slot::Uniform)),
            inputs: self.inputs.map(|buffer| {
                let index = indices.next().expect("four input positions");
                buffer.unwrap_or_else(|| allocate(Slot::Input(index)))
            }),
            output: self.output.unwrap_or_else(|| allocate(Slot::Output)),
            readback: self.readback.unwrap_or_else(|| allocate(Slot::Readback)),
        }
    }
}

/// Bind groups retain buffer handles too. Release that owner first, then all
/// rejected handles, before `complete` can allocate any replacement payload.
fn release<T, G>(group: G, buffers: Buffers<T>, retention: Retention) -> Buffers<Option<T>> {
    drop(group);
    buffers.retain(retention)
}

fn allocate(runtime: &Runtime, capacity: Capacity, slot: Slot) -> wgpu::Buffer {
    let (label, size, usage) = match slot {
        Slot::Uniform => (
            "lazy dimensions",
            UNIFORM_BYTES,
            wgpu::BufferUsages::UNIFORM,
        ),
        Slot::Input(index) => (
            [
                "lazy source offsets",
                "lazy source instances",
                "lazy immutable snapshots",
                "lazy frozen seeds",
            ][index],
            capacity.inputs[index],
            wgpu::BufferUsages::STORAGE,
        ),
        Slot::Output => (
            "lazy head delta",
            capacity.result,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        ),
        Slot::Readback => (
            "lazy delta readback",
            capacity.result,
            wgpu::BufferUsages::MAP_READ,
        ),
    };
    runtime::buffer(
        runtime.device(),
        label,
        size,
        usage | wgpu::BufferUsages::COPY_DST,
    )
}

impl Transport {
    pub(super) fn new(runtime: &Runtime, capacity: Capacity) -> Self {
        let buffers = Buffers::empty().complete(|slot| allocate(runtime, capacity, slot));
        Self::from_buffers(runtime, capacity, buffers)
    }

    pub(super) fn replace(self, runtime: &Runtime, selection: Selection) -> Self {
        let retained = release(self.group, self.buffers, selection.retention);
        let buffers = retained.complete(|slot| allocate(runtime, selection.capacity, slot));
        Self::from_buffers(runtime, selection.capacity, buffers)
    }

    fn from_buffers(runtime: &Runtime, capacity: Capacity, buffers: Buffers<wgpu::Buffer>) -> Self {
        let group = runtime
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("lazy round chunk"),
                layout: &runtime.pipeline.get_bind_group_layout(0),
                entries: &[
                    runtime::entry(0, &buffers.uniform),
                    runtime::entry(1, &buffers.inputs[0]),
                    runtime::entry(2, &buffers.inputs[1]),
                    runtime::entry(3, &buffers.inputs[2]),
                    runtime::entry(4, &buffers.inputs[3]),
                    runtime::entry(5, &buffers.output),
                ],
            });
        Self {
            capacity,
            buffers,
            group,
        }
    }

    pub(super) fn readback(&self) -> &wgpu::Buffer {
        &self.buffers.readback
    }

    #[cfg(test)]
    pub(super) fn buffers(&self) -> [&wgpu::Buffer; 7] {
        [
            &self.buffers.uniform,
            &self.buffers.inputs[0],
            &self.buffers.inputs[1],
            &self.buffers.inputs[2],
            &self.buffers.inputs[3],
            &self.buffers.output,
            &self.buffers.readback,
        ]
    }

    pub(super) fn submit(
        &self,
        runtime: &Runtime,
        chunk: &lazy::Chunk<'_>,
        plan: &Plan,
    ) -> wgpu::SubmissionIndex {
        let dimensions = [
            plan.dimensions[0],
            plan.dimensions[1],
            plan.dimensions[2],
            plan.dimensions[3],
            plan.epoch,
            0,
            0,
            0,
        ];
        runtime
            .queue()
            .write_buffer(&self.buffers.uniform, 0, bytemuck::cast_slice(&dimensions));
        for (buffer, words) in self.buffers.inputs.iter().zip([
            chunk.offsets(),
            chunk.records(),
            chunk.snapshots(),
            chunk.seeds(),
        ]) {
            runtime
                .queue()
                .write_buffer(buffer, 0, bytemuck::cast_slice(words));
        }
        let mut encoder =
            runtime
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("lazy source consequence command"),
                });
        // The shader ORs deltas. No active output word may retain an earlier
        // chunk's consequences, constraint flag, world identity or epoch.
        // Queue writes precede this command; clear precedes compute, then copy.
        encoder.clear_buffer(&self.buffers.output, 0, Some(plan.result_bytes));
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("lazy world consequences"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&runtime.pipeline);
            pass.set_bind_group(0, &self.group, &[]);
            pass.dispatch_workgroups(plan.dimensions[2], 1, 1);
        }
        encoder.copy_buffer_to_buffer(
            &self.buffers.output,
            0,
            &self.buffers.readback,
            0,
            plan.result_bytes,
        );
        runtime.queue().submit(Some(encoder.finish()))
    }
}

#[cfg(test)]
#[path = "../../tests/lazy/buffers.rs"]
mod tests;
