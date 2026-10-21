//! Exactly sized resident formula buffers and bounded submit/readback lifecycle.

use super::FormulaCheck;
use super::packing::{self, Graph, PARAM_BYTES, Plan};
use super::preparation::PreparedGraph;
use crate::runtime::{self, Dispatch, buffer, entry, initialized};
use crate::{GpuError, GpuErrorKind};
use std::time::Duration;

/// A submitted batch. Its logical checks are published only after shared scope
/// settlement; its readback tail and host window are for the oracle's timer.
pub(super) struct Dispatched {
    pub(super) completion: runtime::Completion<Vec<FormulaCheck>>,
    /// Candidates submitted, whether or not the readback decodes.
    pub(super) candidates: usize,
    pub(super) timing: crate::timing::BatchTiming,
}

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
        self.matches_count(plan.worlds as usize)
    }
    pub(super) fn matches_count(&self, count: usize) -> bool {
        self.transport
            .as_ref()
            .is_some_and(|transport| transport.worlds as usize == count)
    }
    pub(super) fn dispatch(
        &mut self,
        runtime: &runtime::Runtime,
        seeds: &[u32],
        plan: &Plan,
        timeout: Duration,
        cancellation: &zetesis_cpu::Cancellation,
        timer: Option<&crate::timing::ComputeTimer>,
    ) -> Result<Dispatched, GpuError> {
        let device = runtime.device();
        let queue = runtime.queue();
        let pipeline = runtime.pipeline_for(self.graph.workgroup_size)?;
        if self.transport.is_none() {
            self.transport = Some(Transport::new(device, pipeline, self, plan)?);
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
                clear_result: false,
                timing: timer,
            },
        );
        let mut timing = crate::timing::BatchTiming::default();
        let completion = runtime::read_polled(
            device,
            &transport.readback,
            submission.index,
            timeout,
            || cancellation.poll().map_err(GpuError::interrupted),
            |words| {
                let (words, tail) = crate::timing::split_readback(words, transport.timed)?;
                let checks = packing::decode(words, plan)?;
                timing.tail = tail;
                Ok(checks)
            },
        );
        timing.window = completion.window_since(submission.at);
        Ok(Dispatched {
            completion,
            candidates: plan.worlds as usize,
            timing,
        })
    }
}
pub(super) struct Transport {
    worlds: u32,
    params: wgpu::Buffer,
    seeds: wgpu::Buffer,
    results: wgpu::Buffer,
    readback: wgpu::Buffer,
    group: wgpu::BindGroup,
    // The readback carries a timestamp tail after the logical results.
    timed: bool,
}
impl Transport {
    fn new(
        device: &wgpu::Device,
        pipeline: &wgpu::ComputePipeline,
        resident: &Resident,
        plan: &Plan,
    ) -> Result<Self, GpuError> {
        let timed = device.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let readback_bytes =
            crate::timing::readback_size(plan.results, timed, device.limits().max_buffer_size)?;
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
            readback_bytes,
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
        Ok(Self {
            worlds: plan.worlds,
            params,
            seeds,
            results,
            readback,
            group,
            timed,
        })
    }
}
