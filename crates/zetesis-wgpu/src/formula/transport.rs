//! Exactly sized resident formula buffers and bounded submit/readback lifecycle.

use super::FormulaCheck;
use super::packing::{self, Graph, PARAM_BYTES, Plan};
use super::preparation::PreparedGraph;
use crate::runtime::{self, Dispatch, buffer, entry, initialized};
use crate::{GpuError, GpuErrorKind};
use std::time::Duration;

pub(super) struct Resident {
    pub(super) graph: Graph,
    nodes: wgpu::Buffer,
    roots: wgpu::Buffer,
    pub(super) transport: Option<Transport>,
}
impl Resident {
    pub(super) fn new(device: &wgpu::Device, prepared: PreparedGraph) -> Self {
        Self {
            graph: prepared.graph,
            nodes: initialized(
                device,
                "formula nodes",
                &prepared.nodes,
                wgpu::BufferUsages::STORAGE,
            ),
            roots: initialized(
                device,
                "formula roots",
                &prepared.roots,
                wgpu::BufferUsages::STORAGE,
            ),
            transport: None,
        }
    }
    pub(super) fn matches(&self, plan: &Plan) -> bool {
        self.transport
            .as_ref()
            .is_some_and(|transport| transport.worlds == plan.worlds)
    }
    pub(super) fn dispatch(
        &mut self,
        runtime: &runtime::Runtime,
        seeds: &[u32],
        plan: &Plan,
        timeout: Duration,
        control: &zetesis_cpu::Control,
        submitted_candidates: &mut Option<usize>,
    ) -> Result<runtime::Completion<Vec<FormulaCheck>>, GpuError> {
        let device = runtime.device();
        let queue = runtime.queue();
        let pipeline = &runtime.pipeline;
        if self.transport.is_none() {
            self.transport = Some(Transport::new(device, pipeline, self, plan));
        }
        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing formula transport"))?;
        queue.write_buffer(
            &transport.params,
            0,
            bytemuck::cast_slice(&plan.params(&self.graph)),
        );
        queue.write_buffer(&transport.seeds, 0, bytemuck::cast_slice(seeds));
        let submission = runtime::submit(
            device,
            queue,
            &Dispatch {
                command_label: "frozen formula propagation",
                pass_label: "candidate-local formula sweeps",
                pipeline,
                group: &transport.group,
                workgroups: [plan.worlds, 1, 1],
                result: &transport.results,
                readback: &transport.readback,
                result_bytes: plan.results,
            },
        );
        *submitted_candidates = Some(plan.worlds as usize);
        Ok(runtime::read_polled(
            device,
            &transport.readback,
            submission,
            timeout,
            || control.poll().map_err(GpuError::interrupted),
            |words| packing::decode(words, plan),
        ))
    }
}
pub(super) struct Transport {
    worlds: u32,
    params: wgpu::Buffer,
    seeds: wgpu::Buffer,
    results: wgpu::Buffer,
    readback: wgpu::Buffer,
    group: wgpu::BindGroup,
}
impl Transport {
    fn new(
        device: &wgpu::Device,
        pipeline: &wgpu::ComputePipeline,
        resident: &Resident,
        plan: &Plan,
    ) -> Self {
        let params = buffer(
            device,
            "formula parameters",
            PARAM_BYTES,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let seeds = buffer(
            device,
            "formula candidates",
            plan.seeds,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        let masks = buffer(
            device,
            "frozen original truth",
            plan.masks,
            wgpu::BufferUsages::STORAGE,
        );
        let domains = buffer(
            device,
            "candidate-local Boolean domains",
            plan.domains,
            wgpu::BufferUsages::STORAGE,
        );
        let results = buffer(
            device,
            "formula outcomes",
            plan.results,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        );
        let readback = buffer(
            device,
            "formula readback",
            plan.results,
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        );
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("resident formula epoch"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                entry(0, &params),
                entry(1, &resident.nodes),
                entry(2, &resident.roots),
                entry(3, &seeds),
                entry(4, &masks),
                entry(5, &domains),
                entry(6, &results),
            ],
        });
        Self {
            worlds: plan.worlds,
            params,
            seeds,
            results,
            readback,
            group,
        }
    }
}
