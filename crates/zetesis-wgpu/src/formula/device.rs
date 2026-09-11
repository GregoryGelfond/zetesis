//! Device ownership and health for the separate finite-formula profile.

use super::packing::{Graph, Plan};
use super::transport::Resident;
use super::{FormulaBatchStats, FormulaCheck, FormulaLimits, GateProjection};
use crate::runtime::{DeviceProfile, ErrorScopes, Runtime};
use crate::{GpuBackendPreference, GpuError, GpuErrorKind, GpuInfo, GpuOptions, GpuSelection};
use zetesis_ferraris::{Interpretation, Theory};

/// GPU original-truth evaluation and sound frozen-query propagation.
///
/// This is a partial reduct primitive; exact residual search remains necessary. No source grounding, objective
/// scoring, candidate enumeration or CPU oracle fallback occurs here.
pub struct GpuFormulaOracle {
    runtime: Runtime,
    projection: GateProjection,
    resident: Option<Resident>,
    epoch: u32,
    last: Option<FormulaBatchStats>,
}
impl GpuFormulaOracle {
    /// Require Metal and the requested physical-device policy.
    ///
    /// # Errors
    /// Refuses unavailable/refused adapters, insufficient capabilities, device
    /// creation and pipeline validation. No different API or CPU is substituted.
    pub fn new_metal(options: GpuOptions) -> Result<Self, GpuError> {
        Self::new_metal_with_projection(options, GateProjection::default())
    }

    /// Require physical Metal with an explicitly selected gate implementation.
    ///
    /// Owns a new device/pipeline and initially no resident theory or transport.
    /// Enumerated borrows its shader; Bitwise reserves and assembles one fixed
    /// source. Device creation and compilation have driver-dependent allocation
    /// and duration, outside dispatch limits.
    /// Existing constructors retain [`GateProjection::Enumerated`].
    ///
    /// # Errors
    /// Refuses unavailable adapters, insufficient capabilities, source allocation,
    /// device creation or validation. It substitutes neither another API nor CPU.
    pub fn new_metal_with_projection(
        options: GpuOptions,
        projection: GateProjection,
    ) -> Result<Self, GpuError> {
        Self::new_selected_with_projection(
            options,
            GpuSelection {
                backend: GpuBackendPreference::Metal,
                ..GpuSelection::default()
            },
            projection,
        )
    }
    /// Select a native adapter and create this profile's separate device/pipeline.
    ///
    /// # Errors
    /// Returns typed adapter, capacity, allocation, validation or device failure.
    /// Selection follows the existing hard API/vendor and physical-device policy.
    pub fn new_selected(options: GpuOptions, selection: GpuSelection) -> Result<Self, GpuError> {
        Self::new_selected_with_projection(options, selection, GateProjection::default())
    }

    /// Create an independently owned native device/pipeline for this projection.
    ///
    /// Uses the same hard API/vendor and physical-device selection policy as
    /// [`Self::new_selected`]. Construction does not ground, enumerate, or alter
    /// a theory. Enumerated source is borrowed; Bitwise assembles one fixed-size
    /// shader with a fallible reservation, then transfers it to shader creation.
    /// Device/pipeline costs depend on the driver and are outside dispatch limits.
    ///
    /// # Errors
    /// Returns typed adapter, capacity, allocation, validation or device failure.
    pub fn new_selected_with_projection(
        options: GpuOptions,
        selection: GpuSelection,
        projection: GateProjection,
    ) -> Result<Self, GpuError> {
        let runtime =
            pollster::block_on(Runtime::new(options, selection, Self::profile(projection)?))?;
        Ok(Self::with_runtime(runtime, projection))
    }

    /// Compile this primitive on an existing device context.
    ///
    /// Prepared subjects belong to this primitive; device health and execution
    /// serialization are shared with all primitives using the context.
    ///
    /// # Errors
    /// Refuses an active or invalidated context, unsupported granted limits,
    /// allocation and shader validation failure. No adapter selection occurs.
    pub fn from_context(context: &crate::GpuContext) -> Result<Self, GpuError> {
        Self::from_context_with_projection(context, GateProjection::default())
    }

    /// Compile the selected implementation on an existing shared context.
    ///
    /// # Errors
    /// Reports the same context, capability and compilation failures as
    /// [`Self::from_context`].
    pub fn from_context_with_projection(
        context: &crate::GpuContext,
        projection: GateProjection,
    ) -> Result<Self, GpuError> {
        let runtime =
            pollster::block_on(Runtime::from_context(context, Self::profile(projection)?))?;
        Ok(Self::with_runtime(runtime, projection))
    }

    /// Exact device context retained by this primitive.
    #[must_use]
    pub fn context(&self) -> &crate::GpuContext {
        &self.runtime.context
    }

    fn with_runtime(runtime: Runtime, projection: GateProjection) -> Self {
        Self {
            runtime,
            projection,
            resident: None,
            epoch: 0,
            last: None,
        }
    }

    fn profile(projection: GateProjection) -> Result<DeviceProfile, GpuError> {
        Ok(DeviceProfile {
            device_label: "zetesis frozen formula device",
            shader_label: "frozen formula propagation",
            pipeline_label: "cooperative finite-formula query",
            shader: projection.shader()?,
            entry_point: "propagate",
            validate_limits: check_limits,
        })
    }

    /// Selected gate implementation; constant-time observation without I/O.
    #[must_use]
    pub const fn projection(&self) -> GateProjection {
        self.projection
    }
    /// Identity of the actual selected native adapter.
    #[must_use]
    pub fn info(&self) -> &GpuInfo {
        self.runtime.context.info()
    }
    /// Last successful nonempty batch's allocation/reuse accounting. Every new
    /// call clears it, including an empty or refused call.
    #[must_use]
    pub fn last_batch_stats(&self) -> Option<&FormulaBatchStats> {
        self.last.as_ref()
    }
    /// Release authored resident handles. This neither cancels GPU work nor
    /// repairs an invalidated device; driver retirement can occur later.
    pub fn clear_residency(&mut self) {
        self.resident = None;
        self.last = None;
    }
    /// Evaluate original roots, freeze M, and cooperatively narrow the proper-
    /// subset query. Results retain input order and no partial batch is returned.
    /// Clones of one Theory reuse its graph; independent equal theories do not.
    /// Exact candidate-count transport is replaced when that count changes.
    ///
    /// # Errors
    /// Refuses foreign interpretations, insufficient setup/work/storage limits,
    /// u32 address overflow, allocation, device/poll, validation or readback
    /// failures. Execution/readback failures invalidate the entire context.
    /// Shape, input and Busy refusals leave it reusable. A propagation limit is an
    /// explicit residual, never a proof of stability.
    pub fn propagate_batch(
        &mut self,
        theory: &Theory,
        candidates: &[Interpretation],
        limits: FormulaLimits,
    ) -> Result<Vec<FormulaCheck>, GpuError> {
        self.last = None;
        let context = self.runtime.context.clone();
        let _lease = context.lease()?;
        self.runtime.check_health()?;
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        let epoch = self.epoch.checked_add(1).ok_or_else(|| {
            GpuError::new(GpuErrorKind::Capacity, "formula epoch counter exhausted")
        })?;
        let fresh = if self
            .resident
            .as_ref()
            .is_some_and(|resident| resident.graph.theory.same_instance(theory))
        {
            None
        } else {
            Some(Graph::new(theory, self.runtime.limits())?)
        };
        let graph = fresh
            .as_ref()
            .or_else(|| self.resident.as_ref().map(|resident| &resident.graph))
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing formula graph"))?;
        let plan = Plan::new(
            graph,
            candidates.len(),
            limits,
            self.runtime.limits(),
            fresh.is_some(),
            epoch,
        )?;
        if candidates
            .iter()
            .any(|candidate| !theory.same_instance(candidate.theory()))
        {
            return Err(GpuError::new(
                GpuErrorKind::Seed,
                "candidate belongs to another Theory",
            ));
        }
        let stats = FormulaBatchStats {
            theory_uploaded: fresh.is_some(),
            transport_allocated: fresh.is_some()
                || self
                    .resident
                    .as_ref()
                    .is_none_or(|resident| !resident.matches(&plan)),
            resident_theory_bytes: graph.bytes,
            resident_transport_bytes: plan.transport,
            accounted_bytes: plan.accounted,
        };
        if fresh.is_some() {
            self.resident = None;
        } else if stats.transport_allocated
            && let Some(resident) = self.resident.as_mut()
        {
            resident.transport = None;
        }
        let graph = fresh
            .as_ref()
            .or_else(|| self.resident.as_ref().map(|resident| &resident.graph))
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing formula packing graph"))?;
        let seeds = plan.pack(graph, candidates)?;
        let packed = fresh.as_ref().map(Graph::pack).transpose()?;
        let scopes = ErrorScopes::new(self.runtime.device());
        if let Some((graph, (nodes, roots))) = fresh.zip(packed) {
            self.resident = Some(Resident::new(self.runtime.device(), graph, &nodes, &roots));
        }
        self.epoch = epoch;
        let outcome = self
            .resident
            .as_mut()
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing resident formula"))?
            .dispatch(
                self.runtime.device(),
                self.runtime.queue(),
                &self.runtime.pipeline,
                &seeds,
                &plan,
                limits.timeout,
            );
        let result = self.runtime.complete(scopes, outcome);
        if result.is_ok() {
            self.last = Some(stats);
        }
        result
    }
}
fn check_limits(limits: &wgpu::Limits) -> Result<(), GpuError> {
    for (label, required, available) in [
        (
            "workgroup invocations",
            64,
            limits.max_compute_invocations_per_workgroup,
        ),
        ("workgroup width", 64, limits.max_compute_workgroup_size_x),
        (
            "workgroup storage",
            16,
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

#[cfg(test)]
#[path = "../../tests/formula/device.rs"]
mod tests;
