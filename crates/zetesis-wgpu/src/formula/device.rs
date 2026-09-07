//! Device ownership and health for the separate finite-formula profile.

use super::packing::{Graph, Plan};
use super::transport::Resident;
use super::{FormulaBatchStats, FormulaCheck, FormulaLimits};
use crate::runtime::{DeviceProfile, ErrorScopes, Runtime};
use crate::{GpuBackendPreference, GpuError, GpuErrorKind, GpuInfo, GpuOptions, GpuSelection};
use zetesis_ferraris::{Interpretation, Theory};

const SHADER: &str = include_str!("../formula.wgsl");

/// GPU original-truth evaluation and sound frozen-query propagation.
///
/// This is a partial reduct primitive; exact residual search remains necessary. No source grounding, objective
/// scoring, candidate enumeration or CPU oracle fallback occurs here.
pub struct GpuFormulaOracle {
    runtime: Runtime,
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
        Self::new_selected(
            options,
            GpuSelection {
                backend: GpuBackendPreference::Metal,
                ..GpuSelection::default()
            },
        )
    }
    /// Select a native adapter and create this profile's separate device/pipeline.
    ///
    /// # Errors
    /// Returns typed adapter, capacity, allocation, validation or device failure.
    /// Selection follows the existing hard API/vendor and physical-device policy.
    pub fn new_selected(options: GpuOptions, selection: GpuSelection) -> Result<Self, GpuError> {
        let runtime = pollster::block_on(Runtime::new(
            options,
            selection,
            DeviceProfile {
                device_label: "zetesis frozen formula device",
                shader_label: "frozen formula propagation",
                pipeline_label: "cooperative finite-formula query",
                shader: SHADER,
                entry_point: "propagate",
                validate_limits: check_limits,
            },
        ))?;
        Ok(Self {
            runtime,
            resident: None,
            epoch: 0,
            last: None,
        })
    }
    /// Identity of the actual selected native adapter.
    #[must_use]
    pub fn info(&self) -> &GpuInfo {
        &self.runtime.info
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
    /// failures. Execution/readback failures invalidate this instance. Shape or
    /// input refusals leave it reusable. A propagation limit is instead an
    /// explicit residual, never a proof of stability.
    pub fn propagate_batch(
        &mut self,
        theory: &Theory,
        candidates: &[Interpretation],
        limits: FormulaLimits,
    ) -> Result<Vec<FormulaCheck>, GpuError> {
        self.last = None;
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
            Some(Graph::new(theory, &self.runtime.limits)?)
        };
        let graph = fresh
            .as_ref()
            .or_else(|| self.resident.as_ref().map(|resident| &resident.graph))
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing formula graph"))?;
        let plan = Plan::new(
            graph,
            candidates.len(),
            limits,
            &self.runtime.limits,
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
        let scopes = ErrorScopes::new(&self.runtime.device);
        if let Some((graph, (nodes, roots))) = fresh.zip(packed) {
            self.resident = Some(Resident::new(&self.runtime.device, graph, &nodes, &roots));
        }
        self.epoch = epoch;
        let outcome = self
            .resident
            .as_mut()
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing resident formula"))?
            .dispatch(
                &self.runtime.device,
                &self.runtime.queue,
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
