//! Shared device lifecycle for one retained prepared numeric group.

use super::{
    AggregateGpuActivity, AggregateGpuBatchStats, AggregateGpuError, AggregateGpuLimits,
    AggregateGpuPlan, AggregateGpuReduction, LANES, SHADER, capacity, packing::Plan, poll,
    transport::Resident,
};
use crate::{
    GpuError, GpuInfo, GpuOptions, GpuSelection,
    runtime::{DeviceProfile, ErrorScopes, Runtime},
};
use std::sync::Arc;
use zetesis_cpu::Control;
use zetesis_ferraris::native_aggregate::Eligibility;

/// Exact device reduction of already-acquired aggregate eligibility.
///
/// Retains one prepared numeric group and an exact occurrence-count transport.
/// Every successful nonempty batch executes the selected device. This is
/// an aggregate primitive: mask acquisition, head permission, source coverage,
/// candidate generation and stable-model minimality remain outside it.
pub struct GpuAggregateOracle {
    runtime: Runtime,
    resident: Option<Resident>,
    epoch: u32,
    last: Option<AggregateGpuBatchStats>,
    activity: AggregateGpuActivity,
}
impl GpuAggregateOracle {
    /// Create an independently owned device/pipeline with hard API/vendor filters.
    /// Initialization is outside batch resource/time bounds; no CPU fallback occurs.
    ///
    /// # Errors
    /// Returns typed adapter, capability, allocation, validation or device failure.
    pub fn new_selected(options: GpuOptions, selection: GpuSelection) -> Result<Self, GpuError> {
        let runtime = pollster::block_on(Runtime::new(options, selection, Self::profile()))?;
        Ok(Self::with_runtime(runtime))
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
        let runtime = pollster::block_on(Runtime::from_context(context, Self::profile()))?;
        Ok(Self::with_runtime(runtime))
    }

    /// Exact device context retained by this primitive.
    #[must_use]
    pub fn context(&self) -> &crate::GpuContext {
        &self.runtime.context
    }

    fn with_runtime(runtime: Runtime) -> Self {
        Self {
            runtime,
            resident: None,
            epoch: 0,
            last: None,
            activity: AggregateGpuActivity::default(),
        }
    }

    fn profile() -> DeviceProfile {
        DeviceProfile {
            device_label: "zetesis numeric aggregate device",
            shader_label: "exact integer aggregate reduction",
            pipeline_label: "original and frozen aggregate guards",
            shader: SHADER.into(),
            entry_point: "reduce",
            validate_limits: check_limits,
        }
    }
    /// Actual selected adapter identity; no device operation.
    #[must_use]
    pub fn info(&self) -> &GpuInfo {
        self.runtime.context.info()
    }
    /// Latest successfully completed nonempty batch; cleared at every attempt.
    #[must_use]
    pub const fn last_batch_stats(&self) -> Option<&AggregateGpuBatchStats> {
        self.last.as_ref()
    }
    /// Actual submitted/validated work retained after a later failure.
    #[must_use]
    pub const fn activity(&self) -> AggregateGpuActivity {
        self.activity
    }
    /// Drop owned residency and successful statistics. This does not repair an
    /// invalidated device, reset submission identity or clear attempted activity.
    pub fn clear_residency(&mut self) {
        self.resident = None;
        self.last = None;
    }

    /// Reduce every acquired occurrence in input order, preserving repetitions.
    /// Records must reference the plan's exact Group. Original-only and paired
    /// records can share one batch; an unrequested frozen result remains absent.
    /// The kernel reserves both phase scans for each occurrence.
    ///
    /// Pure identity/resource/control refusals preserve prior residency. Planned
    /// incompatible owners are evicted before packing or device allocation, so
    /// old cache slack never rejects an admissible exact new shape. Incoming
    /// owned payload is reported separately from the prospective active ceiling.
    /// Work is linear in tuples and guards per occurrence; device reduction uses
    /// one 64-lane workgroup per occurrence with fixed shared scratch.
    ///
    /// # Errors
    /// Returns no partial truth vector on identity, resource, control, allocation,
    /// validation, device or readback failure. Host preparation failures leave the
    /// device reusable; execution-stage failures drop this residency and invalidate
    /// the entire shared context. Busy contention is a reusable preflight refusal.
    /// Empty calls still check health/control, then perform no work or allocation.
    pub fn check_batch(
        &mut self,
        group: &AggregateGpuPlan<'_>,
        records: &[Eligibility<'_>],
        limits: AggregateGpuLimits,
        control: &Control,
    ) -> Result<Vec<AggregateGpuReduction>, AggregateGpuError> {
        self.last = None;
        self.activity = AggregateGpuActivity::default();
        self.check(group, records, limits, control)
            .map_err(Into::into)
    }

    fn check(
        &mut self,
        group: &AggregateGpuPlan<'_>,
        records: &[Eligibility<'_>],
        limits: AggregateGpuLimits,
        control: &Control,
    ) -> Result<Vec<AggregateGpuReduction>, GpuError> {
        let context = self.runtime.context.clone();
        let _lease = context.lease()?;
        poll(control)?;
        self.runtime.check_health()?;
        if records.is_empty() {
            return Ok(Vec::new());
        }
        let epoch = self
            .epoch
            .checked_add(1)
            .ok_or_else(|| capacity("aggregate submission identity exhausted"))?;
        let plan = Plan::new(
            group,
            records,
            limits,
            self.runtime.limits(),
            epoch,
            control,
        )?;
        let fresh = self
            .resident
            .as_ref()
            .is_none_or(|value| !Arc::ptr_eq(&value.numeric, &group.numeric));
        let allocated = fresh
            || self
                .resident
                .as_ref()
                .is_none_or(|value| !value.matches_count(records.len()));
        let stats = AggregateGpuBatchStats {
            group_uploaded: fresh,
            transport_allocated: allocated,
            incoming_resident_bytes: self
                .resident
                .as_ref()
                .map(Resident::bytes)
                .transpose()?
                .unwrap_or(0),
            resident_group_bytes: group.numeric.bytes,
            resident_transport_bytes: plan.transport,
            accounted_bytes: plan.accounted,
            host_work: plan.host_work,
            occurrences: u64::from(plan.worlds),
            device_work: plan.total_work,
        };
        poll(control)?;
        if fresh {
            self.resident = None;
        } else if allocated && let Some(resident) = self.resident.as_mut() {
            resident.transport = None;
        }
        let masks = plan.pack(records, control)?;
        poll(control)?;
        let scopes = ErrorScopes::new(self.runtime.device());
        if fresh {
            self.activity.uploaded_bytes = group.numeric.bytes;
        }
        let resident = self
            .resident
            .get_or_insert_with(|| Resident::new(&self.runtime, Arc::clone(&group.numeric)));
        self.epoch = epoch;
        let outcome = resident.dispatch(
            &self.runtime,
            &masks,
            &plan,
            limits.timeout,
            control,
            &mut self.activity,
        );
        let result = self.runtime.complete(scopes, outcome).and_then(|results| {
            poll(control)?;
            Ok(results)
        });
        if result.is_ok() {
            self.last = Some(stats);
        } else {
            self.resident = None;
            self.runtime.invalidate();
        }
        result
    }
}

fn check_limits(limits: &wgpu::Limits) -> Result<(), GpuError> {
    for (label, required, available) in [
        (
            "workgroup invocations",
            LANES,
            limits.max_compute_invocations_per_workgroup,
        ),
        (
            "workgroup width",
            LANES,
            limits.max_compute_workgroup_size_x,
        ),
        (
            "workgroup scratch",
            4 * LANES * 4,
            limits.max_compute_workgroup_storage_size,
        ),
        (
            "storage bindings",
            4,
            limits.max_storage_buffers_per_shader_stage,
        ),
        (
            "uniform bindings",
            1,
            limits.max_uniform_buffers_per_shader_stage,
        ),
        ("bindings per group", 5, limits.max_bindings_per_bind_group),
    ] {
        if available < required {
            return Err(capacity(&format!(
                "aggregate {label}: need {required}, device provides {available}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/aggregate/device.rs"]
mod tests;
