//! Device ownership and health for the separate finite-formula profile.

use super::packing::{Graph, Plan};
use super::transport::Resident;
use super::{FormulaBatchStats, FormulaCheck, FormulaLimits, GateProjection, GpuFormulaProfile};
use crate::runtime::ErrorScopes;
use crate::{GpuBackendPreference, GpuError, GpuErrorKind, GpuInfo, GpuOptions, GpuSelection};
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, Theory};

/// GPU original-truth evaluation and sound frozen-query propagation.
///
/// This is a partial reduct primitive; exact residual search remains necessary. No source grounding, objective
/// scoring, candidate enumeration or CPU oracle fallback occurs here.
pub struct GpuFormulaOracle {
    profile: GpuFormulaProfile,
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
        Ok(Self::with_profile(
            GpuFormulaProfile::new_selected_with_projection(options, selection, projection)?,
        ))
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
        Ok(Self::with_profile(
            GpuFormulaProfile::from_context_with_projection(context, projection)?,
        ))
    }

    /// Start fresh oracle state using a caller-owned compiled profile.
    ///
    /// Acquires the context's nonblocking lease, checks shared health and granted
    /// formula capabilities, then retains the exact compilation. No shader
    /// assembly, pipeline compilation, theory upload or dispatch occurs. Fresh
    /// residency, epoch and batch accounting are independent of every other
    /// oracle; this operation has fixed cost and retains one shared owner.
    ///
    /// # Errors
    /// Refuses Busy before device health, then unsupported granted capabilities.
    /// No adapter policy is selected here; callers requiring a policy first use
    /// [`crate::GpuContext::check_selection`]. A refusal never creates an oracle.
    pub fn from_profile(profile: &GpuFormulaProfile) -> Result<Self, GpuError> {
        let context = profile.context();
        let _lease = context.lease()?;
        context.check_health()?;
        super::profile::check_limits(context.limits())?;
        Ok(Self::with_profile(profile.clone()))
    }

    /// Exact compiled pipeline owner retained by this oracle.
    #[must_use]
    pub const fn compiled_profile(&self) -> &GpuFormulaProfile {
        &self.profile
    }

    /// Exact device context retained by this primitive.
    #[must_use]
    pub fn context(&self) -> &crate::GpuContext {
        self.profile.context()
    }

    fn with_profile(profile: GpuFormulaProfile) -> Self {
        Self {
            profile,
            resident: None,
            epoch: 0,
            last: None,
        }
    }

    /// Selected gate implementation; constant-time observation without I/O.
    #[must_use]
    pub const fn projection(&self) -> GateProjection {
        self.profile.projection()
    }
    /// Identity of the actual selected native adapter.
    #[must_use]
    pub fn info(&self) -> &GpuInfo {
        self.profile.runtime.context.info()
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
        self.propagate_batch_with_control(theory, candidates, limits, &Control::default())
    }

    /// Propagate with cooperative caller cancellation and deadline observation.
    /// Busy admission precedes control; control precedes device health, including
    /// empty batches. Further polls follow host packing and bracket waits of at
    /// most 50 ms. Host work, driver calls and scope drains are not preempted.
    ///
    /// # Errors
    /// Preserves [`Self::propagate_batch`]'s failures. An interruption returns its
    /// exact stop and no partial checks. Pre-submission control refusal leaves
    /// health reusable; an interrupted submitted operation invalidates it.
    /// Scope/device faults retain priority. This is not a hard wall-clock deadline.
    pub fn propagate_batch_with_control(
        &mut self,
        theory: &Theory,
        candidates: &[Interpretation],
        limits: FormulaLimits,
        control: &Control,
    ) -> Result<Vec<FormulaCheck>, GpuError> {
        self.last = None;
        let context = self.profile.runtime.context.clone();
        let _lease = context.lease()?;
        control.poll().map_err(GpuError::interrupted)?;
        self.profile.runtime.check_health()?;
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
            Some(Graph::new(theory, self.profile.runtime.limits())?)
        };
        let graph = fresh
            .as_ref()
            .or_else(|| self.resident.as_ref().map(|resident| &resident.graph))
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing formula graph"))?;
        let plan = Plan::new(
            graph,
            candidates.len(),
            limits,
            self.profile.runtime.limits(),
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
        control.poll().map_err(GpuError::interrupted)?;
        let scopes = ErrorScopes::new(self.profile.runtime.device());
        if let Some((graph, (nodes, roots))) = fresh.zip(packed) {
            self.resident = Some(Resident::new(
                self.profile.runtime.device(),
                graph,
                &nodes,
                &roots,
            ));
        }
        self.epoch = epoch;
        let outcome = self
            .resident
            .as_mut()
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing resident formula"))
            .and_then(|resident| {
                resident.dispatch(
                    &self.profile.runtime,
                    &seeds,
                    &plan,
                    limits.timeout,
                    control,
                )
            });
        let result = self.profile.runtime.complete(scopes, outcome);
        if result.is_ok() {
            self.last = Some(stats);
        }
        result
    }
}

#[cfg(test)]
#[path = "../../tests/formula/device.rs"]
mod tests;
