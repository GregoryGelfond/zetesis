//! Device ownership for a separately checked complete-theory certificate.

use super::admission::Admission;
use super::transport::{Execution, Resident};
use super::{
    SHADER, TightGpuActivity, TightGpuBatchStats, TightGpuCheck, TightGpuError, TightGpuLimits,
    poll,
};
use crate::runtime::{DeviceProfile, ErrorScopes, Runtime};
use crate::{GpuBackendPreference, GpuError, GpuErrorKind, GpuInfo, GpuOptions, GpuSelection};
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, TightPlan};

/// Real-device original satisfaction and ranked producer support checking.
///
/// Takes a complete opaque [`TightPlan`]; never constructs an unchecked rank or
/// relies on an incomplete lazy registry. Candidate generation, exact residual
/// completion, objectives and CPU fallback remain the caller's responsibilities.
/// Every nonempty successful call executes the support kernel on the selected
/// adapter. This is a separate primitive, not ordinary solver dispatch policy.
///
/// Work is linear in candidates times nodes, roots, producers and atoms. Each
/// workgroup evaluates its DAG sequentially, then shares root/support scans
/// across 64 invocations. Truth uses one word per node; candidate membership and
/// producer support use one bit per atom, in separate world-major arrays. Heads
/// sharing a support word use atomic OR; their contention can affect throughput.
/// Shared graph and transport storage have explicit byte bounds. The oracle
/// retains one shared theory handle and one exact transport
/// shape, without cloning the caller's certificate or storing its ranks.
pub struct GpuTightOracle {
    runtime: Runtime,
    resident: Option<Resident>,
    epoch: u32,
    last: Option<TightGpuBatchStats>,
    activity: TightGpuActivity,
}
impl GpuTightOracle {
    /// Create a Metal device/pipeline without API or CPU fallback.
    ///
    /// # Errors
    /// Refuses unavailable or policy-incompatible adapters, capabilities,
    /// allocation, shader validation or device creation.
    pub fn new_metal(options: GpuOptions) -> Result<Self, GpuError> {
        Self::new_selected(
            options,
            GpuSelection {
                backend: GpuBackendPreference::Metal,
                ..Default::default()
            },
        )
    }
    /// Create an independently owned device/pipeline using hard adapter filters.
    /// Construction and compilation costs are outside per-batch limits.
    ///
    /// # Errors
    /// Returns typed adapter, capability, allocation, validation or device errors.
    pub fn new_selected(options: GpuOptions, selection: GpuSelection) -> Result<Self, GpuError> {
        let runtime = pollster::block_on(Runtime::new(
            options,
            selection,
            DeviceProfile {
                device_label: "zetesis tight support device",
                shader_label: "ranked producer support",
                pipeline_label: "complete original support",
                shader: SHADER.into(),
                entry_point: "check",
                validate_limits: check_limits,
            },
        ))?;
        Ok(Self {
            runtime,
            resident: None,
            epoch: 0,
            last: None,
            activity: TightGpuActivity::default(),
        })
    }
    /// Actual selected native adapter identity; no device operation.
    #[must_use]
    pub fn info(&self) -> &GpuInfo {
        &self.runtime.info
    }
    /// Successful nonempty batch accounting; cleared at every check start.
    #[must_use]
    pub fn last_batch_stats(&self) -> Option<&TightGpuBatchStats> {
        self.last.as_ref()
    }
    /// Submitted and validated work in the latest attempt, including failure.
    /// Absence of completion never establishes that submitted work did not run.
    #[must_use]
    pub const fn activity(&self) -> TightGpuActivity {
        self.activity
    }
    /// Drop authored resident handles and successful-batch statistics. Preserve
    /// latest-attempt activity. Does not cancel work, reset the epoch, or repair
    /// a device invalidated by timeout, control interruption or execution failure.
    pub fn clear_residency(&mut self) {
        self.resident = None;
        self.last = None;
    }

    /// Check complete original truth and support for every ordered occurrence.
    /// Repeated interpretations remain repeated results. The first false root
    /// follows original root-list order; unsupported atoms use ascending order.
    ///
    /// Checks control and health even for an empty batch, then returns no results
    /// without charging storage/work or dispatching. On nonempty calls, clones
    /// of one Theory reuse immutable storage; independent equal theories do not.
    /// Every admitted `TightPlan` canonically extracts producers from all original
    /// roots, independently of the particular valid ranks. Thus theory identity
    /// is sufficient for this producer cache; arbitrary supplied producers are
    /// never accepted. Transport is replaced when candidate count changes.
    ///
    /// # Errors
    /// Returns no partial result vector on foreign identity, capacity, work,
    /// allocation, cancellation, deadline, validation, device or readback failure.
    /// Host preparation refusals leave the oracle reusable. Failures after device
    /// allocation/submission invalidate it; create a new oracle before retrying.
    /// Control is polled during packing, waiting, decoding and before returning.
    pub fn check_batch(
        &mut self,
        certificate: &TightPlan,
        candidates: &[Interpretation],
        limits: TightGpuLimits,
        control: &Control,
    ) -> Result<Vec<TightGpuCheck>, TightGpuError> {
        self.last = None;
        self.activity = TightGpuActivity::default();
        self.check(certificate, candidates, limits, control)
            .map_err(Into::into)
    }

    fn check(
        &mut self,
        certificate: &TightPlan,
        candidates: &[Interpretation],
        limits: TightGpuLimits,
        control: &Control,
    ) -> Result<Vec<TightGpuCheck>, GpuError> {
        poll(control)?;
        self.runtime.check_health()?;
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        let Admission { fresh, plan, stats } = Admission::new(
            certificate,
            candidates,
            limits,
            &self.runtime.limits,
            self.resident
                .as_ref()
                .map(|resident| (&resident.graph, resident.matches_count(candidates.len()))),
            self.epoch,
            control,
        )?;
        // Evict authored buffers before replacing them; deferred driver retirement
        // is explicitly outside the logical payload budget.
        if fresh.is_some() {
            self.resident = None;
        } else if stats.transport_allocated
            && let Some(resident) = self.resident.as_mut()
        {
            resident.transport = None;
        }
        let graph = fresh
            .as_ref()
            .or_else(|| self.resident.as_ref().map(|r| &r.graph))
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing tight packing graph"))?;
        let seeds = plan.pack(graph, candidates, control)?;
        let packed = fresh
            .as_ref()
            .map(|g| g.pack(certificate, control))
            .transpose()?;
        poll(control)?;
        let scopes = ErrorScopes::new(&self.runtime.device);
        if let Some((graph, packed)) = fresh.zip(packed) {
            self.activity.uploaded_bytes = graph.bytes;
            self.resident = Some(Resident::new(&self.runtime.device, graph, &packed));
        }
        self.epoch = plan.epoch;
        let outcome = self
            .resident
            .as_mut()
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing resident tight graph"))
            .and_then(|resident| {
                resident.dispatch(
                    &Execution {
                        device: &self.runtime.device,
                        queue: &self.runtime.queue,
                        pipeline: &self.runtime.pipeline,
                        timeout: limits.timeout,
                        control,
                    },
                    &seeds,
                    &plan,
                    &mut self.activity,
                )
            });
        let result = self.runtime.complete(scopes, outcome).and_then(|checks| {
            poll(control)?;
            Ok(checks)
        });
        if result.is_ok() {
            self.last = Some(stats);
        } else {
            self.runtime.invalidate();
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
            8,
            limits.max_compute_workgroup_storage_size,
        ),
        (
            "storage bindings",
            7,
            limits.max_storage_buffers_per_shader_stage,
        ),
        (
            "uniform bindings",
            1,
            limits.max_uniform_buffers_per_shader_stage,
        ),
        ("bindings per group", 8, limits.max_bindings_per_bind_group),
    ] {
        if available < required {
            return Err(GpuError::new(
                GpuErrorKind::Capacity,
                format!("tight {label}: need {required}, device provides {available}"),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/tight/device.rs"]
mod tests;
