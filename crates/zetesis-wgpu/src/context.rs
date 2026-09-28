//! Device ownership shared by serialized primitive operations.

use crate::{GpuError, GpuErrorKind, GpuInfo, GpuOptions, GpuSelection, selection};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

/// One selected device, queue, granted capabilities and failure boundary.
///
/// Clones retain the same context. Primitives prepared from separate clones can
/// retain their own subjects simultaneously; each operation takes a nonblocking
/// exclusive execution lease. Overlap returns [`GpuErrorKind::Busy`] without
/// submitting work or invalidating the context. This is serialized composition,
/// not concurrent GPU scheduling.
///
/// Device loss, scope/readback failure or panic unwinding during an operation
/// invalidates every primitive using this context. No asynchronous submission
/// handle is exposed. Checked input/capacity refusals before device
/// execution leave it reusable. A fresh context is required after invalidation.
/// Relation preparation submits no queue work: cancellation after its mapped
/// host access is released leaves the context reusable only if scope settlement
/// and device health checks both succeed. Submitted cancellation/deadline stops
/// may also preserve reuse when queue completion, successful mapping and released
/// mapped access are established. An interrupted wait with unknown completion,
/// mapping failure or any other execution failure still invalidates the context.
/// Per-primitive byte limits retain their documented scope; they do not sum other
/// live primitives, device infrastructure or deferred driver retirement.
#[derive(Clone)]
pub struct GpuContext {
    resources: Arc<Resources>,
}

struct Resources {
    device: wgpu::Device,
    queue: wgpu::Queue,
    info: GpuInfo,
    limits: wgpu::Limits,
    lifecycle: Lifecycle,
}

/// Effect information at scope settlement, never inferred from an error kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Effects {
    /// No queue work was submitted and all mapped host access has been released.
    NoSubmission,
    /// Queue completion and successful mapping were observed, and every mapped
    /// view was dropped and the buffer unmapped before scope settlement.
    SubmittedAndReleased,
    /// A submission may remain live, including after an interrupted wait.
    MayBeLive,
}

impl GpuContext {
    /// Select a physical device of the requested API under the hard selection policy.
    ///
    /// Primitive constructors subsequently check their own profile against this
    /// device's granted limits. Construction is outside per-operation budgets.
    ///
    /// # Errors
    /// Returns adapter, policy or device creation failure; no CPU fallback occurs.
    pub fn new_selected(options: GpuOptions, selection: GpuSelection) -> Result<Self, GpuError> {
        let context = pollster::block_on(Self::create(
            options,
            selection,
            "zetesis shared device",
            |_| Ok(()),
        ))?;
        context.check_health()?;
        Ok(context)
    }

    /// Actual adapter metadata; observation performs no device operation.
    #[must_use]
    pub fn info(&self) -> &GpuInfo {
        &self.resources.info
    }

    /// Optional wgpu features granted at device creation.
    ///
    /// These can be a strict subset of [`GpuInfo::features`]. The current
    /// constructors request no optional features; reporting adapter support
    /// must not be mistaken for enabling an optimized kernel. Observation
    /// performs no submission and does not establish context health.
    #[must_use]
    pub fn features(&self) -> wgpu::Features {
        self.resources.device.features()
    }

    /// Limits granted to this device, distinct from a primitive's work budgets.
    ///
    /// Primitive constructors and operations validate their own requirements
    /// against these limits. Reading them performs no submission or allocation.
    #[must_use]
    pub fn limits(&self) -> &wgpu::Limits {
        &self.resources.limits
    }

    /// Check this device's reported identity against a caller's hard policy.
    ///
    /// Uses the same API and physical-category predicate as discovery. No adapter is rediscovered or substituted. This metadata-only
    /// operation does not acquire a lease, inspect device health or establish a
    /// primitive's capability requirements; its constructor still checks those.
    /// A matching context can therefore be busy, invalidated or unsuitable for
    /// the requested primitive. Success has fixed cost; a refusal formats detail.
    ///
    /// # Errors
    /// Returns [`GpuErrorKind::AdapterRefused`] when the retained device does not
    /// match the policy. Refusal leaves context health and residency unchanged.
    pub fn check_selection(
        &self,
        options: GpuOptions,
        selection: GpuSelection,
    ) -> Result<(), GpuError> {
        selection::check_selection(self.info(), options, selection)
    }

    /// Whether both handles retain the exact same device context.
    ///
    /// Equal adapter metadata does not imply shared context identity.
    #[must_use]
    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.resources, &other.resources)
    }

    pub(crate) async fn create(
        options: GpuOptions,
        selection: GpuSelection,
        label: &'static str,
        validate: fn(&wgpu::Limits) -> Result<(), GpuError>,
    ) -> Result<Self, GpuError> {
        let (adapter, info) = selection::select_adapter(options, selection, validate).await?;
        let limits = adapter.limits();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some(label),
                required_limits: limits,
                ..Default::default()
            })
            .await
            .map_err(|error| GpuError::new(GpuErrorKind::Device, error.to_string()))?;
        let limits = device.limits();
        validate(&limits)?;
        let faults = Faults::register(&device);
        Ok(Self {
            resources: Arc::new(Resources {
                device,
                queue,
                info,
                limits,
                lifecycle: Lifecycle::new(faults),
            }),
        })
    }

    pub(crate) fn device(&self) -> &wgpu::Device {
        &self.resources.device
    }
    pub(crate) fn queue(&self) -> &wgpu::Queue {
        &self.resources.queue
    }
    pub(crate) fn lease(&self) -> Result<Lease<'_>, GpuError> {
        self.resources.lifecycle.lease()
    }
    pub(crate) fn check_health(&self) -> Result<(), GpuError> {
        self.resources.lifecycle.check()
    }
    pub(crate) fn invalidate(&self) {
        self.resources.lifecycle.invalidate();
    }
    pub(crate) fn complete<T>(
        &self,
        validation: Result<(), GpuError>,
        outcome: Result<T, GpuError>,
        effects: Effects,
    ) -> Result<T, GpuError> {
        self.resources
            .lifecycle
            .complete(validation, outcome, effects)
    }
}

struct Lifecycle {
    active: AtomicBool,
    faults: Mutex<Faults>,
}

impl Lifecycle {
    fn new(faults: Faults) -> Self {
        Self {
            active: AtomicBool::new(false),
            faults: Mutex::new(faults),
        }
    }
    fn lease(&self) -> Result<Lease<'_>, GpuError> {
        self.active
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .map_err(|_| {
                GpuError::new(GpuErrorKind::Busy, "device context is already executing")
            })?;
        Ok(Lease { lifecycle: self })
    }
    fn check(&self) -> Result<(), GpuError> {
        self.faults.lock().map_err(|_| poisoned())?.check()
    }
    fn invalidate(&self) {
        // Poisoning is already permanent failure; never recover a poisoned state.
        if let Ok(mut faults) = self.faults.lock() {
            faults.invalidated = true;
        }
    }
    fn complete<T>(
        &self,
        validation: Result<(), GpuError>,
        outcome: Result<T, GpuError>,
        effects: Effects,
    ) -> Result<T, GpuError> {
        self.faults
            .lock()
            .map_err(|_| poisoned())?
            .complete(validation, outcome, effects)
    }
}

pub(crate) struct Lease<'a> {
    lifecycle: &'a Lifecycle,
}
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.lifecycle.invalidate();
        }
        self.lifecycle.active.store(false, Ordering::Release);
    }
}

fn poisoned() -> GpuError {
    GpuError::new(
        GpuErrorKind::Device,
        "device context health state was poisoned",
    )
}

struct Faults {
    receiver: mpsc::Receiver<String>,
    invalidated: bool,
}
impl Faults {
    fn register(device: &wgpu::Device) -> Self {
        // Callbacks only publish the first fault; they never wait for a lock.
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
                "this context was invalidated by an earlier execution failure",
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
        effects: Effects,
    ) -> Result<T, GpuError> {
        let health = self.check();
        let settled_interruption = effects != Effects::MayBeLive
            && validation.is_ok()
            && health.is_ok()
            && outcome.as_ref().is_err_and(|error| {
                matches!(
                    error.interruption,
                    Some(zetesis_cpu::Stop::Cancelled | zetesis_cpu::Stop::Deadline)
                )
            });
        let result = validation.and(health).and(outcome);
        if result.is_err() && !settled_interruption {
            self.invalidated = true;
        }
        result
    }
}

#[cfg(test)]
#[path = "../tests/context/state.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/context/control.rs"]
mod control_tests;
