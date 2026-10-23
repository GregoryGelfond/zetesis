//! Caller-owned compilation for one exact context and gate implementation.

use super::GateProjection;
use crate::runtime::{DeviceProfile, Runtime};
use crate::{
    GpuContext, GpuDeviceSource, GpuError, GpuErrorKind, GpuInfo, GpuOptions, GpuSelection,
};
use std::sync::Arc;

/// One finite-formula kernel on one exact device context, shared by every
/// oracle started from it.
///
/// A profile retains its validated shader module and the pipeline of each
/// workgroup size compiled for it. A fixed size's pipeline is compiled when the
/// profile is created; a fitted size's by the first check, from any oracle
/// sharing the profile, that dispatches that size. Oracles and sessions sharing
/// a profile share its pipelines, so each size is compiled once per profile.
///
/// Cloning retains the same shader, pipelines, context and gate implementation
/// in constant time. Independent compilation has a different identity even on
/// the same context. No theory, transport, epoch, search state or batch
/// statistics belong to this owner. Each
/// [`super::GpuFormulaOracle::from_profile`] starts those mutable resources
/// afresh; simultaneously live oracles retain separate storage.
///
/// Compilation has driver-dependent allocation and duration outside dispatch
/// limits. Keeping a profile alive retains its shader, pipelines and device
/// infrastructure; per-oracle byte limits do not account for that
/// infrastructure or sum all live oracles. This is explicit reuse, with no
/// global cache or concurrent scheduling.
#[derive(Clone)]
pub struct GpuFormulaProfile {
    pub(super) runtime: Arc<Runtime>,
    projection: GateProjection,
}

impl GpuFormulaProfile {
    /// Select a native device and compile the default gate implementation's
    /// shader; its pipelines are compiled as the type documents.
    ///
    /// # Errors
    /// Returns typed adapter, capacity, allocation, validation or device failure.
    /// Selection retains the hard API and physical-device policy.
    pub fn new_selected(options: GpuOptions, selection: GpuSelection) -> Result<Self, GpuError> {
        Self::new_selected_with_projection(options, selection, GateProjection::default())
    }

    /// Select a native device and request optional compute-pass timestamps.
    ///
    /// Retains the default gate implementation and the same profile-specific
    /// adapter eligibility as [`Self::new_selected`]. Unsupported timestamps
    /// leave the solver available; inspect [`GpuContext::compute_timing`].
    ///
    /// # Errors
    /// Returns the same typed construction failures as [`Self::new_selected`].
    pub fn new_selected_with_timing(
        options: GpuOptions,
        selection: GpuSelection,
    ) -> Result<Self, GpuError> {
        Self::from_source(GpuDeviceSource::Selected {
            options,
            selection,
            timing: true,
        })
    }

    /// The default gate implementation on the device `source` provides: as
    /// [`Self::new_selected`], or [`Self::new_selected_with_timing`] when the
    /// source requests timestamps, for a selected or discovered source, and as
    /// [`Self::from_context`] for a context.
    ///
    /// # Errors
    /// The typed failures of the constructor matching `source`, at the same step.
    pub(crate) fn from_source(source: GpuDeviceSource<'_>) -> Result<Self, GpuError> {
        let projection = GateProjection::default();
        Self::open(source, projection, Self::descriptor(projection)?)
    }

    /// Select a native device and compile exactly the requested implementation's
    /// shader.
    ///
    /// Enumerated source is borrowed. Bitwise assembles one fixed source with a
    /// fallible reservation. Construction does not prepare a theory or dispatch.
    ///
    /// # Errors
    /// Returns adapter, capability, source allocation, validation or device
    /// failure. No different API, projection or CPU is substituted.
    pub fn new_selected_with_projection(
        options: GpuOptions,
        selection: GpuSelection,
        projection: GateProjection,
    ) -> Result<Self, GpuError> {
        Self::open(
            GpuDeviceSource::Selected {
                options,
                selection,
                timing: false,
            },
            projection,
            Self::descriptor(projection)?,
        )
    }

    /// Compile the default implementation's shader on an existing context; its
    /// pipelines are compiled as the type documents.
    ///
    /// # Errors
    /// Refuses an active or invalidated context, unsupported granted limits,
    /// allocation or shader validation failure. No adapter selection occurs.
    pub fn from_context(context: &GpuContext) -> Result<Self, GpuError> {
        Self::from_context_with_projection(context, GateProjection::default())
    }

    /// Compile the requested implementation's shader on an existing context.
    ///
    /// The exact context is retained. This does not discover a replacement or
    /// reuse another independently compiled profile with equal metadata.
    ///
    /// # Errors
    /// Reports the context, capability and compilation failures of
    /// [`Self::from_context`], plus fallible shader-source construction.
    pub fn from_context_with_projection(
        context: &GpuContext,
        projection: GateProjection,
    ) -> Result<Self, GpuError> {
        Self::from_context_with_kernel(context, projection, crate::GpuKernelOptions::default())
    }

    /// Compile the shader of explicit physical choices on the exact retained
    /// context.
    ///
    /// The workgroup sizing's base size is checked against granted limits and
    /// the shader validated; a fixed size is compiled now, and a fitted size by
    /// the first check that dispatches it. Optional instructions
    /// require granted features; otherwise the portable reduction is retained.
    /// None of these choices changes the frozen reduct or its work allowance.
    ///
    /// # Errors
    /// Reports the construction failures of [`Self::from_context`], including
    /// a fixed workgroup size exceeding the device's granted limits, and a
    /// failure to create the shader or compile a fixed size, which invalidates
    /// the context. A fitted size's compilation failure is reported by the
    /// first check that needs it.
    pub fn from_context_with_kernel(
        context: &GpuContext,
        projection: GateProjection,
        options: crate::GpuKernelOptions,
    ) -> Result<Self, GpuError> {
        let mut descriptor = Self::descriptor(projection)?;
        descriptor.kernel = crate::kernel::Kernel::Formula(options);
        Self::open(GpuDeviceSource::Context(context), projection, descriptor)
    }

    fn open(
        source: GpuDeviceSource<'_>,
        projection: GateProjection,
        descriptor: DeviceProfile,
    ) -> Result<Self, GpuError> {
        let runtime = pollster::block_on(Runtime::open(source, descriptor))?;
        Ok(Self {
            runtime: Arc::new(runtime),
            projection,
        })
    }

    /// Exact device context retained by this compilation.
    #[must_use]
    pub fn context(&self) -> &GpuContext {
        &self.runtime.context
    }

    /// Selected gate implementation; inspection performs no device operation.
    #[must_use]
    pub const fn projection(&self) -> GateProjection {
        self.projection
    }

    /// Physical options retained by this exact compilation; no device operation.
    ///
    /// # Panics
    /// Panics if the internal runtime is not a formula kernel. Every public
    /// constructor establishes this invariant; callers cannot replace it.
    #[must_use]
    pub fn kernel_options(&self) -> crate::GpuKernelOptions {
        self.runtime.kernel.options().expect("formula kernel")
    }

    /// Actual adapter metadata; inspection does not establish device health.
    #[must_use]
    pub fn info(&self) -> &GpuInfo {
        self.context().info()
    }

    /// Whether both handles retain the exact same shader and pipeline owner.
    ///
    /// Equal context, shader or projection metadata alone is insufficient.
    #[must_use]
    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.runtime, &other.runtime)
    }

    fn descriptor(projection: GateProjection) -> Result<DeviceProfile, GpuError> {
        Ok(DeviceProfile {
            device_label: "zetesis frozen formula device",
            shader_label: "frozen formula propagation",
            pipeline_label: "cooperative finite-formula query",
            shader: projection.shader()?,
            entry_point: "propagate",
            dense_entry_point: None,
            validate_limits: check_limits,
            kernel: crate::kernel::Kernel::Formula(crate::GpuKernelOptions::default()),
        })
    }
}

pub(super) fn check_limits(limits: &wgpu::Limits) -> Result<(), GpuError> {
    for (label, required, available) in [
        (
            "workgroup invocations",
            64,
            limits.max_compute_invocations_per_workgroup,
        ),
        ("workgroup width", 64, limits.max_compute_workgroup_size_x),
        (
            "workgroup storage",
            24,
            limits.max_compute_workgroup_storage_size,
        ),
        (
            "storage bindings",
            6,
            limits.max_storage_buffers_per_shader_stage,
        ),
        (
            "uniform bindings",
            1,
            limits.max_uniform_buffers_per_shader_stage,
        ),
        ("bindings per group", 7, limits.max_bindings_per_bind_group),
    ] {
        if available < required {
            return Err(GpuError::new(
                GpuErrorKind::Capacity,
                format!("formula {label}: need {required}, device provides {available}"),
            ));
        }
    }
    Ok(())
}
