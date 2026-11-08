//! Exact-shape resident storage and the shared bounded device lifecycle.

use super::packing::{self, Decoded, Graph, PARAM_BYTES, Packed, Plan};
use super::update::{self, Built, Stage};
use super::{TightGpuActivity, poll};
use crate::runtime::{self, Dispatch, Observation, Variant, buffer, entry, initialized};
use crate::{GpuError, GpuErrorKind};
use std::time::Duration;
use zetesis_algebra::{Cancellation, words::Words};

pub(super) struct Resident {
    pub(super) graph: Graph,
    nodes: wgpu::Buffer,
    roots: wgpu::Buffer,
    producers: wgpu::Buffer,
    /// Counts producer-buffer replacements, so a transport bound to an older
    /// buffer rebinds before its next dispatch.
    producers_generation: u64,
    /// The update route's admission for this theory.
    pub(super) updates: Stage,
    /// The runtime guard, which belongs to the theory beside its stage.
    pub(super) guard: update::Guard,
    pub(super) transport: Option<Transport>,
}
impl Resident {
    pub(super) fn new(device: &wgpu::Device, graph: Graph, packed: &Packed) -> Self {
        Self {
            updates: Stage::of(&graph),
            guard: update::Guard::new(),
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
            producers_generation: 0,
            graph,
            transport: None,
        }
    }
    pub(super) fn matches_count(&self, candidates: usize) -> bool {
        self.transport
            .as_ref()
            .is_some_and(|t| t.worlds as usize == candidates)
    }

    pub(super) fn original_truth_valid(&self) -> bool {
        self.transport
            .as_ref()
            .is_some_and(|transport| transport.original_truth_valid)
    }

    pub(super) fn invalidate_original_truth(&mut self) {
        if let Some(transport) = self.transport.as_mut() {
            transport.original_truth_valid = false;
        }
    }

    /// Called only after decode, context health and the final control poll all
    /// succeeded. An earlier successful readback alone is insufficient.
    pub(super) fn complete_original_truth(&mut self) {
        if let Some(transport) = self.transport.as_mut() {
            transport.original_truth_valid = true;
        }
    }

    /// Replace the producer buffer with the records and the readers index, and
    /// account the index as resident theory storage. The owner column is no
    /// longer needed and is released. Returns the bytes uploaded.
    pub(super) fn install_index(&mut self, device: &wgpu::Device, built: &Built) -> u64 {
        let bytes = u64::try_from(built.words.len()).unwrap_or(u64::MAX) * 4;
        self.producers = initialized(
            device,
            "tight producers and readers",
            &built.words,
            wgpu::BufferUsages::STORAGE,
        );
        self.producers_generation += 1;
        let graph = &mut self.graph;
        graph.bytes = graph.bytes - graph.producer_bytes + bytes;
        graph.producer_bytes = bytes;
        graph.owners = Vec::new();
        graph.owner_bytes = 0;
        self.updates = Stage::Built(built.index);
        bytes
    }

    /// Write and submit the batch, and request its readback's mapping. The
    /// submission is in flight when this returns; [`Self::finish`] waits for
    /// it. An error leaves nothing in flight.
    pub(super) fn submit(
        &mut self,
        execution: &Execution<'_>,
        call: &Call<'_>,
        activity: &mut TightGpuActivity,
    ) -> Result<Submitted, GpuError> {
        let Execution {
            device,
            queue,
            pipeline,
            variant,
            cancellation,
            timer,
            ..
        } = *execution;
        let Call { seeds, plan } = *call;
        let bindings = Bindings {
            nodes: &self.nodes,
            roots: &self.roots,
            producers: &self.producers,
            generation: self.producers_generation,
        };
        match self.transport.as_mut() {
            Some(transport) => transport.conform(device, pipeline, variant, &bindings, plan),
            None => {
                self.transport = Some(Transport::new(device, pipeline, variant, &bindings, plan)?);
            }
        }
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing tight transport"))?;
        // Consume the host authority before the first queue write. Every error
        // after this point leaves retained truth unavailable to the next call.
        // Clearing the result hints also clears each slot's seeded record.
        let retained = std::mem::take(&mut transport.original_truth_valid);
        let clear_result = !retained || !plan.original_truth_reused;
        poll(cancellation)?;
        queue.write_buffer(
            &transport.params,
            0,
            bytemuck::cast_slice(&plan.params(&self.graph)),
        );
        queue.write_buffer(&transport.seeds, 0, bytemuck::cast_slice(&seeds));
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
                clear_result,
                timing: timer,
            },
        );
        activity.submissions = 1;
        activity.submitted_candidates = u64::from(plan.worlds);
        activity.scheduled_work = u64::from(plan.worlds) * u64::from(plan.allowance);
        Ok(Submitted {
            pending: runtime::request_read(&transport.readback, submission.index),
            at: submission.at,
        })
    }

    /// Wait for the submitted batch, observing the control as `observation`
    /// says, then decode its readback. The transport is the one `submit` used:
    /// nothing replaces it within one check.
    pub(super) fn finish(
        &self,
        execution: &Execution<'_>,
        submitted: Submitted,
        observation: Observation,
        call: &Call<'_>,
        activity: &mut TightGpuActivity,
        timing: &mut crate::timing::BatchTiming,
    ) -> Result<runtime::Completion<Decoded>, GpuError> {
        let Execution {
            device,
            timeout,
            cancellation,
            ..
        } = *execution;
        let Call { seeds, plan } = *call;
        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing tight transport"))?;
        let mut performed = 0;
        let result = runtime::finish_pending(
            device,
            &transport.readback,
            submitted.pending,
            timeout,
            observation,
            || poll(cancellation),
            |words| {
                let (words, tail) = crate::timing::split_readback(words, transport.timed)?;
                let decoded = packing::decode(words, &self.graph, plan, &seeds, cancellation)?;
                performed = decoded.work;
                timing.tail = tail;
                Ok(decoded)
            },
        );
        timing.window = result.window_since(submitted.at);
        if result.is_ok() {
            activity.completed_candidates = u64::from(plan.worlds);
            activity.completed_work = performed;
            activity.downloaded_bytes = plan.results
                + if transport.timed {
                    crate::timing::READBACK_BYTES
                } else {
                    0
                };
        }
        Ok(result)
    }
}

/// One batch's host side: the candidate seeds it uploads and its plan.
#[derive(Clone, Copy)]
pub(super) struct Call<'a> {
    pub(super) seeds: Words<u32, &'a [u32]>,
    pub(super) plan: &'a Plan,
}

/// A submitted batch awaiting its finish: its pending readback and the host
/// instant just before its submission, from which its window is measured.
pub(super) struct Submitted {
    pending: runtime::PendingRead,
    at: std::time::Instant,
}

pub(super) struct Execution<'a> {
    pub(super) device: &'a wgpu::Device,
    pub(super) queue: &'a wgpu::Queue,
    pub(super) pipeline: &'a wgpu::ComputePipeline,
    /// The kernel `pipeline` compiles: the dense twin binds no update state.
    pub(super) variant: Variant,
    pub(super) timeout: Duration,
    pub(super) cancellation: &'a Cancellation,
    /// The oracle's timer, present under enabled timing.
    pub(super) timer: Option<&'a crate::timing::ComputeTimer>,
}

/// The resident theory's buffers a transport binds.
struct Bindings<'a> {
    nodes: &'a wgpu::Buffer,
    roots: &'a wgpu::Buffer,
    producers: &'a wgpu::Buffer,
    generation: u64,
}

pub(super) struct Transport {
    worlds: u32,
    params: wgpu::Buffer,
    seeds: wgpu::Buffer,
    truth: wgpu::Buffer,
    support: wgpu::Buffer,
    results: wgpu::Buffer,
    readback: wgpu::Buffer,
    /// The per-slot update state, when the plan holds one: bound by the full
    /// kernel, and kept allocated but unbound by the dense twin when a limit or
    /// the runtime guard declines the route of a batch that admitted it.
    state: Option<wgpu::Buffer>,
    /// The state's bytes; zero without state.
    state_bytes: u64,
    /// The kernel `group` binds for: the full kernel's layout has binding 8,
    /// the dense twin's does not.
    variant: Variant,
    group: wgpu::BindGroup,
    /// The producer buffer generation `group` binds.
    generation: u64,
    // The readback carries a timestamp tail after the logical results.
    timed: bool,
    original_truth_valid: bool,
}
impl Transport {
    fn new(
        device: &wgpu::Device,
        pipeline: &wgpu::ComputePipeline,
        variant: Variant,
        bindings: &Bindings<'_>,
        plan: &Plan,
    ) -> Result<Self, GpuError> {
        let timed = device.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let readback_bytes =
            crate::timing::readback_size(plan.results, timed, device.limits().max_buffer_size)?;
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
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        );
        let readback = buffer(
            device,
            "tight readback",
            readback_bytes,
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        );
        let state = state_buffer(device, plan.state);
        let group = bind(
            device,
            pipeline,
            bindings,
            [&params, &seeds, &truth, &support, &results],
            variant,
            state.as_ref(),
        );
        Ok(Self {
            worlds: plan.worlds,
            params,
            seeds,
            truth,
            support,
            results,
            readback,
            state,
            state_bytes: plan.state,
            variant,
            group,
            generation: bindings.generation,
            timed,
            original_truth_valid: false,
        })
    }

    /// Bring a retained transport to `plan`'s state, `variant`'s kernel and the
    /// resident's current producer buffer, keeping its truth and result
    /// authority. A state is added only where none was held, so the previous
    /// records, written by the kernel without the route, say no slot is seeded;
    /// a dropped state takes its retained evaluations with it, and this batch,
    /// on that kernel, records every slot unseeded. A batch that keeps its
    /// state while a limit or the runtime guard declines its route moves to the
    /// kernel without the route and leaves the state allocated, unbound; every
    /// record it writes says unseeded, so the state's contents bind no later
    /// batch. Each change of state or kernel rebinds.
    fn conform(
        &mut self,
        device: &wgpu::Device,
        pipeline: &wgpu::ComputePipeline,
        variant: Variant,
        bindings: &Bindings<'_>,
        plan: &Plan,
    ) {
        let kernel_changed = self.variant != variant;
        let state_changed = self.state_bytes != plan.state;
        if state_changed {
            self.state = state_buffer(device, plan.state);
            self.state_bytes = plan.state;
        }
        if state_changed || kernel_changed || self.generation != bindings.generation {
            self.variant = variant;
            self.group = bind(
                device,
                pipeline,
                bindings,
                [
                    &self.params,
                    &self.seeds,
                    &self.truth,
                    &self.support,
                    &self.results,
                ],
                variant,
                self.state.as_ref(),
            );
            self.generation = bindings.generation;
        }
    }
}

/// The update state of `bytes`, or none for a plan without state. Device
/// buffers start zeroed, so every slot starts unseeded.
fn state_buffer(device: &wgpu::Device, bytes: u64) -> Option<wgpu::Buffer> {
    (bytes != 0).then(|| {
        buffer(
            device,
            "tight update state",
            bytes,
            wgpu::BufferUsages::STORAGE,
        )
    })
}

/// The resident's and the transport's buffers as `pipeline`'s layout binds
/// them: binding 8 holds `state` for the full kernel, which runs only with the
/// route allowed and so with the state, and the dense twin's layout has no
/// binding 8, whatever state the transport keeps.
fn bind(
    device: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
    bindings: &Bindings<'_>,
    [params, seeds, truth, support, results]: [&wgpu::Buffer; 5],
    variant: Variant,
    state: Option<&wgpu::Buffer>,
) -> wgpu::BindGroup {
    let mut entries = vec![
        entry(0, params),
        entry(1, bindings.nodes),
        entry(2, bindings.roots),
        entry(3, bindings.producers),
        entry(4, seeds),
        entry(5, truth),
        entry(6, support),
        entry(7, results),
    ];
    if let (Variant::Full, Some(state)) = (variant, state) {
        entries.push(entry(8, state));
    }
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("resident tight support"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &entries,
    })
}
