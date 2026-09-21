//! Exact-shape resident storage and the shared bounded device lifecycle.

use super::packing::{self, Graph, PARAM_BYTES, Packed, Plan};
use super::{TightGpuActivity, TightGpuCheck, poll};
use crate::runtime::{self, Dispatch, buffer, entry, initialized};
use crate::{GpuError, GpuErrorKind};
use std::time::Duration;
use zetesis_cpu::Cancellation;

pub(super) struct Resident {
    pub(super) graph: Graph,
    nodes: wgpu::Buffer,
    roots: wgpu::Buffer,
    producers: wgpu::Buffer,
    pub(super) transport: Option<Transport>,
}
impl Resident {
    pub(super) fn new(device: &wgpu::Device, graph: Graph, packed: &Packed) -> Self {
        Self {
            graph,
            nodes: initialized(
                device,
                "tight nodes",
                &packed.nodes,
                wgpu::BufferUsages::STORAGE,
            ),
            roots: initialized(
                device,
                "tight roots",
                &packed.roots,
                wgpu::BufferUsages::STORAGE,
            ),
            producers: initialized(
                device,
                "tight producers",
                &packed.producers,
                wgpu::BufferUsages::STORAGE,
            ),
            transport: None,
        }
    }
    pub(super) fn matches_count(&self, candidates: usize) -> bool {
        self.transport
            .as_ref()
            .is_some_and(|t| t.worlds as usize == candidates)
    }

    pub(super) fn dispatch(
        &mut self,
        execution: &Execution<'_>,
        seeds: &[u32],
        plan: &Plan,
        activity: &mut TightGpuActivity,
    ) -> Result<runtime::Completion<Vec<TightGpuCheck>>, GpuError> {
        let Execution {
            device,
            queue,
            pipeline,
            timeout,
            cancellation,
        } = *execution;
        if self.transport.is_none() {
            self.transport = Some(Transport::new(device, pipeline, self, plan));
        }
        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing tight transport"))?;
        poll(cancellation)?;
        queue.write_buffer(
            &transport.params,
            0,
            bytemuck::cast_slice(&plan.params(&self.graph)),
        );
        queue.write_buffer(&transport.seeds, 0, bytemuck::cast_slice(seeds));
        activity.uploaded_bytes += PARAM_BYTES + plan.seeds;
        let submission = runtime::submit(
            device,
            queue,
            &Dispatch {
                command_label: "ranked support checking",
                pass_label: "original truth and producer support",
                pipeline,
                group: &transport.group,
                workgroups: [plan.worlds, 1, 1],
                result: &transport.results,
                readback: &transport.readback,
                result_bytes: plan.results,
            },
        );
        activity.submissions = 1;
        activity.submitted_candidates = u64::from(plan.worlds);
        activity.scheduled_work = u64::from(plan.worlds) * u64::from(plan.work);
        let result = runtime::read_polled(
            device,
            &transport.readback,
            submission,
            timeout,
            || poll(cancellation),
            |words| packing::decode(words, &self.graph, plan, seeds, cancellation),
        );
        if result.is_ok() {
            activity.completed_candidates = u64::from(plan.worlds);
            activity.completed_work = activity.scheduled_work;
            activity.downloaded_bytes = plan.results;
        }
        Ok(result)
    }
}

pub(super) struct Execution<'a> {
    pub(super) device: &'a wgpu::Device,
    pub(super) queue: &'a wgpu::Queue,
    pub(super) pipeline: &'a wgpu::ComputePipeline,
    pub(super) timeout: Duration,
    pub(super) cancellation: &'a Cancellation,
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
            "tight parameters",
            PARAM_BYTES,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let seeds = buffer(
            device,
            "tight candidates",
            plan.seeds,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        let truth = buffer(
            device,
            "original truth",
            plan.truth,
            wgpu::BufferUsages::STORAGE,
        );
        let support = buffer(
            device,
            "producer support",
            plan.support,
            wgpu::BufferUsages::STORAGE,
        );
        let results = buffer(
            device,
            "tight outcomes",
            plan.results,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        );
        let readback = buffer(
            device,
            "tight readback",
            plan.results,
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        );
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("resident tight support"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                entry(0, &params),
                entry(1, &resident.nodes),
                entry(2, &resident.roots),
                entry(3, &resident.producers),
                entry(4, &seeds),
                entry(5, &truth),
                entry(6, &support),
                entry(7, &results),
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
