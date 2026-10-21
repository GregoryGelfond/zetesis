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
    pub(crate) validate_limits: fn(&wgpu::Limits) -> Result<(), GpuError>,
    pub(crate) kernel: crate::kernel::Kernel,
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
    /// A fixed formula or tight kernel's size and its pipeline, compiled when
    /// the kernel is created.
    Fixed(crate::GpuWorkgroupSize, wgpu::ComputePipeline),
    /// A fitted formula or tight kernel, whose sizes are compiled at first need.
    Fitted(Fitted),
}

/// A fitted kernel's shader module, created and validated with the kernel,
/// and one write-once slot for each fitted size the context grants. A slot is
/// written by the first check that dispatches its size, under the context
/// lease, which is exclusive across every primitive on the context; reads
/// take no lock.
struct Fitted {
    shader: wgpu::ShaderModule,
    entry_point: &'static str,
    label: &'static str,
    slots: Vec<(
        crate::GpuWorkgroupSize,
        std::sync::OnceLock<wgpu::ComputePipeline>,
    )>,
    /// Test seam: the invocation count compiled for the next first need,
    /// standing in for a compilation the device refuses.
    #[cfg(test)]
    refused: std::sync::Mutex<Option<u32>>,
}

impl Fitted {
    fn slot(
        &self,
        size: crate::GpuWorkgroupSize,
    ) -> Result<&std::sync::OnceLock<wgpu::ComputePipeline>, GpuError> {
        self.slots
            .iter()
            .find(|(fitted, _)| *fitted == size)
            .map(|(_, slot)| slot)
            .ok_or_else(|| unavailable(size))
    }
}

/// A size this runtime cannot provide: a static kernel, another fixed size, or
/// a fitted size the context does not grant or has not compiled.
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
            Some(crate::GpuWorkgroupSizing::Fixed(size)) => {
                Pipelines::Fixed(size, compile(Some(size.invocations())))
            }
            Some(sizing @ crate::GpuWorkgroupSizing::Fitted) => Pipelines::Fitted(Fitted {
                slots: crate::kernel::candidate_sizes(sizing, context.limits())
                    .map(|size| (size, std::sync::OnceLock::new()))
                    .collect(),
                shader,
                entry_point: profile.entry_point,
                label: profile.pipeline_label,
                #[cfg(test)]
                refused: std::sync::Mutex::new(None),
            }),
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
            Pipelines::Fixed(..) | Pipelines::Fitted(_) => Err(GpuError::new(
                GpuErrorKind::Validation,
                "this kernel's pipelines are chosen by workgroup size",
            )),
        }
    }

    /// Make the pipeline of `size` available to the check about to dispatch
    /// it. A fixed kernel's own size, and a fitted size already compiled,
    /// return at once. Otherwise the fitted size is compiled now, inside its own
    /// error scopes settled as at construction, and its slot written. The
    /// caller holds the context lease, has finished preparing its batch, and has
    /// neither opened its dispatch scopes nor replaced any residency, so a
    /// failure leaves the oracle as it was.
    ///
    /// # Errors
    /// A size this runtime cannot provide is a validation failure. A
    /// compilation failure is the scope's typed error and invalidates the
    /// context, as the same failure at construction did; the slot stays
    /// unwritten and is never retried.
    pub(crate) fn ensure_pipeline(&self, size: crate::GpuWorkgroupSize) -> Result<(), GpuError> {
        match &self.pipelines {
            Pipelines::Fixed(compiled, _) if *compiled == size => Ok(()),
            Pipelines::Fitted(fitted) => {
                let slot = fitted.slot(size)?;
                if slot.get().is_some() {
                    return Ok(());
                }
                #[cfg(test)]
                let invocations = fitted
                    .refused
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take()
                    .unwrap_or(size.invocations());
                #[cfg(not(test))]
                let invocations = size.invocations();
                let pipeline = self.compile_fitted(fitted, invocations)?;
                // Every write happens under the context lease, so the slot is
                // still empty here; a lost race could only store an equal pipeline.
                let _ = slot.set(pipeline);
                Ok(())
            }
            Pipelines::Static(_) | Pipelines::Fixed(..) => Err(unavailable(size)),
        }
    }

    /// Compile `fitted` with `invocations` per workgroup, settling its own
    /// error scopes through the context's completion, as construction does.
    fn compile_fitted(
        &self,
        fitted: &Fitted,
        invocations: u32,
    ) -> Result<wgpu::ComputePipeline, GpuError> {
        let device = self.device();
        let scopes = ErrorScopes::new(device);
        let pipeline = create_pipeline(
            device,
            &fitted.shader,
            fitted.entry_point,
            fitted.label,
            Some(invocations),
        );
        self.context.complete(
            pollster::block_on(scopes.finish()),
            Ok(()),
            Effects::MayBeLive,
        )?;
        Ok(pipeline)
    }

    /// The pipeline of `size`, compiled at construction for a fixed kernel or
    /// by `ensure_pipeline` for a fitted one.
    ///
    /// # Errors
    /// A size this runtime has not compiled, including any size of a static
    /// kernel, is a validation failure.
    pub(crate) fn pipeline_for(
        &self,
        size: crate::GpuWorkgroupSize,
    ) -> Result<&wgpu::ComputePipeline, GpuError> {
        match &self.pipelines {
            Pipelines::Fixed(compiled, pipeline) if *compiled == size => Ok(pipeline),
            Pipelines::Fitted(fitted) => fitted.slot(size)?.get().ok_or_else(|| unavailable(size)),
            Pipelines::Static(_) | Pipelines::Fixed(..) => Err(unavailable(size)),
        }
    }

    /// The sizes whose pipelines exist: a fixed kernel's size, or a fitted
    /// kernel's written slots, ascending.
    #[cfg(test)]
    pub(crate) fn compiled_sizes(&self) -> Vec<crate::GpuWorkgroupSize> {
        match &self.pipelines {
            Pipelines::Static(_) => Vec::new(),
            Pipelines::Fixed(size, _) => vec![*size],
            Pipelines::Fitted(fitted) => fitted
                .slots
                .iter()
                .filter(|(_, slot)| slot.get().is_some())
                .map(|(size, _)| *size)
                .collect(),
        }
    }

    /// Make the next first-need compilation use `invocations` per workgroup,
    /// standing in for a compilation the device refuses.
    #[cfg(test)]
    pub(crate) fn refuse_next_compilation(&self, invocations: u32) {
        if let Pipelines::Fitted(fitted) = &self.pipelines {
            *fitted
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
        let Completion {
            outcome, effects, ..
        } = outcome.unwrap_or_else(|error| Completion {
            outcome: Err(error),
            effects: Effects::MayBeLive,
            observed: None,
        });
        self.context
            .complete(pollster::block_on(scopes.finish()), outcome, effects)
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
/// removing it. Every kernel satisfies the rule that makes `false` sound: each
/// workgroup location a kernel reads was written earlier in its workgroup, by
/// the same invocation or by another ordered before the read by a barrier or
/// uniform load, and every workgroup array index is within its array, since the
/// bounds policy would otherwise substitute some element, possibly one nothing
/// wrote (CONTRIBUTING, "Carry the semantic contract into Lean and Rust").
const ZERO_FILL_WORKGROUP_MEMORY: bool = true;

/// The compilation options of every pipeline in the crate: `constants` for the
/// overridable constants, and the workgroup zero-fill that
/// `ZERO_FILL_WORKGROUP_MEMORY` selects.
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
    mut cancellation: impl FnMut() -> Result<(), GpuError>,
    decode: impl FnOnce(&[u32]) -> Result<T, GpuError>,
) -> Completion<T> {
    let (sender, receiver) = mpsc::sync_channel(1);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |mapped| {
            let _ = sender.try_send(mapped);
        });
    let started = std::time::Instant::now();
    let submission = Some(submission);
    let wait = wait_for_submission(
        timeout,
        &mut cancellation,
        |wait| {
            device.poll(wgpu::PollType::Wait {
                submission_index: submission.clone(),
                timeout: Some(wait),
            })
        },
        || started.elapsed(),
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
