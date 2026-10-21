//! Device ownership and health for the separate finite-formula profile.

use super::packing::Plan;
use super::preparation::{Preparation, PreparedGraph};
use super::transport::Resident;
use super::{FormulaBatchStats, FormulaCheck, FormulaLimits, GateProjection, GpuFormulaProfile};
use crate::runtime::ErrorScopes;
use crate::timing::{BatchTiming, ComputeTimer};
use crate::{GpuError, GpuErrorKind, GpuInfo, GpuOptions, GpuSelection};
use std::time::Duration;
use zetesis_backend::{BatchComputeTime, ComputeTimingStatus};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Theory};

/// GPU original-truth evaluation and sound frozen-query propagation.
///
/// The immutable transport retains native threshold and extremum-cut rows as
/// shared condition/value columns. Portable u32 limbs preserve all admitted
/// weights and bounds without optional device integer capabilities. Original
/// truth is exact; frozen numeric projection uses conservative linear bounds,
/// which can leave more residual work than expanded Boolean formulas.
///
/// This is a partial reduct primitive; exact residual search remains necessary.
/// A residual reports no narrowed atom region: callers retain the full original
/// proper-subset query. No source grounding, objective scoring, candidate
/// enumeration or CPU oracle fallback occurs here.
pub struct GpuFormulaOracle {
    profile: GpuFormulaProfile,
    resident: Option<Resident>,
    candidates: crate::candidates::Staging,
    epoch: u32,
    last: Option<FormulaBatchStats>,
    last_submission_candidates: Option<usize>,
    // Timestamp storage and pair ledger, from the first timed batch until the
    // residency is cleared; transports and theories come and go beneath it.
    timer: Option<ComputeTimer>,
    reported: Option<BatchComputeTime>,
}

struct PreparedCall {
    fresh: Option<PreparedGraph>,
    plan: Plan,
    stats: FormulaBatchStats,
}
impl GpuFormulaOracle {
    /// Select a native adapter and create this profile's separate device/pipeline.
    ///
    /// # Errors
    /// Returns typed adapter, capacity, allocation, validation or device failure.
    /// Selection follows the hard API and physical-device policy.
    pub fn new_selected(options: GpuOptions, selection: GpuSelection) -> Result<Self, GpuError> {
        Self::new_selected_with_projection(options, selection, GateProjection::default())
    }

    /// Select a native device and request optional compute-pass timestamps.
    ///
    /// Keeps the same profile-specific adapter eligibility as [`Self::new_selected`].
    /// Unsupported timing is reported by [`crate::GpuContext::compute_timing`];
    /// it does not prevent solving.
    ///
    /// # Errors
    /// Returns the same typed construction failures as [`Self::new_selected`].
    pub fn new_selected_with_timing(
        options: GpuOptions,
        selection: GpuSelection,
    ) -> Result<Self, GpuError> {
        Ok(Self::with_profile(
            GpuFormulaProfile::new_selected_with_timing(options, selection)?,
        ))
    }

    /// Create an independently owned native device/pipeline for this projection.
    ///
    /// Uses the same hard API and physical-device selection policy as
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
            candidates: crate::candidates::Staging::default(),
            epoch: 0,
            last: None,
            last_submission_candidates: None,
            timer: None,
            reported: None,
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
    /// Candidate count from this oracle's most recent call, if it reached queue
    /// submission, even if the result was interrupted or could not decode.
    /// Every new call clears this receipt, including empty and pre-stopped calls.
    /// It certifies submission only, not completed shader work or candidate checks.
    #[must_use]
    pub const fn last_submission_candidates(&self) -> Option<usize> {
        self.last_submission_candidates
    }
    /// The compute-pass time of the batch this oracle's latest call consumed.
    ///
    /// Under enabled timing, a submitting call that completes reports the batch
    /// completed by the previous submitting call, from that batch's own interval;
    /// its own batch's interval is read by the next submitting call or by
    /// [`Self::flush_compute_time`]. A call that fails after submission reports
    /// the previous batch unavailable, abandons its own and retires the timer, so
    /// the next timed batch starts a fresh one. The flush reports the batch it
    /// resolved, and [`Self::clear_residency`] reports a discarded unresolved
    /// batch unavailable. Under unsupported timing each completing call
    /// reports its own batch unavailable; disabled timing reports nothing.
    /// `None` when the latest call consumed no batch: every call clears it at its
    /// start, including empty and refused calls. A measured duration is never
    /// zero and excludes host preparation, transfers and waits.
    #[must_use]
    pub const fn reported_compute_time(&self) -> Option<BatchComputeTime> {
        self.reported
    }
    /// Whether a completed batch still waits for its interval to be resolved by
    /// the next submitting call or by [`Self::flush_compute_time`].
    #[must_use]
    pub fn compute_time_unresolved(&self) -> bool {
        self.timer.as_ref().is_some_and(ComputeTimer::pending)
    }
    /// Resolve the unresolved batch's interval with one resolve-only submission.
    ///
    /// Returns at once, with no report, when no batch is unresolved. Otherwise a
    /// zero `timeout` is refused; the context lease is taken, and `Busy` submits
    /// nothing; `cancellation` and context health are then checked once, and a
    /// stop or an unhealthy context submits nothing. Each refusal leaves the
    /// batch unresolved and a healthy context reusable. The flush then waits at most
    /// `timeout` for its submission, polling no control. On observed completion
    /// the batch is reported, measured or unavailable, and the context stays
    /// reusable. A timeout or a mapping, device or scope failure reports it
    /// unavailable, retires the timer and invalidates the context, as for any
    /// submission whose completion is unknown or faulted. The flush allocates nothing and
    /// downloads 16 bytes; batch statistics and submission receipts exclude it.
    ///
    /// # Errors
    /// Returns the refusals and failures above.
    pub fn flush_compute_time(
        &mut self,
        timeout: Duration,
        cancellation: &Cancellation,
    ) -> Result<(), GpuError> {
        self.reported = None;
        let Some(timer) = self.timer.as_mut().filter(|timer| timer.pending()) else {
            return Ok(());
        };
        crate::runtime::positive_timeout(timeout)?;
        let runtime = &self.profile.runtime;
        let _lease = runtime.context.lease()?;
        cancellation.poll().map_err(GpuError::interrupted)?;
        runtime.check_health()?;
        let scopes = ErrorScopes::new(runtime.device());
        let completion = timer.flush(runtime.device(), runtime.queue(), timeout);
        match runtime.complete(scopes, Ok(completion)) {
            Ok(tail) => {
                self.reported = timer.flushed(tail, runtime.queue().get_timestamp_period());
                Ok(())
            }
            Err(error) => {
                self.reported = crate::timing::retire(&mut self.timer);
                Err(error)
            }
        }
    }
    /// Release authored resident handles and the compute timer. This neither
    /// cancels GPU work nor repairs an invalidated device; driver retirement can
    /// occur later. Clears the last batch and submission receipts. A batch still
    /// unresolved is reported unavailable, since it can never be resolved; the
    /// next timed batch starts a fresh timer.
    pub fn clear_residency(&mut self) {
        self.resident = None;
        self.candidates.release();
        self.last = None;
        self.last_submission_candidates = None;
        self.reported = crate::timing::retire(&mut self.timer);
    }
    /// Evaluate original roots, freeze M, and cooperatively narrow the proper-
    /// subset query. Results retain input order and no partial batch is returned.
    /// Clones of one Theory reuse its graph; independent equal theories do not.
    /// Exact candidate-count transport is replaced when that count changes.
    /// Host candidate words reuse initialized scratch for the same subject/count;
    /// replacement releases the old scratch before cold preparation. Actual
    /// capacity is charged on every call, and a smaller allowance can replace an
    /// oversized optional allocation. Explicit clearing releases this scratch too.
    /// A cold subject is prepared in O(nodes + edges + roots + levels) host work, outside
    /// the per-candidate device-work limit. One buffer retains fixed headers and
    /// complete native operand rows; no binary expansion or per-candidate graph
    /// copy is made. Its upload buffers are released after
    /// device copies are created. Pure chains retain serial truth; other DAGs
    /// distribute independent nodes by dependency level. Narrow levels can incur
    /// synchronization cost without useful parallelism; no crossover is assumed.
    ///
    /// # Errors
    /// Refuses foreign interpretations, insufficient setup/work/storage limits,
    /// u32 address overflow, allocation, device/poll, validation or readback
    /// failures. Execution/readback failures invalidate the entire context.
    /// Minimum shape, input and Busy refusals preserve residency. Later cold
    /// preparation refusal releases the old resident handles and leaves a
    /// healthy, cold context without advancing its epoch. Driver retirement is
    /// excluded from authored byte limits. A propagation limit is an explicit
    /// residual, never a proof of stability.
    pub fn propagate_batch(
        &mut self,
        theory: &Theory,
        candidates: &[Interpretation],
        limits: FormulaLimits,
    ) -> Result<Vec<FormulaCheck>, GpuError> {
        self.propagate_batch_with_cancellation(theory, candidates, limits, &Cancellation::default())
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
    pub fn propagate_batch_with_cancellation(
        &mut self,
        theory: &Theory,
        candidates: &[Interpretation],
        limits: FormulaLimits,
        cancellation: &Cancellation,
    ) -> Result<Vec<FormulaCheck>, GpuError> {
        self.propagate(
            theory,
            candidates,
            limits,
            cancellation,
            crate::candidates::BatchSize::Exact(candidates.len()),
        )
    }

    /// Check the largest nonempty prefix admitted by the prepared device layout.
    ///
    /// The prefix is bounded by both the supplied slice and `max_candidates`.
    /// Cold graph packing occurs once; admission then selects a count using the
    /// same storage, address and work checks as the exact batch operation. The
    /// receipt borrows exactly the checked prefix and retains input order.
    /// An empty slice still observes control and context health before returning.
    /// No completed prefix is returned after an interrupted or failed operation.
    ///
    /// # Errors
    /// Preserves [`Self::propagate_batch_with_cancellation`]'s typed failures.
    /// If one candidate cannot fit, returns that refusal without dispatch. Only
    /// capacity limits reduce the selected count; allocation and device failures
    /// do not select a fallback. The chosen schedule is not changed to enlarge
    /// the prefix. Subsequent calls may reuse the prepared subject.
    pub fn propagate_prefix_with_cancellation<'a>(
        &mut self,
        theory: &'a Theory,
        candidates: &'a [Interpretation],
        limits: FormulaLimits,
        cancellation: &Cancellation,
    ) -> Result<crate::CheckedBatch<'a, FormulaCheck>, GpuError> {
        let checks = self.propagate(
            theory,
            candidates,
            limits,
            cancellation,
            crate::candidates::BatchSize::Prefix(candidates.len()),
        )?;
        Ok(crate::CheckedBatch::new(
            theory,
            &candidates[..checks.len()],
            checks,
        ))
    }

    fn propagate(
        &mut self,
        theory: &Theory,
        candidates: &[Interpretation],
        limits: FormulaLimits,
        cancellation: &Cancellation,
        size: crate::candidates::BatchSize,
    ) -> Result<Vec<FormulaCheck>, GpuError> {
        self.last = None;
        self.last_submission_candidates = None;
        self.reported = None;
        let context = self.profile.runtime.context.clone();
        let _lease = context.lease()?;
        cancellation.poll().map_err(GpuError::interrupted)?;
        self.profile.runtime.check_health()?;
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        let epoch = self.epoch.checked_add(1).ok_or_else(|| {
            GpuError::new(GpuErrorKind::Capacity, "formula epoch counter exhausted")
        })?;
        let PreparedCall { fresh, plan, stats } =
            self.prepare(theory, candidates, limits, cancellation, epoch, size)?;
        cancellation.poll().map_err(GpuError::interrupted)?;
        let scopes = ErrorScopes::new(self.profile.runtime.device());
        if let Some(prepared) = fresh {
            self.resident = Some(Resident::new(self.profile.runtime.device(), prepared));
        }
        let status = context.compute_timing();
        if status == ComputeTimingStatus::Enabled && self.timer.is_none() {
            self.timer = Some(ComputeTimer::new(self.profile.runtime.device()));
        }
        self.epoch = epoch;
        let dispatched = self
            .resident
            .as_mut()
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing resident formula"))
            .and_then(|resident| {
                resident.dispatch(
                    &self.profile.runtime,
                    self.candidates.words(),
                    &plan,
                    limits.timeout,
                    cancellation,
                    self.timer.as_ref(),
                )
            });
        let (outcome, timing) = match dispatched {
            Ok(dispatched) => {
                self.last_submission_candidates = Some(dispatched.candidates);
                (Ok(dispatched.completion), dispatched.timing)
            }
            Err(error) => (Err(error), BatchTiming::default()),
        };
        let checks = self.profile.runtime.complete(scopes, outcome);
        if self.last_submission_candidates.is_some() {
            self.reported = crate::timing::settle_call(
                status,
                &mut self.timer,
                checks.is_ok(),
                &timing,
                self.profile.runtime.queue().get_timestamp_period(),
            );
        }
        let checks = checks?;
        self.last = Some(stats);
        Ok(checks)
    }

    // All cold staging is owned and admitted before device effects. Minimum
    // shape/identity refusals preserve residency; later cold failures leave a
    // healthy context without a resident subject or a committed epoch.
    fn prepare(
        &mut self,
        theory: &Theory,
        candidates: &[Interpretation],
        mut limits: FormulaLimits,
        cancellation: &Cancellation,
        epoch: u32,
        size: crate::candidates::BatchSize,
    ) -> Result<PreparedCall, GpuError> {
        let timed =
            self.context().compute_timing() == zetesis_backend::ComputeTimingStatus::Enabled;
        let timing_bytes = if timed {
            crate::timing::OVERHEAD_BYTES
        } else {
            0
        };
        limits.max_batch_bytes = limits
            .max_batch_bytes
            .checked_sub(timing_bytes)
            .ok_or_else(|| {
                GpuError::new(
                    GpuErrorKind::Capacity,
                    "formula timestamp storage exceeds the batch byte allowance",
                )
            })?;
        // Old scratch must not coexist with a cold graph's temporary directory.
        // Releasing it does not change the resident device subject on refusal.
        if self.resident.as_ref().is_none_or(|resident| {
            !resident.graph.shape.theory.same_instance(theory)
                || (size.is_exact() && !resident.matches_count(candidates.len()))
        }) {
            self.candidates.release();
        }
        let retained = self.retained_plan(theory, size, limits, epoch, timed, cancellation)?;
        let (fresh, plan) = if let Some(plan) = retained {
            identities(theory, candidates)?;
            (None, plan)
        } else {
            self.candidates.release();
            let preparation = Preparation::with_control(
                theory,
                size.initial(),
                limits,
                self.profile.runtime.limits(),
                epoch,
                cancellation,
            )?
            .with_propagation(self.profile.kernel_options().propagation)
            .with_workgroup_sizing(self.profile.kernel_options().workgroup_sizing);
            identities(theory, candidates)?;
            self.resident = None;
            let (prepared, exact) =
                preparation.finish(self.profile.runtime.limits(), cancellation)?;
            let plan = if size.is_exact() {
                exact
            } else {
                size.select(limits.max_candidates, cancellation, |count| {
                    let plan =
                        prepared.plan(count, limits, self.profile.runtime.limits(), epoch)?;
                    crate::timing::readback_size(
                        plan.results,
                        timed,
                        self.profile.runtime.limits().max_buffer_size,
                    )?;
                    Ok(plan)
                })?
            };
            (Some(prepared), plan)
        };
        crate::timing::readback_size(
            plan.results,
            timed,
            self.profile.runtime.limits().max_buffer_size,
        )?;
        self.stage_candidates(fresh, plan, candidates, limits, timing_bytes, cancellation)
    }

    fn stage_candidates(
        &mut self,
        fresh: Option<PreparedGraph>,
        plan: Plan,
        candidates: &[Interpretation],
        limits: FormulaLimits,
        timing_bytes: u64,
        cancellation: &Cancellation,
    ) -> Result<PreparedCall, GpuError> {
        let graph = fresh
            .as_ref()
            .map(|prepared| &prepared.graph)
            .or_else(|| self.resident.as_ref().map(|resident| &resident.graph))
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing formula graph"))?;
        let mut stats = FormulaBatchStats {
            propagation: graph.schedule.propagation(),
            theory_uploaded: fresh.is_some(),
            transport_allocated: fresh.is_some()
                || self
                    .resident
                    .as_ref()
                    .is_none_or(|resident| !resident.matches(&plan)),
            resident_theory_bytes: graph.schedule.bytes,
            // The timer is the oracle's; only the readback tail is transport.
            resident_transport_bytes: plan.transport
                + if timing_bytes == 0 {
                    0
                } else {
                    crate::timing::READBACK_BYTES
                },
            accounted_bytes: plan.accounted + timing_bytes,
            workgroup_size: graph.workgroup_size.invocations(),
        };
        if stats.transport_allocated {
            self.candidates.release();
            if let Some(resident) = self.resident.as_mut() {
                resident.transport = None;
            }
        }
        let graph = fresh
            .as_ref()
            .map(|prepared| &prepared.graph)
            .or_else(|| self.resident.as_ref().map(|resident| &resident.graph))
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing formula packing graph"))?;
        stats.accounted_bytes = plan.pack(
            graph,
            &candidates[..plan.worlds as usize],
            &mut self.candidates,
            limits.max_batch_bytes,
            cancellation,
        )? + timing_bytes;
        Ok(PreparedCall { fresh, plan, stats })
    }

    // Optional indexing must not turn a smaller caller allowance into a new
    // mandatory representation. A capacity refusal re-enters cold admission;
    // only after its minimum and identities pass is the old resident retired.
    fn retained_plan(
        &self,
        theory: &Theory,
        size: crate::candidates::BatchSize,
        limits: FormulaLimits,
        epoch: u32,
        timed: bool,
        cancellation: &Cancellation,
    ) -> Result<Option<Plan>, GpuError> {
        let Some(resident) = self
            .resident
            .as_ref()
            .filter(|resident| resident.graph.shape.theory.same_instance(theory))
        else {
            return Ok(None);
        };
        match size.select(limits.max_candidates, cancellation, |count| {
            let plan = Plan::new(
                &resident.graph,
                count,
                limits,
                self.profile.runtime.limits(),
                false,
                epoch,
            )?;
            crate::timing::readback_size(
                plan.results,
                timed,
                self.profile.runtime.limits().max_buffer_size,
            )?;
            Ok(plan)
        }) {
            Ok(plan) => Ok(Some(plan)),
            Err(error)
                if resident.graph.schedule.incidence.is_some()
                    && error.kind() == GpuErrorKind::Capacity =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

fn identities(theory: &Theory, candidates: &[Interpretation]) -> Result<(), GpuError> {
    if candidates
        .iter()
        .any(|candidate| !theory.same_instance(candidate.theory()))
    {
        return Err(GpuError::new(
            GpuErrorKind::Seed,
            "candidate belongs to another Theory",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
