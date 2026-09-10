//! Immutable group residency and exact mask/output transport.

use super::{
    AggregateGpuActivity, AggregateGpuReduction, PARAM_BYTES, capacity,
    packing::{self, Plan},
    poll,
    preparation::Numeric,
};
use crate::{
    GpuError,
    runtime::{self, Runtime},
};
use std::{sync::Arc, time::Duration};
use zetesis_cpu::Control;

pub(super) struct Resident {
    pub(super) numeric: Arc<Numeric>,
    tuples: wgpu::Buffer,
    guards: wgpu::Buffer,
    pub(super) transport: Option<Transport>,
}
impl Resident {
    pub(super) fn new(runtime: &Runtime, numeric: Arc<Numeric>) -> Self {
        Self {
            tuples: runtime::initialized(
                &runtime.device,
                "aggregate contributions",
                &numeric.tuples,
                wgpu::BufferUsages::STORAGE,
            ),
            guards: runtime::initialized(
                &runtime.device,
                "aggregate guards",
                &numeric.guards,
                wgpu::BufferUsages::STORAGE,
            ),
            numeric,
            transport: None,
        }
    }
    pub(super) fn bytes(&self) -> Result<u64, GpuError> {
        self.numeric
            .bytes
            .checked_mul(2)
            .and_then(|bytes| {
                bytes.checked_add(self.transport.as_ref().map_or(0, |value| value.bytes))
            })
            .ok_or_else(|| capacity("aggregate incoming residency bytes overflow"))
    }
    pub(super) fn matches_count(&self, count: usize) -> bool {
        self.transport
            .as_ref()
            .is_some_and(|value| value.worlds as usize == count)
    }

    pub(super) fn dispatch(
        &mut self,
        runtime: &Runtime,
        masks: &[u32],
        plan: &Plan,
        timeout: Duration,
        control: &Control,
        activity: &mut AggregateGpuActivity,
    ) -> Result<Vec<AggregateGpuReduction>, GpuError> {
        let uploaded = activity
            .uploaded_bytes
            .checked_add(PARAM_BYTES)
            .and_then(|bytes| bytes.checked_add(plan.masks))
            .ok_or_else(|| capacity("aggregate upload accounting overflow"))?;
        if self.transport.is_none() {
            self.transport = Some(Transport::new(runtime, self, plan));
        }
        let transport = self
            .transport
            .as_ref()
            .expect("aggregate transport allocated");
        poll(control)?;
        runtime.queue.write_buffer(
            &transport.params,
            0,
            bytemuck::cast_slice(&plan.params(&self.numeric)),
        );
        runtime
            .queue
            .write_buffer(&transport.masks, 0, bytemuck::cast_slice(masks));
        activity.uploaded_bytes = uploaded;
        let submission = runtime::submit(
            &runtime.device,
            &runtime.queue,
            &runtime::Dispatch {
                command_label: "aggregate batch",
                pass_label: "original and frozen reductions",
                pipeline: &runtime.pipeline,
                group: &transport.group,
                workgroups: [plan.worlds, 1, 1],
                result: &transport.output,
                readback: &transport.readback,
                result_bytes: plan.results,
            },
        );
        activity.submissions = 1;
        activity.submitted_occurrences = u64::from(plan.worlds);
        activity.scheduled_work = plan.total_work;
        let result = runtime::read_polled(
            &runtime.device,
            &transport.readback,
            submission,
            timeout,
            || poll(control),
            |words| packing::decode(words, &self.numeric, plan, masks, control),
        );
        if result.is_ok() {
            activity.completed_occurrences = u64::from(plan.worlds);
            activity.completed_work = plan.total_work;
            activity.downloaded_bytes = plan.results;
        }
        result
    }
}

pub(super) struct Transport {
    worlds: u32,
    bytes: u64,
    params: wgpu::Buffer,
    masks: wgpu::Buffer,
    output: wgpu::Buffer,
    readback: wgpu::Buffer,
    group: wgpu::BindGroup,
}
impl Transport {
    fn new(runtime: &Runtime, resident: &Resident, plan: &Plan) -> Self {
        let params = runtime::buffer(
            &runtime.device,
            "aggregate dimensions",
            PARAM_BYTES,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let masks = runtime::buffer(
            &runtime.device,
            "aggregate eligibility",
            plan.masks,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        let output = runtime::buffer(
            &runtime.device,
            "aggregate measures",
            plan.results,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        );
        let readback = runtime::buffer(
            &runtime.device,
            "aggregate readback",
            plan.results,
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        );
        let group = runtime
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("numeric aggregate batch"),
                layout: &runtime.pipeline.get_bind_group_layout(0),
                entries: &[
                    runtime::entry(0, &params),
                    runtime::entry(1, &resident.tuples),
                    runtime::entry(2, &resident.guards),
                    runtime::entry(3, &masks),
                    runtime::entry(4, &output),
                ],
            });
        Self {
            worlds: plan.worlds,
            bytes: plan.transport,
            params,
            masks,
            output,
            readback,
            group,
        }
    }
}
