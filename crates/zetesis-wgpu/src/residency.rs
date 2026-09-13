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
        runtime: &runtime::Runtime,
        packed: &PackedSeeds<'_>,
        plan: &BatchPlan,
        timeout: Duration,
        control: &zetesis_cpu::Control,
    ) -> Result<runtime::Completion<Vec<GpuCheck>>, GpuError> {
        let device = runtime.device();
        let queue = runtime.queue();
        let pipeline = &runtime.pipeline;
        if self.transport.is_none() {
            self.transport = Some(Transport::new(device, pipeline, self, packed, plan));
        }
        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing batch transport"))?;
        queue.write_buffer(&transport.params, 0, bytemuck::cast_slice(&packed.params));
        queue.write_buffer(&transport.seeds, 0, bytemuck::cast_slice(&packed.seeds));
        let submission = runtime::submit(
            device,
            queue,
            &Dispatch {
                command_label: "static reduct batch",
                pass_label: "independent candidate worlds",
                pipeline,
                group: &transport.group,
                workgroups: [plan.world_count, 1, 1],
                result: &transport.result,
                readback: &transport.readback,
                result_bytes: plan.result_bytes,
            },
        );
        Ok(runtime::read_polled(
            device,
            &transport.readback,
            submission,
            timeout,
            || control.poll().map_err(GpuError::interrupted),
            |words| packing::decode(words, plan, packed),
        ))
    }
}

struct Transport {
    world_count: u32,
    params: wgpu::Buffer,
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
        packed: &PackedSeeds<'_>,
        plan: &BatchPlan,
    ) -> Self {
        // Dimensions remain fixed for this transport shape; every dispatch
        // rewrites its checked epoch as well as the candidate bits.
        let params = initialized(
            device,
            "parameters",
            &packed.params,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
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
            params,
            seeds,
            result,
            readback,
            group,
        }
    }
}
