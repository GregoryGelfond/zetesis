//! Shared real-device lifecycle for the distinct static and formula profiles.
//! Profile capability checks, shader identity and result decoding remain explicit.

use crate::{GpuError, GpuErrorKind, GpuInfo, GpuOptions, GpuSelection, selection};
use std::sync::{Arc, mpsc};
use std::time::Duration;
use wgpu::util::DeviceExt as _;

pub(crate) struct DeviceProfile {
    pub(crate) device_label: &'static str,
    pub(crate) shader_label: &'static str,
    pub(crate) pipeline_label: &'static str,
    pub(crate) shader: &'static str,
    pub(crate) entry_point: &'static str,
    pub(crate) validate_limits: fn(&wgpu::Limits) -> Result<(), GpuError>,
}

pub(crate) struct Runtime {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) pipeline: wgpu::ComputePipeline,
    pub(crate) info: GpuInfo,
    pub(crate) limits: wgpu::Limits,
    faults: Faults,
}

impl Runtime {
    pub(crate) async fn new(
        options: GpuOptions,
        selection: GpuSelection,
        profile: DeviceProfile,
    ) -> Result<Self, GpuError> {
        let (adapter, info) = selection::select_adapter(options, selection).await?;
        let limits = adapter.limits();
        (profile.validate_limits)(&limits)?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some(profile.device_label),
                required_limits: limits,
                ..Default::default()
            })
            .await
            .map_err(|error| GpuError::new(GpuErrorKind::Device, error.to_string()))?;
        // Unrequested features can reduce granted limits. Both profiles use
        // these granted limits for their subsequent packing, never advertisements.
        let limits = device.limits();
        (profile.validate_limits)(&limits)?;
        let faults = Faults::register(&device);
        let scopes = ErrorScopes::new(&device);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(profile.shader_label),
            source: wgpu::ShaderSource::Wgsl(profile.shader.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(profile.pipeline_label),
            layout: None,
            module: &shader,
            entry_point: Some(profile.entry_point),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        scopes.finish().await?;
        let mut runtime = Self {
            device,
            queue,
            pipeline,
            info,
            limits,
            faults,
        };
        runtime.check_health()?;
        Ok(runtime)
    }

    pub(crate) fn check_health(&mut self) -> Result<(), GpuError> {
        self.faults.check()
    }

    // Drain every scope, then inspect asynchronous health even if execution
    // already failed. Precedence and permanent invalidation are shared.
    pub(crate) fn complete<T>(
        &mut self,
        scopes: ErrorScopes,
        outcome: Result<T, GpuError>,
    ) -> Result<T, GpuError> {
        let validation = pollster::block_on(scopes.finish());
        self.faults.complete(validation, outcome)
    }
}

struct Faults {
    receiver: mpsc::Receiver<String>,
    invalidated: bool,
}
impl Faults {
    fn register(device: &wgpu::Device) -> Self {
        // Only the first fault is needed. A full bounded channel cannot recover
        // the device; callbacks therefore never block or wait for a shared lock.
        let (sender, receiver) = mpsc::sync_channel(1);
        let lost = sender.clone();
        device.set_device_lost_callback(move |reason, message| {
            let _ = lost.try_send(format!("device lost ({reason:?}): {message}"));
        });
        device.on_uncaptured_error(Arc::new(move |error| {
            let _ = sender.try_send(format!("uncaptured device error: {error}"));
        }));
        Self {
            receiver,
            invalidated: false,
        }
    }
    fn check(&mut self) -> Result<(), GpuError> {
        if self.invalidated {
            return Err(GpuError::new(
                GpuErrorKind::Device,
                "this oracle was invalidated by an earlier execution failure",
            ));
        }
        if let Ok(detail) = self.receiver.try_recv() {
            self.invalidated = true;
            return Err(GpuError::new(GpuErrorKind::Device, detail));
        }
        Ok(())
    }
    fn complete<T>(
        &mut self,
        validation: Result<(), GpuError>,
        outcome: Result<T, GpuError>,
    ) -> Result<T, GpuError> {
        let health = self.check();
        let result = validation.and(health).and(outcome);
        if result.is_err() {
            self.invalidated = true;
        }
        result
    }
}

pub(crate) struct Dispatch<'a> {
    pub(crate) command_label: &'static str,
    pub(crate) pass_label: &'static str,
    pub(crate) pipeline: &'a wgpu::ComputePipeline,
    pub(crate) group: &'a wgpu::BindGroup,
    pub(crate) worlds: u32,
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
        pass.dispatch_workgroups(dispatch.worlds, 1, 1);
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

// The decoder owns its output; mapped bytes cannot escape this lifecycle.
// Both successful decoding and a decode/range error release the mapping.
pub(crate) fn read<T>(
    device: &wgpu::Device,
    readback: &wgpu::Buffer,
    submission: wgpu::SubmissionIndex,
    timeout: Duration,
    decode: impl FnOnce(&[u32]) -> Result<T, GpuError>,
) -> Result<T, GpuError> {
    let (sender, receiver) = mpsc::sync_channel(1);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |mapped| {
            let _ = sender.try_send(mapped);
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(timeout),
        })
        .map_err(|error| {
            GpuError::new(
                if matches!(error, wgpu::PollError::Timeout) {
                    GpuErrorKind::Timeout
                } else {
                    GpuErrorKind::Device
                },
                error.to_string(),
            )
        })?;
    receiver
        .try_recv()
        .map_err(|error| {
            GpuError::new(
                GpuErrorKind::Readback,
                format!("mapping callback absent after completed poll: {error}"),
            )
        })?
        .map_err(|error| GpuError::new(GpuErrorKind::Readback, error.to_string()))?;
    let outcome = readback
        .slice(..)
        .get_mapped_range()
        .map_err(|error| GpuError::new(GpuErrorKind::Readback, error.to_string()))
        .and_then(|mapped| {
            bytemuck::try_cast_slice::<u8, u32>(&mapped)
                .map_err(|error| GpuError::new(GpuErrorKind::Readback, error.to_string()))
                .and_then(decode)
        });
    readback.unmap();
    outcome
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
#[path = "../tests/runtime/state.rs"]
mod tests;
