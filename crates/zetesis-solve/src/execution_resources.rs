//! Explicit device ownership reused across independent ordinary solves.

/// Caller-owned execution infrastructure, independent of any semantic input.
///
/// The default retains no device and preserves ordinary per-session device
/// selection. With the `gpu` feature, `with_gpu` retains the supplied context
/// and creates each session's primitive on it. `with_formula_profile` also
/// retains one formula profile, whose shader and compiled pipelines repeated
/// formula sessions share. `discovering` retains an adapter enumeration begun
/// before the session: the session's GPU primitive selects over it exactly as
/// ordinary selection does, then requests its own device. Every session still
/// owns fresh resident subjects, epochs, search state, budgets, pending
/// answers, counters and incumbents. CPU worker pools remain session-owned.
///
/// Resources do not select a solving policy: the CPU backend ignores the
/// supplied device, and the session's configuration alone decides whether its
/// primitive requests timestamps. A GPU backend uses this exact context or
/// fails; it never discovers a replacement. Resource ownership does not
/// override the session's selected backend.
///
/// Context operations are serialized through nonblocking leases; an overlapping
/// check is one operation, whose lease also spans the production it runs while
/// its submission is in flight. Busy refuses an overlapping operation without
/// poisoning the owner; device invalidation is
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
    Discovery(zetesis_wgpu::GpuDiscovery),
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

    /// Retain a formula profile for later ordinary formula sessions.
    ///
    /// The profile's exact context also serves other GPU primitives. This makes
    /// one shared-owner clone with no discovery, health check, compilation or
    /// submission. General formula sessions share the profile's shader and its
    /// pipelines, as [`zetesis_wgpu::GpuFormulaProfile`] documents; a
    /// tight-support session creates its own primitive on the profile's exact
    /// context. Each formula session validates policy, Busy, health and
    /// granted capabilities before starting independent residency and search.
    /// CPU sessions ignore these resources.
    #[cfg(feature = "gpu")]
    #[must_use]
    pub fn with_formula_profile(profile: &zetesis_wgpu::GpuFormulaProfile) -> Self {
        Self {
            gpu: Some(GpuResources::Formula(profile.clone())),
        }
    }

    /// Resources for an ordinary session of `config`: for a GPU backend, an
    /// adapter enumeration of that backend's API begun now on a background
    /// thread ([`zetesis_wgpu::GpuDiscovery`]); otherwise, and in a build
    /// without GPU support, the default.
    ///
    /// The session's GPU primitive selects over the adapter enumeration as
    /// ordinary selection does, with its own capability check, then requests
    /// its own device. Adapter choice and failures are therefore those of a
    /// session without these resources started when the enumeration began, a
    /// snapshot whose age [`zetesis_wgpu::GpuDiscovery`] states; timestamps are
    /// requested as in that session. It serves one primitive; a later session,
    /// or a session of another API, selects as it would without it.
    /// Dropping the last clone before a primitive takes the adapter enumeration
    /// waits for it to finish.
    #[must_use]
    pub fn discovering(config: &crate::SolveConfig) -> Self {
        #[cfg(feature = "gpu")]
        if config.backend.is_gpu() {
            return Self {
                gpu: Some(GpuResources::Discovery(zetesis_wgpu::GpuDiscovery::spawn(
                    selection(config.backend),
                ))),
            };
        }
        #[cfg(not(feature = "gpu"))]
        let _ = config;
        Self::default()
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
            Some(GpuResources::Discovery(_)) | None => None,
        }
    }

    /// Where a GPU session of `config` obtains its primitive's device, decided
    /// here once for every primitive the session may construct.
    ///
    /// A retained context, a formula profile's included, is used exactly once
    /// it matches the session's API and the default adapter policy. Otherwise
    /// the primitive selects its own adapter under that policy, over a retained
    /// adapter enumeration of the session's API when there is one. Explicit resources
    /// thus keep their exact identity and granted features, and every selection
    /// keeps primitive-specific adapter eligibility. A selected primitive
    /// requests timestamps exactly when the session requests statistics; only
    /// the formula and tight primitives read them.
    ///
    /// # Errors
    /// A retained context whose device does not match the session's policy.
    #[cfg(feature = "gpu")]
    pub(crate) fn device_source(
        &self,
        config: &crate::SolveConfig,
    ) -> Result<zetesis_wgpu::GpuDeviceSource<'_>, zetesis_wgpu::GpuError> {
        let options = zetesis_wgpu::GpuOptions::default();
        let selection = selection(config.backend);
        let timing = config.stats;
        if let Some(context) = self.gpu_context() {
            context.check_selection(options, selection)?;
            return Ok(zetesis_wgpu::GpuDeviceSource::Context(context));
        }
        Ok(match self.gpu.as_ref() {
            Some(GpuResources::Discovery(discovery)) if discovery.selection() == selection => {
                zetesis_wgpu::GpuDeviceSource::Discovered {
                    discovery,
                    options,
                    timing,
                }
            }
            _ => zetesis_wgpu::GpuDeviceSource::Selected {
                options,
                selection,
                timing,
            },
        })
    }
}

#[cfg(all(test, feature = "gpu"))]
mod tests;

/// The GPU a device route opens: the solver's one application of the backend
/// vocabulary's resolution rule, so `gpu` becomes the platform's native API here
/// and nowhere else. Device routes run only for a GPU backend; the route choice
/// on [`SolveConfig::backend`](crate::SolveConfig::backend) establishes that
/// before any of them is entered.
#[cfg(feature = "gpu")]
pub(crate) fn selection(backend: crate::Backend) -> zetesis_wgpu::GpuSelection {
    zetesis_wgpu::GpuSelection {
        api: backend
            .resolved_api()
            .expect("device routes run only for a GPU backend"),
    }
}
