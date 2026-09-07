//! Device resources for one immutable program and one exact transport shape.

use std::time::Duration;

use crate::packing::{self, BatchPlan, GraphPlan, PackedGraph, PackedSeeds};
use crate::runtime::{self, Dispatch, buffer, entry, initialized};
use crate::{GpuCheck, GpuError, GpuErrorKind};

pub(crate) struct ResidentGraph {
    pub(crate) plan: GraphPlan,
    rules: wgpu::Buffer,
    antecedents: wgpu::Buffer,
    carrier: wgpu::Buffer,
    transport: Option<Transport>,
}

impl ResidentGraph {
    pub(crate) fn new(device: &wgpu::Device, plan: GraphPlan, packed: &PackedGraph) -> Self {
        Self {
            plan,
            rules: initialized(
                device,
                "resident rules",
                &packed.rules,
                wgpu::BufferUsages::STORAGE,
            ),
            antecedents: initialized(
                device,
                "resident antecedents",
                &packed.antecedents,
                wgpu::BufferUsages::STORAGE,
            ),
            carrier: initialized(
                device,
                "resident carrier",
                &packed.carrier,
                wgpu::BufferUsages::STORAGE,
            ),
            transport: None,
        }
    }

    pub(crate) fn has_transport(&self, plan: &BatchPlan) -> bool {
        self.transport
            .as_ref()
            .is_some_and(|transport| transport.world_count == plan.world_count)
    }

    pub(crate) fn clear_transport(&mut self) {
        self.transport = None;
    }

    pub(crate) fn dispatch(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pipeline: &wgpu::ComputePipeline,
        packed: &PackedSeeds,
        plan: &BatchPlan,
        timeout: Duration,
    ) -> Result<Vec<GpuCheck>, GpuError> {
        if self.transport.is_none() {
            self.transport = Some(Transport::new(device, pipeline, self, packed, plan));
        }
        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing batch transport"))?;
        queue.write_buffer(&transport.seeds, 0, bytemuck::cast_slice(&packed.seeds));
        let submission = runtime::submit(
            device,
            queue,
            &Dispatch {
                command_label: "static reduct batch",
                pass_label: "independent candidate worlds",
                pipeline,
                group: &transport.group,
                worlds: plan.world_count,
                result: &transport.result,
                readback: &transport.readback,
                result_bytes: plan.result_bytes,
            },
        );
        runtime::read(device, &transport.readback, submission, timeout, |words| {
            packing::decode(words, plan)
        })
    }
}

struct Transport {
    world_count: u32,
    seeds: wgpu::Buffer,
    result: wgpu::Buffer,
    readback: wgpu::Buffer,
    group: wgpu::BindGroup,
}

impl Transport {
    fn new(
        device: &wgpu::Device,
        pipeline: &wgpu::ComputePipeline,
        graph: &ResidentGraph,
        packed: &PackedSeeds,
        plan: &BatchPlan,
    ) -> Self {
        // The group retains the parameter buffer, whose values are fixed for
        // this program and exact world count. Candidate bits alone are rewritten.
        let params = initialized(
            device,
            "parameters",
            &packed.params,
            wgpu::BufferUsages::UNIFORM,
        );
        let seeds = buffer(
            device,
            "frozen seeds",
            plan.seed_bytes,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        let result = buffer(
            device,
            "reduct closures and verdicts",
            plan.result_bytes,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        );
        let readback = buffer(
            device,
            "reduct readback",
            plan.result_bytes,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("resident static epoch"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                entry(0, &params),
                entry(1, &graph.rules),
                entry(2, &graph.antecedents),
                entry(3, &seeds),
                entry(4, &graph.carrier),
                entry(5, &result),
            ],
        });
        Self {
            world_count: plan.world_count,
            seeds,
            result,
            readback,
            group,
        }
    }
}
