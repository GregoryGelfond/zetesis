//! Caller-owned compilation for one exact context and gate implementation.

use super::GateProjection;
use crate::runtime::{DeviceProfile, Runtime};
use crate::{GpuContext, GpuError, GpuErrorKind, GpuInfo, GpuOptions, GpuSelection};
use std::sync::Arc;

/// One compiled finite-formula pipeline on one exact device context.
///
/// Cloning retains the same pipeline, context and gate implementation in constant
/// time. Independent compilation has a different identity even on the same
/// context. No theory, transport, epoch, search state or batch statistics belong
/// to this owner. Each [`super::GpuFormulaOracle::from_profile`] starts those
/// mutable resources afresh; simultaneously live oracles retain separate storage.
///
/// Compilation has driver-dependent allocation and duration outside dispatch
/// limits. Keeping a profile alive retains its pipeline and device infrastructure;
/// per-oracle byte limits do not account for that infrastructure or sum all live
/// oracles. This is explicit reuse, with no global cache or concurrent scheduling.
#[derive(Clone)]
pub struct GpuFormulaProfile {
    pub(super) runtime: Arc<Runtime>,
    projection: GateProjection,
}

impl GpuFormulaProfile {
    /// Select a native device and compile the default gate implementation.
    ///
    /// # Errors
    /// Returns typed adapter, capacity, allocation, validation or device failure.
    /// Selection retains the hard API and physical-device policy.
    pub fn new_selected(options: GpuOptions, selection: GpuSelection) -> Result<Self, GpuError> {
        Self::new_selected_with_projection(options, selection, GateProjection::default())
    }

    /// Select a native device and compile exactly the requested implementation.
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
        let runtime = pollster::block_on(Runtime::new(
            options,
            selection,
            Self::descriptor(projection)?,
        ))?;
        Ok(Self {
            runtime: Arc::new(runtime),
            projection,
        })
    }

    /// Compile the default implementation on an existing context.
    ///
    /// # Errors
    /// Refuses an active or invalidated context, unsupported granted limits,
    /// allocation or shader validation failure. No adapter selection occurs.
    pub fn from_context(context: &GpuContext) -> Result<Self, GpuError> {
        Self::from_context_with_projection(context, GateProjection::default())
    }

    /// Compile the requested implementation on an existing context.
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
        let runtime = pollster::block_on(Runtime::from_context(
            context,
            Self::descriptor(projection)?,
        ))?;
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

    /// Actual adapter metadata; inspection does not establish device health.
    #[must_use]
    pub fn info(&self) -> &GpuInfo {
        self.context().info()
    }

    /// Whether both handles retain the exact same compiled pipeline owner.
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
            validate_limits: check_limits,
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
