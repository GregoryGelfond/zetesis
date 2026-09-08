//! Batch-owned buffers: reuse capacity, replace every active input and delta.

use zetesis_cpu::lazy;

use crate::runtime::{self, Runtime};

use super::{Capacity, Plan, UNIFORM_BYTES};

pub(super) struct Transport {
    pub(super) capacity: Capacity,
    uniform: wgpu::Buffer,
    inputs: [wgpu::Buffer; 4],
    group: wgpu::BindGroup,
    output: wgpu::Buffer,
    pub(super) readback: wgpu::Buffer,
}

impl Transport {
    pub(super) fn new(runtime: &Runtime, capacity: Capacity) -> Self {
        let device = &runtime.device;
        let uniform = runtime::buffer(
            device,
            "lazy dimensions",
            UNIFORM_BYTES,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let labels = [
            "lazy source offsets",
            "lazy source instances",
            "lazy immutable snapshots",
            "lazy frozen seeds",
        ];
        let inputs = std::array::from_fn(|index| {
            runtime::buffer(
                device,
                labels[index],
                capacity.inputs[index],
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            )
        });
        let output = runtime::buffer(
            device,
            "lazy head delta",
            capacity.result,
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        );
        let readback = runtime::buffer(
            device,
            "lazy delta readback",
            capacity.result,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lazy round chunk"),
            layout: &runtime.pipeline.get_bind_group_layout(0),
            entries: &[
                runtime::entry(0, &uniform),
                runtime::entry(1, &inputs[0]),
                runtime::entry(2, &inputs[1]),
                runtime::entry(3, &inputs[2]),
                runtime::entry(4, &inputs[3]),
                runtime::entry(5, &output),
            ],
        });
        Self {
            capacity,
            uniform,
            inputs,
            group,
            output,
            readback,
        }
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
            .queue
            .write_buffer(&self.uniform, 0, bytemuck::cast_slice(&dimensions));
        for (buffer, words) in self.inputs.iter().zip([
            chunk.offsets(),
            chunk.records(),
            chunk.snapshots(),
            chunk.seeds(),
        ]) {
            runtime
                .queue
                .write_buffer(buffer, 0, bytemuck::cast_slice(words));
        }
        let mut encoder = runtime
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lazy source consequence command"),
            });
        // The shader ORs deltas. No active output word may retain an earlier
        // chunk's consequences, constraint flag, world identity or epoch.
        // Queue writes precede this command; clear precedes compute, then copy.
        encoder.clear_buffer(&self.output, 0, Some(plan.result_bytes));
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("lazy world consequences"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&runtime.pipeline);
            pass.set_bind_group(0, &self.group, &[]);
            pass.dispatch_workgroups(plan.dimensions[2], 1, 1);
        }
        encoder.copy_buffer_to_buffer(&self.output, 0, &self.readback, 0, plan.result_bytes);
        runtime.queue.submit(Some(encoder.finish()))
    }
}
