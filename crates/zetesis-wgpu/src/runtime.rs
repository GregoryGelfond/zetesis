//! Shared real-device lifecycle for the distinct static and formula profiles.
//! Profile capability checks, shader identity and result decoding remain explicit.

use crate::context::Effects;
use crate::{GpuContext, GpuError, GpuErrorKind, GpuOptions, GpuSelection};
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
}

// Compiled immutable resources only. Formula profiles share this owner while
// keeping residency, epochs and result accounting in each fresh oracle.
pub(crate) struct Runtime {
    pub(crate) context: GpuContext,
    pub(crate) pipeline: wgpu::ComputePipeline,
}

impl Runtime {
    pub(crate) async fn new(
        options: GpuOptions,
        selection: GpuSelection,
        profile: DeviceProfile,
    ) -> Result<Self, GpuError> {
        // Independent constructors retain both early advertised-limit validation
        // and the granted-limit check before shader construction.
        let context = GpuContext::create(
            options,
            selection,
            profile.device_label,
            profile.validate_limits,
        )
        .await?;
        let _lease = context.lease()?;
        Self::compile(&context, profile).await
    }

    pub(crate) async fn from_context(
        context: &GpuContext,
        profile: DeviceProfile,
    ) -> Result<Self, GpuError> {
        let _lease = context.lease()?;
        context.check_health()?;
        (profile.validate_limits)(context.limits())?;
        Self::compile(context, profile).await
    }

    // Both entry points retain their context lease through all scopes. The
    // independent path already checked advertised and granted profile limits.
    async fn compile(context: &GpuContext, profile: DeviceProfile) -> Result<Self, GpuError> {
        let device = context.device();
        let scopes = ErrorScopes::new(device);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(profile.shader_label),
            source: wgpu::ShaderSource::Wgsl(profile.shader),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(profile.pipeline_label),
            layout: None,
            module: &shader,
            entry_point: Some(profile.entry_point),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        context.complete(scopes.finish().await, Ok(()), Effects::MayBeLive)?;
        Ok(Self {
            context: context.clone(),
            pipeline,
        })
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
        let Completion { outcome, effects } = outcome.unwrap_or_else(|error| Completion {
            outcome: Err(error),
            effects: Effects::MayBeLive,
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
}
impl<T> Completion<T> {
    pub(crate) fn is_ok(&self) -> bool {
        self.outcome.is_ok()
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
}

pub(crate) fn submit(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    dispatch: &Dispatch<'_>,
) -> wgpu::SubmissionIndex {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some(dispatch.command_label),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(dispatch.pass_label),
            timestamp_writes: None,
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
    queue.submit([encoder.finish()])
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
    mut control: impl FnMut() -> Result<(), GpuError>,
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
        &mut control,
        |wait| {
            device.poll(wgpu::PollType::Wait {
                submission_index: submission.clone(),
                timeout: Some(wait),
            })
        },
        || started.elapsed(),
    );
    finish_read(
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
        &mut control,
    )
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
    mut control: impl FnMut() -> Result<(), GpuError>,
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
            control()?;
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
    }
}

// The production wait loop; injected poll/elapsed operations let controls cover
// timing boundaries deterministically without invoking a driver or sleeping.
fn wait_for_submission(
    timeout: Duration,
    mut control: impl FnMut() -> Result<(), GpuError>,
    mut poll: impl FnMut(Duration) -> Result<wgpu::PollStatus, wgpu::PollError>,
    mut elapsed: impl FnMut() -> Duration,
) -> SubmissionWait {
    loop {
        if let Err(error) = control() {
            return SubmissionWait {
                outcome: Err(error),
                completed: false,
            };
        }
        let remaining = timeout.saturating_sub(elapsed());
        match poll(remaining.min(Duration::from_millis(50))) {
            Ok(status) if status.wait_finished() => {
                return SubmissionWait {
                    outcome: control(),
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
#[path = "../tests/context/wait.rs"]
mod wait_tests;
