//! Explicit device ownership reused across independent ordinary solves.

/// Caller-owned execution infrastructure, independent of any semantic subject.
///
/// The default retains no device and preserves ordinary per-session discovery.
/// With the `gpu` feature, `with_gpu` retains the supplied context and compiles
/// primitive pipelines per session. `with_formula_profile` also retains one
/// explicit formula compilation for repeated formula sessions. Every session
/// still owns fresh resident subjects, epochs, search state, budgets, pending
/// answers, counters and incumbents. CPU worker pools remain session-owned.
///
/// Resources do not select a solving policy: CPU and automatic execution ignore
/// the supplied device. An explicit device request uses this exact context or
/// fails; it never discovers a replacement. Resource ownership does not override
/// the session's selected backend.
///
/// Context operations are serialized through nonblocking leases. Busy refuses
/// an overlapping operation without poisoning the owner; device invalidation is
/// shared by every client. Per-primitive limits do not bound all simultaneously
/// live sessions or the context's driver storage. Cloning only retains ownership;
/// it does not copy device contents or provide concurrent GPU scheduling.
#[derive(Clone, Default)]
pub struct ExecutionResources {
    #[cfg(feature = "gpu")]
    gpu: Option<GpuResources>,
}

// The profile determines its context: callers cannot pair a compilation with
// another context, even one reporting identical adapter metadata.
#[cfg(feature = "gpu")]
#[derive(Clone)]
enum GpuResources {
    Context(zetesis_wgpu::GpuContext),
    Formula(zetesis_wgpu::GpuFormulaProfile),
}

impl ExecutionResources {
    /// Retain an existing device for later ordinary-session GPU attempts.
    ///
    /// This performs one shared-owner clone, with no discovery, health check,
    /// pipeline construction or submission. The supplied handle need not outlive
    /// this owner. Actual setup checks the requested hardware policy and primitive
    /// capability; it can refuse a foreign, busy or invalidated context.
    #[cfg(feature = "gpu")]
    #[must_use]
    pub fn with_gpu(context: &zetesis_wgpu::GpuContext) -> Self {
        Self {
            gpu: Some(GpuResources::Context(context.clone())),
        }
    }

    /// Retain a compiled formula pipeline for later ordinary formula sessions.
    ///
    /// The profile's exact context also serves other GPU primitives. This makes
    /// one shared-owner clone with no discovery, health check, compilation or
    /// submission. Each formula session validates policy, Busy, health and
    /// granted capabilities before starting independent residency and search.
    /// CPU and automatic formula policies continue to ignore these resources.
    #[cfg(feature = "gpu")]
    #[must_use]
    pub fn with_formula_profile(profile: &zetesis_wgpu::GpuFormulaProfile) -> Self {
        Self {
            gpu: Some(GpuResources::Formula(profile.clone())),
        }
    }

    /// The exact retained formula compilation, if explicitly supplied.
    ///
    /// Context-only resources do not implicitly compile or cache a profile.
    #[cfg(feature = "gpu")]
    #[must_use]
    pub const fn formula_profile(&self) -> Option<&zetesis_wgpu::GpuFormulaProfile> {
        match self.gpu.as_ref() {
            Some(GpuResources::Formula(profile)) => Some(profile),
            _ => None,
        }
    }

    /// The exact retained context, if the caller supplied one.
    ///
    /// Metadata and identity inspection do not establish device health. The
    /// returned borrow cannot outlive this owner; clone the context to retain it.
    #[cfg(feature = "gpu")]
    #[must_use]
    pub fn gpu_context(&self) -> Option<&zetesis_wgpu::GpuContext> {
        match self.gpu.as_ref() {
            Some(GpuResources::Context(context)) => Some(context),
            Some(GpuResources::Formula(profile)) => Some(profile.context()),
            None => None,
        }
    }

    #[cfg(feature = "gpu")]
    pub(crate) fn gpu_for(
        &self,
        backend: crate::Backend,
    ) -> Result<Option<&zetesis_wgpu::GpuContext>, zetesis_wgpu::GpuError> {
        if let Some(context) = self.gpu_context() {
            context.check_selection(
                zetesis_wgpu::GpuOptions::default(),
                crate::engine::selection(backend),
            )?;
        }
        Ok(self.gpu_context())
    }
}
