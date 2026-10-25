//! Shared real-device lifecycle for the distinct static and formula profiles.
//! Profile capability checks, shader identity and result decoding remain explicit.

use crate::context::Effects;
use crate::{GpuContext, GpuDeviceSource, GpuError, GpuErrorKind};
use std::sync::mpsc;
use std::time::Duration;
use wgpu::util::DeviceExt as _;

pub(crate) struct DeviceProfile {
    pub(crate) device_label: &'static str,
    pub(crate) shader_label: &'static str,
    pub(crate) pipeline_label: &'static str,
    pub(crate) shader: std::borrow::Cow<'static, str>,
    pub(crate) entry_point: &'static str,
    /// The entry point of a sized kernel's dense twin, which evaluates without
    /// the update route and so binds no update state: the tight kernel's, for
    /// every batch whose update route is not allowed. `None` for every other
    /// kernel.
    pub(crate) dense_entry_point: Option<&'static str>,
    pub(crate) validate_limits: fn(&wgpu::Limits) -> Result<(), GpuError>,
    pub(crate) kernel: crate::kernel::Kernel,
}

/// Which of a sized kernel's entry points a pipeline compiles: the full
/// kernel, or its dense twin ([`DeviceProfile::dense_entry_point`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Variant {
    Full,
    Dense,
}

// Compiled immutable resources, plus a fitted kernel's write-once pipeline
// slots. Formula profiles share this owner while keeping residency, epochs and
// result accounting in each fresh oracle.
pub(crate) struct Runtime {
    pub(crate) context: GpuContext,
    pub(crate) kernel: crate::kernel::Kernel,
    pipelines: Pipelines,
}

/// A runtime's pipelines, by kernel kind.
enum Pipelines {
    /// A static kernel's one pipeline.
    Static(wgpu::ComputePipeline),
    /// A formula or tight kernel, compiled by workgroup size.
    Sized(Sized),
}

/// A sized kernel's shader module, created and validated with the kernel, and
/// one slot for each size it can dispatch: its fixed size, whose full pipeline
/// is compiled when the kernel is created, or each fitted size the context
/// grants. Every other pipeline is compiled by the first check that dispatches
/// it, under the context lease, which is exclusive across every primitive on
/// the context; reads take no lock.
struct Sized {
    shader: wgpu::ShaderModule,
    entry_point: &'static str,
    dense_entry_point: Option<&'static str>,
    label: &'static str,
    slots: Vec<(crate::GpuWorkgroupSize, Slot)>,
    /// Test seam: the invocation count compiled for the next first need,
    /// standing in for a compilation the device refuses.
    #[cfg(test)]
    refused: std::sync::Mutex<Option<u32>>,
}

/// One size's write-once pipelines, one per variant.
#[derive(Default)]
struct Slot {
    full: std::sync::OnceLock<wgpu::ComputePipeline>,
    dense: std::sync::OnceLock<wgpu::ComputePipeline>,
}

impl Slot {
    const fn of(&self, variant: Variant) -> &std::sync::OnceLock<wgpu::ComputePipeline> {
        match variant {
            Variant::Full => &self.full,
            Variant::Dense => &self.dense,
        }
    }
}

impl Sized {
    /// The slot of `size` and `variant`'s entry point, or a validation failure
    /// for a size the kernel cannot dispatch or a variant it lacks.
    fn slot(
        &self,
        size: crate::GpuWorkgroupSize,
        variant: Variant,
    ) -> Result<(&std::sync::OnceLock<wgpu::ComputePipeline>, &'static str), GpuError> {
        let entry_point = match variant {
            Variant::Full => self.entry_point,
            Variant::Dense => self.dense_entry_point.ok_or_else(|| unavailable(size))?,
        };
        self.slots
            .iter()
            .find(|(fitted, _)| *fitted == size)
            .map(|(_, slot)| (slot.of(variant), entry_point))
            .ok_or_else(|| unavailable(size))
    }
}

/// A size this runtime cannot provide: a static kernel, another fixed size, a
/// fitted size the context does not grant or has not compiled, or a dense
/// variant of a kernel without one.
fn unavailable(size: crate::GpuWorkgroupSize) -> GpuError {
    GpuError::new(
        GpuErrorKind::Validation,
        format!(
            "no {}-invocation pipeline was compiled for this kernel",
            size.invocations()
        ),
    )
}

impl Runtime {
    /// Open `profile`'s runtime on the device `source` provides.
    ///
    /// A selected or discovered source checks the profile's limits against the
    /// adapters' advertised limits during selection and against the new
    /// device's granted limits before any shader is created. A context source
    /// leases the context, then checks its health and the profile's limits
    /// against its granted limits. Only formula and tight kernels act on the
    /// source's timestamp request.
    pub(crate) async fn open(
        source: GpuDeviceSource<'_>,
        profile: DeviceProfile,
    ) -> Result<Self, GpuError> {
        let measures = profile.kernel.measures_compute_time();
        let context = match source {
            GpuDeviceSource::Selected {
                options,
                selection,
                timing,
            } => {
                GpuContext::create_with_timing(
                    options,
                    selection,
                    profile.device_label,
                    profile.validate_limits,
                    timing && measures,
                )
                .await?
            }
            GpuDeviceSource::Discovered {
                discovery,
                options,
                timing,
            } => {
                discovery
                    .context(
                        options,
                        profile.device_label,
                        profile.validate_limits,
                        timing && measures,
                    )
                    .await?
            }
            GpuDeviceSource::Context(context) => {
                let _lease = context.lease()?;
                context.check_health()?;
                (profile.validate_limits)(context.limits())?;
                return Self::compile(context, profile).await;
            }
        };
        let _lease = context.lease()?;
        Self::compile(&context, profile).await
    }

    // Every source retains its context lease through all scopes, and its
    // profile limits are already checked. A static or fixed kernel compiles its
    // one pipeline here; a fitted kernel creates and validates its shader
    // module and compiles each size at the first check that dispatches it
    // (`ensure_pipeline`).
    async fn compile(context: &GpuContext, profile: DeviceProfile) -> Result<Self, GpuError> {
        profile.kernel.validate(context.limits())?;
        let source = profile.kernel.source(profile.shader, context.features())?;
        let device = context.device();
        let scopes = ErrorScopes::new(device);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(profile.shader_label),
            source: wgpu::ShaderSource::Wgsl(source),
        });
        let compile = |invocations| {
            create_pipeline(
                device,
                &shader,
                profile.entry_point,
                profile.pipeline_label,
                invocations,
            )
        };
        let pipelines = match profile
            .kernel
            .options()
            .map(|options| options.workgroup_sizing)
        {
            None => Pipelines::Static(compile(None)),
            Some(sizing) => {
                let slots = match sizing {
                    crate::GpuWorkgroupSizing::Fixed(size) => {
                        let slot = Slot::default();
                        let _ = slot.full.set(compile(Some(size.invocations())));
                        vec![(size, slot)]
                    }
                    crate::GpuWorkgroupSizing::Fitted => {
                        crate::kernel::candidate_sizes(sizing, context.limits())
                            .map(|size| (size, Slot::default()))
                            .collect()
                    }
                };
                Pipelines::Sized(Sized {
                    slots,
                    shader,
                    entry_point: profile.entry_point,
                    dense_entry_point: profile.dense_entry_point,
                    label: profile.pipeline_label,
                    #[cfg(test)]
                    refused: std::sync::Mutex::new(None),
                })
            }
        };
        context.complete(scopes.finish().await, Ok(()), Effects::MayBeLive)?;
        Ok(Self {
            context: context.clone(),
            kernel: profile.kernel,
            pipelines,
        })
    }

    /// The pipeline of a static kernel, which has no workgroup sizing.
    ///
    /// # Errors
    /// A formula or tight kernel's pipelines are chosen by size: a validation
    /// failure.
    pub(crate) fn static_pipeline(&self) -> Result<&wgpu::ComputePipeline, GpuError> {
        match &self.pipelines {
            Pipelines::Static(pipeline) => Ok(pipeline),
            Pipelines::Sized(_) => Err(GpuError::new(
                GpuErrorKind::Validation,
                "this kernel's pipelines are chosen by workgroup size",
            )),
        }
    }

    /// Make the pipeline of `size` and `variant` available to the check about
    /// to dispatch it. A pipeline already compiled, a fixed kernel's full
    /// pipeline included, returns at once. Otherwise it is compiled now, inside
    /// its own error scopes settled as at construction, and its slot written.
    /// The caller holds the context lease, has finished preparing its batch and
    /// has not opened its dispatch scopes. A freshly prepared graph can need a
    /// size's first pipeline: for a tight theory the dense twin, since a fresh
    /// theory's first batch never allows the update route. A resident tight
    /// theory can need only the full kernel first, at its first batch allowing
    /// the route, and only under fitted sizing: a fixed size compiles its full
    /// pipeline at construction. A fresh graph's cold preparation has already
    /// released any previous resident; a resident theory stays, and its oracle
    /// invalidates its retained truth as after every failed call. Either way a
    /// failure leaves the slot unwritten and the context invalidated.
    ///
    /// # Errors
    /// A size or variant this runtime cannot provide is a validation failure. A
    /// compilation failure is the scope's typed error and invalidates the
    /// context, as the same failure at construction did; the slot stays
    /// unwritten and is never retried.
    pub(crate) fn ensure_pipeline(
        &self,
        size: crate::GpuWorkgroupSize,
        variant: Variant,
    ) -> Result<(), GpuError> {
        match &self.pipelines {
            Pipelines::Sized(sized) => {
                let (slot, entry_point) = sized.slot(size, variant)?;
                if slot.get().is_some() {
                    return Ok(());
                }
                #[cfg(test)]
                let invocations = sized
                    .refused
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take()
                    .unwrap_or(size.invocations());
                #[cfg(not(test))]
                let invocations = size.invocations();
                let pipeline = self.compile_sized(sized, entry_point, invocations)?;
                // Every write happens under the context lease, so the slot is
                // still empty here; a lost race could only store an equal pipeline.
                let _ = slot.set(pipeline);
                Ok(())
            }
            Pipelines::Static(_) => Err(unavailable(size)),
        }
    }

    /// Compile `sized`'s `entry_point` with `invocations` per workgroup,
    /// settling its own error scopes through the context's completion, as
    /// construction does.
    fn compile_sized(
        &self,
        sized: &Sized,
        entry_point: &'static str,
        invocations: u32,
    ) -> Result<wgpu::ComputePipeline, GpuError> {
        let device = self.device();
        let scopes = ErrorScopes::new(device);
        let pipeline = create_pipeline(
            device,
            &sized.shader,
            entry_point,
            sized.label,
            Some(invocations),
        );
        self.context.complete(
            pollster::block_on(scopes.finish()),
            Ok(()),
            Effects::MayBeLive,
        )?;
        Ok(pipeline)
    }

    /// The pipeline of `size` and `variant`, compiled at construction for a
    /// fixed kernel's full pipeline or by `ensure_pipeline` otherwise.
    ///
    /// # Errors
    /// A pipeline this runtime has not compiled, including any size of a
    /// static kernel, is a validation failure.
    pub(crate) fn pipeline_for(
        &self,
        size: crate::GpuWorkgroupSize,
        variant: Variant,
    ) -> Result<&wgpu::ComputePipeline, GpuError> {
        match &self.pipelines {
            Pipelines::Sized(sized) => sized
                .slot(size, variant)?
                .0
                .get()
                .ok_or_else(|| unavailable(size)),
            Pipelines::Static(_) => Err(unavailable(size)),
        }
    }

    /// The sizes with a compiled pipeline of either variant, ascending.
    #[cfg(test)]
    pub(crate) fn compiled_sizes(&self) -> Vec<crate::GpuWorkgroupSize> {
        self.compiled()
            .into_iter()
            .map(|(size, _)| size)
            .fold(Vec::new(), |mut sizes, size| {
                if sizes.last() != Some(&size) {
                    sizes.push(size);
                }
                sizes
            })
    }

    /// The compiled pipelines, by size ascending and then variant, full first.
    #[cfg(test)]
    pub(crate) fn compiled(&self) -> Vec<(crate::GpuWorkgroupSize, Variant)> {
        match &self.pipelines {
            Pipelines::Static(_) => Vec::new(),
            Pipelines::Sized(sized) => sized
                .slots
                .iter()
                .flat_map(|(size, slot)| {
                    [Variant::Full, Variant::Dense]
                        .into_iter()
                        .filter(|variant| slot.of(*variant).get().is_some())
                        .map(|variant| (*size, variant))
                })
                .collect(),
        }
    }

    /// Make the next first-need compilation use `invocations` per workgroup,
    /// standing in for a compilation the device refuses.
    #[cfg(test)]
    pub(crate) fn refuse_next_compilation(&self, invocations: u32) {
        if let Pipelines::Sized(sized) = &self.pipelines {
            *sized
                .refused
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(invocations);
        }
    }

    pub(crate) fn device(&self) -> &wgpu::Device {
        self.context.device()
    }
    pub(crate) fn queue(&self) -> &wgpu::Queue {
        self.context.queue()
    }
    pub(crate) fn limits(&self) -> &wgpu::Limits {
        self.context.limits()
    }
    pub(crate) fn check_health(&self) -> Result<(), GpuError> {
        self.context.check_health()
    }
    pub(crate) fn invalidate(&self) {
        self.context.invalidate();
    }

    // The caller retains its context lease through scope and health completion.
    pub(crate) fn complete<T>(
        &self,
        scopes: ErrorScopes,
        outcome: Result<Completion<T>, GpuError>,
    ) -> Result<T, GpuError> {
        self.complete_settled(pollster::block_on(scopes.finish()), outcome)
    }

    /// Complete an operation whose error scopes are in either state.
    pub(crate) fn complete_scopes<T>(
        &self,
        scopes: Scopes,
        outcome: Result<Completion<T>, GpuError>,
    ) -> Result<T, GpuError> {
        match scopes {
            Scopes::Open(scopes) => self.complete(scopes, outcome),
            Scopes::Settled(validation) => self.complete_settled(validation, outcome),
        }
    }

    /// Complete an operation whose error scopes were already popped, as an
    /// overlapping check pops them right after its submission, before the
    /// caller's work runs. `validation` is their settled outcome.
    fn complete_settled<T>(
        &self,
        validation: Result<(), GpuError>,
        outcome: Result<Completion<T>, GpuError>,
    ) -> Result<T, GpuError> {
        let Completion {
            outcome, effects, ..
        } = outcome.unwrap_or_else(|error| Completion {
            outcome: Err(error),
            effects: Effects::MayBeLive,
            observed: None,
        });
        self.context.complete(validation, outcome, effects)
    }

    /// For operations that submitted no queue work and released mapped access.
    /// Scopes are fully drained and health checked before a caller interruption
    /// may leave the context reusable. Other failures retain strict invalidation.
    pub(crate) fn complete_unsubmitted<T>(
        &self,
        scopes: ErrorScopes,
        outcome: Result<T, GpuError>,
    ) -> Result<T, GpuError> {
        self.context.complete(
            pollster::block_on(scopes.finish()),
            outcome,
            Effects::NoSubmission,
        )
    }
}

/// Readback outcome plus private evidence about released device effects.
/// Only the shared readback boundary can construct this envelope. Successful
/// queue polling alone does not establish mapping success or released access.
#[derive(Debug)]
pub(crate) struct Completion<T> {
    outcome: Result<T, GpuError>,
    effects: Effects,
    // The instant at which the wait observed the submission's completion.
    observed: Option<std::time::Instant>,
}
impl<T> Completion<T> {
    pub(crate) fn is_ok(&self) -> bool {
        self.outcome.is_ok()
    }

    /// The host window from `submitted` to the observed completion; absent when
    /// the wait did not observe completion.
    pub(crate) fn window_since(&self, submitted: std::time::Instant) -> Option<Duration> {
        self.observed
            .map(|observed| observed.saturating_duration_since(submitted))
    }
}

pub(crate) struct Dispatch<'a> {
    pub(crate) command_label: &'static str,
    pub(crate) pass_label: &'static str,
    pub(crate) pipeline: &'a wgpu::ComputePipeline,
    pub(crate) group: &'a wgpu::BindGroup,
    pub(crate) workgroups: [u32; 3],
    pub(crate) result: &'a wgpu::Buffer,
    pub(crate) readback: &'a wgpu::Buffer,
    pub(crate) result_bytes: u64,
    /// Clear prior receipt hints in this encoder before computing. The owning
    /// oracle, not bytes from a previous dispatch, authorizes retained truth.
    pub(crate) clear_result: bool,
    pub(crate) timing: Option<&'a crate::timing::ComputeTimer>,
}

/// A queue submission and the host instant just before it was made.
pub(crate) struct Submission {
    pub(crate) index: wgpu::SubmissionIndex,
    pub(crate) at: std::time::Instant,
}

/// A compute pipeline over `shader`'s entry point, overriding `WORKGROUP_SIZE`
/// with `invocations` when the kernel is compiled by size.
fn create_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    entry_point: &'static str,
    label: &'static str,
    invocations: Option<u32>,
) -> wgpu::ComputePipeline {
    let constants = invocations.map(|invocations| [("WORKGROUP_SIZE", f64::from(invocations))]);
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: None,
        module: shader,
        entry_point: Some(entry_point),
        compilation_options: compilation_options(
            constants.as_ref().map_or(&[], |values| values.as_slice()),
        ),
        cache: None,
    })
}

/// Whether every pipeline keeps WebGPU's zero-fill of workgroup memory.
///
/// It stays `true`, wgpu's default, until a measured kernel-time gain repays
/// removing it. Every kernel satisfies the rule that makes `false` sound, held
/// pending that measurement: each workgroup location a kernel reads was written
/// earlier in its workgroup, by the same invocation or by another ordered
/// before the read by a barrier or uniform load, and every workgroup array
/// index is within its array, since the bounds policy would otherwise
/// substitute some element, possibly one nothing wrote (CONTRIBUTING, "Carry
/// the semantic contract into Lean and Rust"). A decision that differs between
/// kernels belongs in a field of each kernel's `DeviceProfile`, which
/// `compilation_options` would then take in place of this constant.
const ZERO_FILL_WORKGROUP_MEMORY: bool = true;

/// The compilation options of every pipeline in the crate: `constants` for the
/// overridable constants, and WebGPU's zero-fill of workgroup memory, which
/// every pipeline keeps while `ZERO_FILL_WORKGROUP_MEMORY` is `true`.
fn compilation_options<'a>(
    constants: &'a [(&'a str, f64)],
) -> wgpu::PipelineCompilationOptions<'a> {
    wgpu::PipelineCompilationOptions {
        constants,
        zero_initialize_workgroup_memory: ZERO_FILL_WORKGROUP_MEMORY,
    }
}

pub(crate) fn submit(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    dispatch: &Dispatch<'_>,
) -> Submission {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some(dispatch.command_label),
    });
    if dispatch.clear_result {
        encoder.clear_buffer(dispatch.result, 0, Some(dispatch.result_bytes));
    }
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(dispatch.pass_label),
            timestamp_writes: dispatch.timing.map(crate::timing::ComputeTimer::writes),
        });
        pass.set_pipeline(dispatch.pipeline);
        pass.set_bind_group(0, dispatch.group, &[]);
        let [x, y, z] = dispatch.workgroups;
        pass.dispatch_workgroups(x, y, z);
    }
    encoder.copy_buffer_to_buffer(
        dispatch.result,
        0,
        dispatch.readback,
        0,
        dispatch.result_bytes,
    );
    if let Some(timing) = dispatch.timing {
        timing.resolve(&mut encoder, dispatch.readback, dispatch.result_bytes);
    }
    let command = encoder.finish();
    let at = std::time::Instant::now();
    Submission {
        index: queue.submit([command]),
        at,
    }
}

/// Admit a dispatch's wait policy before any device allocation or submission.
/// Empty operations that submit no work do not need a wait allowance.
pub(crate) fn positive_timeout(timeout: Duration) -> Result<(), GpuError> {
    if timeout.is_zero() {
        Err(GpuError::new(
            GpuErrorKind::Capacity,
            "the GPU wait timeout must be positive",
        ))
    } else {
        Ok(())
    }
}

// Poll control between bounded waits. Keep settled effects distinct from the
// caller outcome: cancellation is never converted to success for reuse. Resident
// buffer handles may remain owned until explicit release or owner destruction.
pub(crate) fn read_polled<T>(
    device: &wgpu::Device,
    readback: &wgpu::Buffer,
    submission: wgpu::SubmissionIndex,
    timeout: Duration,
    cancellation: impl FnMut() -> Result<(), GpuError>,
    decode: impl FnOnce(&[u32]) -> Result<T, GpuError>,
) -> Completion<T> {
    let pending = request_read(readback, submission);
    finish_pending(
        device,
        readback,
        pending,
        timeout,
        Observation::Polled,
        cancellation,
        decode,
    )
}

/// A readback whose mapping was requested at its submission, and whose wait
/// follows whatever the caller runs while the submission is in flight.
pub(crate) struct PendingRead {
    receiver: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
    submission: wgpu::SubmissionIndex,
    // The submission's timeout runs from here, just after the submit.
    requested: std::time::Instant,
}

/// Request `readback`'s mapping for `submission`. The mapping resolves when
/// the submission completes; [`finish_pending`] waits for it.
pub(crate) fn request_read(
    readback: &wgpu::Buffer,
    submission: wgpu::SubmissionIndex,
) -> PendingRead {
    let (sender, receiver) = mpsc::sync_channel(1);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |mapped| {
            let _ = sender.try_send(mapped);
        });
    PendingRead {
        receiver,
        submission,
        requested: std::time::Instant::now(),
    }
}

/// How a pending read's wait observes the caller's control.
pub(crate) enum Observation {
    /// Between bounded waits, as every synchronous read does.
    Polled,
    /// Not at all: the control fired while the caller's work ran beside the
    /// submission. The wait then only establishes the submission's completion,
    /// within its timeout, and the read reports this stop after it without
    /// decoding, so a completed and released readback leaves the context
    /// reusable.
    AfterStop(GpuError),
}

/// Wait for a pending read's submission, then map, decode and release its
/// readback, observing the control as `observation` says.
pub(crate) fn finish_pending<T>(
    device: &wgpu::Device,
    readback: &wgpu::Buffer,
    pending: PendingRead,
    timeout: Duration,
    observation: Observation,
    mut cancellation: impl FnMut() -> Result<(), GpuError>,
    decode: impl FnOnce(&[u32]) -> Result<T, GpuError>,
) -> Completion<T> {
    let PendingRead {
        receiver,
        submission,
        requested,
    } = pending;
    let submission = Some(submission);
    let wait = wait_observing(
        timeout,
        observation,
        &mut cancellation,
        |wait| {
            device.poll(wgpu::PollType::Wait {
                submission_index: submission.clone(),
                timeout: Some(wait),
            })
        },
        || requested.elapsed(),
    );
    let observed = wait.completed.then(std::time::Instant::now);
    let mut completion = finish_read(
        wait,
        || {
            receiver
                .try_recv()
                .map_err(|error| {
                    GpuError::new(
                        GpuErrorKind::Readback,
                        format!("mapping callback absent after completed poll: {error}"),
                    )
                })?
                .map_err(|error| GpuError::new(GpuErrorKind::Readback, error.to_string()))
        },
        || {
            readback
                .slice(..)
                .get_mapped_range()
                .map_err(|error| GpuError::new(GpuErrorKind::Readback, error.to_string()))
                .and_then(|mapped| {
                    bytemuck::try_cast_slice::<u8, u32>(&mapped)
                        .map_err(|error| GpuError::new(GpuErrorKind::Readback, error.to_string()))
                        .and_then(decode)
                })
        },
        // Also terminate a pending map after an interrupted wait. Unmapping
        // alone does not establish queue completion or permit context reuse.
        || readback.unmap(),
        &mut cancellation,
    );
    completion.observed = observed;
    completion
}

struct SubmissionWait {
    outcome: Result<(), GpuError>,
    completed: bool,
}

// The actual production cleanup sequence, factored so deterministic controls
// exercise mapping/decoding/control/release ordering without a physical device.
fn finish_read<T>(
    wait: SubmissionWait,
    mapping: impl FnOnce() -> Result<(), GpuError>,
    decode: impl FnOnce() -> Result<T, GpuError>,
    release: impl FnOnce(),
    mut cancellation: impl FnMut() -> Result<(), GpuError>,
) -> Completion<T> {
    // A completed wait can have returned a late control stop. Inspect its map
    // callback for health evidence, but preserve that original stop and skip
    // decoding. An absent/failed callback prevents reuse even in that case.
    let mapped = if wait.completed { mapping() } else { Ok(()) };
    let mapped_successfully = wait.completed && mapped.is_ok();
    let outcome = wait
        .outcome
        .and(mapped)
        .and_then(|()| decode())
        .and_then(|value| {
            cancellation()?;
            Ok(value)
        });
    release(); // Every mapped view has left the decode closure before this call.
    Completion {
        outcome,
        effects: if mapped_successfully {
            Effects::SubmittedAndReleased
        } else {
            Effects::MayBeLive
        },
        observed: None,
    }
}

// The wait's observation of the control, factored so deterministic controls
// cover it without a device. After a stop the caller's work observed, the wait
// polls no control: it reports that stop once completion is observed, which
// skips decoding, and otherwise the wait's own failure, which says why
// completion was not observed.
fn wait_observing(
    timeout: Duration,
    observation: Observation,
    cancellation: impl FnMut() -> Result<(), GpuError>,
    poll: impl FnMut(Duration) -> Result<wgpu::PollStatus, wgpu::PollError>,
    elapsed: impl FnMut() -> Duration,
) -> SubmissionWait {
    match observation {
        Observation::Polled => wait_for_submission(timeout, cancellation, poll, elapsed),
        Observation::AfterStop(stop) => {
            let wait = wait_for_submission(timeout, || Ok(()), poll, elapsed);
            SubmissionWait {
                outcome: if wait.completed {
                    Err(stop)
                } else {
                    wait.outcome
                },
                completed: wait.completed,
            }
        }
    }
}

// The production wait loop; injected poll/elapsed operations let controls cover
// timing boundaries deterministically without invoking a driver or sleeping.
fn wait_for_submission(
    timeout: Duration,
    mut cancellation: impl FnMut() -> Result<(), GpuError>,
    mut poll: impl FnMut(Duration) -> Result<wgpu::PollStatus, wgpu::PollError>,
    mut elapsed: impl FnMut() -> Duration,
) -> SubmissionWait {
    loop {
        if let Err(error) = cancellation() {
            return SubmissionWait {
                outcome: Err(error),
                completed: false,
            };
        }
        let remaining = timeout.saturating_sub(elapsed());
        match poll(remaining.min(Duration::from_millis(50))) {
            Ok(status) if status.wait_finished() => {
                return SubmissionWait {
                    outcome: cancellation(),
                    completed: true,
                };
            }
            Ok(_) => {
                return SubmissionWait {
                    outcome: Err(GpuError::new(
                        GpuErrorKind::Device,
                        "device wait did not establish submission completion",
                    )),
                    completed: false,
                };
            }
            Err(wgpu::PollError::Timeout) if elapsed() < timeout => {}
            Err(error) => {
                return SubmissionWait {
                    outcome: Err(GpuError::new(
                        if matches!(error, wgpu::PollError::Timeout) {
                            GpuErrorKind::Timeout
                        } else {
                            GpuErrorKind::Device
                        },
                        error.to_string(),
                    )),
                    completed: false,
                };
            }
        }
    }
}

pub(crate) fn buffer(
    device: &wgpu::Device,
    label: &str,
    size: u64,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage,
        mapped_at_creation: false,
    })
}
pub(crate) fn initialized(
    device: &wgpu::Device,
    label: &str,
    words: &[u32],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(words),
        usage,
    })
}
pub(crate) fn entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

/// An operation's error scopes: still open, as a synchronous operation keeps
/// them until its readback settles, or already popped with their outcome, as an
/// overlapping check pops them right after its submission.
pub(crate) enum Scopes {
    Open(ErrorScopes),
    Settled(Result<(), GpuError>),
}

impl Scopes {
    /// Pop open scopes now, keeping their outcome for the settlement.
    pub(crate) fn settle(&mut self) {
        if let Self::Open(_) = self
            && let Self::Open(scopes) = std::mem::replace(self, Self::Settled(Ok(())))
        {
            *self = Self::Settled(pollster::block_on(scopes.finish()));
        }
    }
}

pub(crate) struct ErrorScopes {
    // Field drop order is the reverse of the push order, including unwinding.
    allocation: wgpu::ErrorScopeGuard,
    internal: wgpu::ErrorScopeGuard,
    validation: wgpu::ErrorScopeGuard,
}

impl ErrorScopes {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        Self {
            validation: device.push_error_scope(wgpu::ErrorFilter::Validation),
            internal: device.push_error_scope(wgpu::ErrorFilter::Internal),
            allocation: device.push_error_scope(wgpu::ErrorFilter::OutOfMemory),
        }
    }

    pub(crate) async fn finish(self) -> Result<(), GpuError> {
        let mut failure = None;
        for scope in [self.allocation, self.internal, self.validation] {
            if let Some(error) = scope.pop().await {
                let kind = match error {
                    wgpu::Error::OutOfMemory { .. } => GpuErrorKind::Allocation,
                    wgpu::Error::Internal { .. } => GpuErrorKind::Device,
                    wgpu::Error::Validation { .. } => GpuErrorKind::Validation,
                };
                failure.get_or_insert_with(|| GpuError::new(kind, error.to_string()));
            }
        }
        failure.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests;
