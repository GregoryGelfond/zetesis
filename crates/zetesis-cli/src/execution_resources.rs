//! Explicit device ownership reused across independent ordinary solves.

/// Caller-owned execution infrastructure, independent of any semantic subject.
///
/// The default retains no device and preserves ordinary per-session discovery.
/// With the `gpu` feature, `with_gpu` retains the supplied context through
/// its shared owner. Each session still creates fresh primitive pipelines,
/// resident subjects, search state, budgets, pending answers and incumbents.
/// CPU worker pools remain session-owned.
///
/// Resources do not select a solving policy: CPU and automatic formula execution
/// ignore the supplied device. Automatic relational execution considers it only
/// at the existing delayed GPU-attempt boundary. A device attempt uses this exact
/// context or fails; it never discovers a replacement. The session's existing
/// automatic CPU fallback policy remains separate from resource ownership.
///
/// Context operations are serialized through nonblocking leases. Busy refuses
/// an overlapping operation without poisoning the owner; device invalidation is
/// shared by every client. Per-primitive limits do not bound all simultaneously
/// live sessions or the context's driver storage. Cloning only retains ownership;
/// it does not copy device contents or provide concurrent GPU scheduling.
#[derive(Clone, Default)]
pub struct ExecutionResources {
    #[cfg(feature = "gpu")]
    gpu: Option<zetesis_wgpu::GpuContext>,
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
            gpu: Some(context.clone()),
        }
    }

    /// The exact retained context, if the caller supplied one.
    ///
    /// Metadata and identity inspection do not establish device health. The
    /// returned borrow cannot outlive this owner; clone the context to retain it.
    #[cfg(feature = "gpu")]
    #[must_use]
    pub const fn gpu_context(&self) -> Option<&zetesis_wgpu::GpuContext> {
        self.gpu.as_ref()
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
