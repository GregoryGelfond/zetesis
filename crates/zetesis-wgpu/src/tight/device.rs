//! Device ownership for a separately checked complete-theory certificate.

use super::admission::Admission;
use super::packing::{Graph, Packed, Packing, Plan, StateLayout};
use super::transport::{Execution, Resident};
use super::update::{self, Bounds, Built, Index, Stage};
use super::{
    SHADER, TightGpuActivity, TightGpuBatchStats, TightGpuCheck, TightGpuError, TightGpuLimits,
    TightGpuUpdateUnavailable, TightGpuUpdates, TightSupport, poll,
};
use crate::runtime::{DeviceProfile, ErrorScopes, Runtime};
use crate::timing::{BatchTiming, ComputeTimer};
use crate::{GpuError, GpuErrorKind, GpuInfo, GpuOptions, GpuSelection};
use std::time::Duration;
use zetesis_backend::{BatchComputeTime, ComputeTimingStatus};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, TightPlan};

/// Real-device original satisfaction and ranked producer support checking.
///
/// Takes a complete opaque [`TightPlan`]; never constructs an unchecked rank or
/// relies on an incomplete lazy registry. Candidate generation, objectives and
/// any witness-producing residual completion remain the caller's responsibilities.
/// A complete failed support scan also permits proof-based rejection by the
/// support-necessity theorem; it publishes no concrete deletion witness.
/// Every nonempty successful call executes the support kernel on the selected
/// adapter. Ordinary solver dispatch selects this independent primitive only
/// after preparing its complete certificate; no CPU fallback occurs here.
///
/// Native threshold and extremum-cut rows remain shared condition/value columns
/// in the immutable transport; both original-truth checkers use the same portable
/// limb arithmetic. No Boolean numeric expansion or optional-width feature is
/// required. Work is linear in candidates times nodes, row occurrences, roots,
/// producers and atoms. Each workgroup evaluates independent nodes of each
/// dependency level across its workgroup size of invocations, by default the
/// size the theory's schedule fits (see [`crate::GpuWorkgroupSizing`]), then
/// shares root/support scans.
/// Pure chains and cold preparations without room for the additional schedule
/// retain serial original-truth evaluation. Truth uses one word per node; candidate membership and
/// producer support use one bit per atom, in separate world-major arrays. Heads
/// sharing a support word use atomic OR by default. Grouped support instead
/// visits each word's producers sequentially in one invocation; this trades
/// contention for an index and potentially uneven parallel work.
///
/// Successive batches of one theory and count may also update retained
/// evaluations (see [`TightGpuLimits::min_update_candidates`]). The first such
/// batch appends a readers index to the producer storage: for each node the
/// connectives and numeric-row owners reading it, the heads of the producers
/// whose body it is and the root positions holding it, and for each atom its
/// atom nodes. Each candidate slot then retains the candidate it evaluated, its
/// support counts per head, and its false-root and unsupported-atom sets, and
/// re-evaluates in dependency-level order only the nodes a new candidate's
/// changed atoms reach.
/// Shared graph and transport storage have explicit byte bounds. The oracle
/// retains one shared theory handle and one exact transport
/// shape, without cloning the caller's certificate or storing its ranks.
pub struct GpuTightOracle {
    runtime: Runtime,
    support: TightSupport,
    resident: Option<Resident>,
    candidates: crate::candidates::Staging,
    epoch: u32,
    last: Option<TightGpuBatchStats>,
    activity: TightGpuActivity,
    // Timestamp storage and pair ledger, from the first timed batch until the
    // residency is cleared; transports and theories come and go beneath it.
    timer: Option<ComputeTimer>,
    reported: Option<BatchComputeTime>,
}

struct PreparedCall {
    fresh: Option<(Graph, Packed)>,
    plan: Plan,
    stats: TightGpuBatchStats,
    /// A readers index to install before dispatch.
    build: Option<Built>,
}

/// The update route's per-batch admission before the dense charges are known.
enum Updates {
    /// No state is bound; the route is unavailable or not yet possible.
    Stateless(TightGpuUpdates),
    /// A state-inclusive plan was selected under `index`, which `build`
    /// installs first when this batch builds it.
    State { index: Index, build: Option<Built> },
}

impl GpuTightOracle {
    /// Create an independently owned device and kernel using hard adapter
    /// filters. The default fitted sizing compiles a size's pipeline at the
    /// first check that dispatches it. Neither construction nor that
    /// compilation is charged to per-batch limits.
    ///
    /// # Errors
    /// Returns typed adapter, capability, allocation, validation or device errors.
    pub fn new_selected(options: GpuOptions, selection: GpuSelection) -> Result<Self, GpuError> {
        Self::new_with_support(options, selection, TightSupport::Atomic)
    }

    /// Select a device with optional compute-pass timing, using the same tight
    /// capability requirements and adapter policy as [`Self::new_selected`].
    /// Unsupported timestamps leave solving available without a device sample.
    ///
    /// # Errors
    /// Returns the same typed construction failures as [`Self::new_selected`].
    pub fn new_selected_with_timing(
        options: GpuOptions,
        selection: GpuSelection,
    ) -> Result<Self, GpuError> {
        let support = TightSupport::Atomic;
        let runtime = pollster::block_on(Runtime::new_with_timing(
            options,
            selection,
            Self::profile(support),
            true,
        ))?;
        Ok(Self::with_runtime(runtime, support))
    }

    /// Create a device with an explicit producer-support construction policy.
    /// The policy remains fixed for the oracle's lifetime. Grouped construction
    /// retains one offset per support word plus an end offset; its temporary
    /// packing cursors are included in fresh-batch byte admission.
    /// Each complete device record identifies the construction branch. The
    /// decoder refuses a marker for the other policy, even if its verdict agrees.
    /// This is protocol validation, not a proof of shader or hardware correctness.
    ///
    /// # Errors
    /// Returns the same typed construction failures as [`Self::new_selected`].
    pub fn new_with_support(
        options: GpuOptions,
        selection: GpuSelection,
        support: TightSupport,
    ) -> Result<Self, GpuError> {
        let runtime = pollster::block_on(Runtime::new(options, selection, Self::profile(support)))?;
        Ok(Self::with_runtime(runtime, support))
    }

    /// Compile this primitive's shader on an existing device context; the
    /// default fitted sizing compiles a size's pipeline at the first check that
    /// dispatches it.
    ///
    /// Prepared subjects belong to this primitive; device health and execution
    /// serialization are shared with all primitives using the context.
    ///
    /// # Errors
    /// Refuses an active or invalidated context, unsupported granted limits,
    /// allocation and shader validation failure. No adapter selection occurs.
    pub fn from_context(context: &crate::GpuContext) -> Result<Self, GpuError> {
        Self::from_context_with_support(context, TightSupport::Atomic)
    }

    /// Compile the selected implementation's shader on an existing shared context.
    ///
    /// # Errors
    /// Reports the same context, capability and compilation failures as
    /// [`Self::from_context`].
    pub(crate) fn from_context_with_support(
        context: &crate::GpuContext,
        support: TightSupport,
    ) -> Result<Self, GpuError> {
        let runtime = pollster::block_on(Runtime::from_context(context, Self::profile(support)))?;
        Ok(Self::with_runtime(runtime, support))
    }

    /// Exact device context retained by this primitive.
    #[must_use]
    pub fn context(&self) -> &crate::GpuContext {
        &self.runtime.context
    }

    /// Compile the shader of explicit physical choices on an existing context.
    ///
    /// The original-truth and support contracts are unchanged. Optional integer
    /// instructions require granted features and retain exact wide arithmetic.
    ///
    /// # Errors
    /// Reports the failures of [`Self::from_context`], including a fixed
    /// workgroup size exceeding the granted limits, and a failure to create the
    /// shader or compile a fixed size, which invalidates the context; a fitted
    /// size's compilation failure is reported by the first check that needs it.
    /// No different device or support policy is substituted.
    pub fn from_context_with_kernel(
        context: &crate::GpuContext,
        support: TightSupport,
        options: crate::GpuKernelOptions,
    ) -> Result<Self, GpuError> {
        let mut profile = Self::profile(support);
        profile.kernel = crate::kernel::Kernel::Tight(options);
        let runtime = pollster::block_on(Runtime::from_context(context, profile))?;
        Ok(Self::with_runtime(runtime, support))
    }

    /// The workgroup sizes this oracle's runtime has compiled.
    #[cfg(test)]
    pub(crate) fn compiled_sizes(&self) -> Vec<crate::GpuWorkgroupSize> {
        self.runtime.compiled_sizes()
    }

    /// Physical options retained by this kernel; no device operation.
    ///
    /// # Panics
    /// Panics if the internal runtime is not a tight kernel. Every public
    /// constructor establishes this invariant; callers cannot replace it.
    #[must_use]
    pub fn kernel_options(&self) -> crate::GpuKernelOptions {
        self.runtime.kernel.options().expect("tight kernel")
    }

    fn with_runtime(runtime: Runtime, support: TightSupport) -> Self {
        Self {
            runtime,
            support,
            resident: None,
            candidates: crate::candidates::Staging::default(),
            epoch: 0,
            last: None,
            activity: TightGpuActivity::default(),
            timer: None,
            reported: None,
        }
    }

    fn profile(support: TightSupport) -> DeviceProfile {
        DeviceProfile {
            device_label: "zetesis tight support device",
            shader_label: "ranked producer support",
            pipeline_label: "complete original support",
            shader: SHADER.into(),
            entry_point: match support {
                TightSupport::Atomic => "check",
                TightSupport::Grouped => "check_grouped",
            },
            validate_limits: check_limits,
            kernel: crate::kernel::Kernel::Tight(crate::GpuKernelOptions::default()),
        }
    }
    /// Fixed physical support-construction policy; no device operation.
    #[must_use]
    pub const fn support(&self) -> TightSupport {
        self.support
    }
    /// Actual selected native adapter identity; no device operation.
    #[must_use]
    pub fn info(&self) -> &GpuInfo {
        self.runtime.context.info()
    }
    /// Successful nonempty batch accounting; cleared at every check start.
    #[must_use]
    pub fn last_batch_stats(&self) -> Option<&TightGpuBatchStats> {
        self.last.as_ref()
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
    /// zero and excludes host packing, uploads, readback and waits.
    #[must_use]
    pub const fn reported_compute_time(&self) -> Option<BatchComputeTime> {
        self.reported
    }
    /// Whether a completed batch still waits for its interval to be resolved by
    /// the next submitting call or by [`Self::flush_compute_time`].
    #[must_use]
    pub fn compute_time_unresolved(&self) -> bool {
        self.timer.as_ref().is_some_and(ComputeTimer::unresolved)
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
    /// downloads 16 bytes; batch statistics and [`Self::activity`] exclude it.
    ///
    /// # Errors
    /// Returns the refusals and failures above.
    pub fn flush_compute_time(
        &mut self,
        timeout: Duration,
        cancellation: &Cancellation,
    ) -> Result<(), GpuError> {
        self.reported = None;
        let Some(timer) = self.timer.as_mut().filter(|timer| timer.unresolved()) else {
            return Ok(());
        };
        crate::runtime::positive_timeout(timeout)?;
        let runtime = &self.runtime;
        let _lease = runtime.context.lease()?;
        poll(cancellation)?;
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
    /// Submitted and validated work in the latest attempt, including failure.
    /// Absence of completion never establishes that submitted work did not run.
    #[must_use]
    pub const fn activity(&self) -> TightGpuActivity {
        self.activity
    }
    /// Drop authored resident handles, the compute timer and successful-batch
    /// statistics. Preserve latest-attempt activity. Does not cancel work, reset
    /// the epoch, or repair a device invalidated by timeout, control interruption
    /// or execution failure. A batch still unresolved is reported unavailable,
    /// since it can never be resolved; the next timed batch starts a fresh timer.
    pub fn clear_residency(&mut self) {
        self.resident = None;
        self.candidates.release();
        self.last = None;
        self.reported = crate::timing::retire(&mut self.timer);
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
    /// Original truth may reuse unchanged dependencies from a wholly successful
    /// prior batch with that same subject/count. Every root and producer
    /// remains covered: the complete and dense routes check each of them, and
    /// the update route maintains their support counts and the false-root and
    /// unsupported sets. Both reuse routes are optional: change detection runs
    /// only when its work allowance fits, otherwise the complete evaluation runs
    /// without new storage; the update route runs only on a schedule wide enough
    /// to engage it and when its per-slot state and its reservation are
    /// admitted, otherwise the dense routes run. A later
    /// call that cannot admit a held state releases it and keeps its truth and
    /// result authority; a built readers index stays resident and accounted,
    /// like a retained schedule.
    /// Host upload words reuse initialized scratch for that same subject/count.
    /// Their actual capacity is charged, and an oversized optional allocation is
    /// replaced when a smaller allowance fits fresh storage. Replacement drops
    /// old scratch before cold preparation; explicit clearing releases it too.
    ///
    /// # Errors
    /// Returns no partial result vector on foreign identity, capacity, work,
    /// allocation, cancellation, deadline, validation, device or readback failure.
    /// Host preparation refusals leave the oracle reusable. Failures after device
    /// allocation/submission invalidate the entire context, and a fresh context is
    /// required before retrying, except a cancellation or deadline stop whose
    /// submission was observed to complete and whose readback was mapped and
    /// released: that leaves the context reusable, as [`crate::GpuContext`]
    /// documents. Busy contention submits no work and permits retry.
    /// Cancellation is polled during packing, waiting, decoding and before returning.
    /// A tighter work limit can bypass a resident dependency schedule. Its bytes
    /// remain charged: a smaller byte limit may refuse that retained layout.
    /// Requested device timing charges 96 bytes to the same batch allowance: the
    /// oracle's query slots, two resolve buffers and flush readback, and the
    /// readback tail of the batch's transport. It never changes logical verdicts.
    /// Under fitted sizing, the first call that dispatches a size compiles its
    /// pipeline. A failure to compile it is reported before any dispatch and
    /// invalidates the context; the size stays uncompiled, and no theory stays
    /// resident, because only a freshly prepared theory can need a size first
    /// and its cold preparation released the previous one.
    pub fn check_batch(
        &mut self,
        certificate: &TightPlan,
        candidates: &[Interpretation],
        limits: TightGpuLimits,
        cancellation: &Cancellation,
    ) -> Result<Vec<TightGpuCheck>, TightGpuError> {
        self.last = None;
        self.reported = None;
        self.activity = TightGpuActivity::default();
        self.check(
            certificate,
            candidates,
            limits,
            cancellation,
            crate::candidates::BatchSize::Exact(candidates.len()),
        )
        .map_err(Into::into)
    }

    /// Check the largest nonempty prefix admitted by the prepared tight layout.
    ///
    /// The input slice and `max_candidates` both cap the count. Cold certificate
    /// packing occurs once, then the existing checked layout selects the largest
    /// count for that schedule. The returned receipt borrows exactly the checked
    /// prefix in its original order. Empty input still observes control and
    /// context health. Statistics and submission activity name the actual prefix.
    ///
    /// # Errors
    /// Preserves [`Self::check_batch`]'s typed failures and returns no partial
    /// checks. A one-candidate capacity or mandatory-work refusal remains an
    /// error. Allocation, cancellation and device faults never authorize retry
    /// at a smaller count or CPU fallback.
    pub fn check_prefix<'a>(
        &mut self,
        certificate: &'a TightPlan,
        candidates: &'a [Interpretation],
        limits: TightGpuLimits,
        cancellation: &Cancellation,
    ) -> Result<crate::CheckedBatch<'a, TightGpuCheck>, TightGpuError> {
        self.last = None;
        self.activity = TightGpuActivity::default();
        self.reported = None;
        let checks = self.check(
            certificate,
            candidates,
            limits,
            cancellation,
            crate::candidates::BatchSize::Prefix(candidates.len()),
        )?;
        Ok(crate::CheckedBatch::new(
            certificate.theory(),
            &candidates[..checks.len()],
            checks,
        ))
    }

    fn check(
        &mut self,
        certificate: &TightPlan,
        candidates: &[Interpretation],
        limits: TightGpuLimits,
        cancellation: &Cancellation,
        size: crate::candidates::BatchSize,
    ) -> Result<Vec<TightGpuCheck>, GpuError> {
        let result = self.check_inner(certificate, candidates, limits, cancellation, size);
        if result.is_err()
            && let Some(resident) = self.resident.as_mut()
        {
            resident.invalidate_original_truth();
        }
        result
    }

    fn check_inner(
        &mut self,
        certificate: &TightPlan,
        candidates: &[Interpretation],
        limits: TightGpuLimits,
        cancellation: &Cancellation,
        size: crate::candidates::BatchSize,
    ) -> Result<Vec<TightGpuCheck>, GpuError> {
        let context = self.runtime.context.clone();
        let _lease = context.lease()?;
        poll(cancellation)?;
        self.runtime.check_health()?;
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        let PreparedCall {
            fresh,
            plan,
            mut stats,
            build,
        } = self.prepare(certificate, candidates, limits, cancellation, size)?;
        poll(cancellation)?;
        // The dispatched size's pipeline is compiled at first need, before this
        // call's scopes and before the fresh graph becomes resident. Only a
        // freshly prepared graph can need a size first, and its preparation has
        // released any previous resident, so a refusal leaves none, as a
        // cold-preparation failure does.
        let workgroup_size = fresh
            .as_ref()
            .map(|(graph, _)| graph.workgroup_size)
            .or_else(|| {
                self.resident
                    .as_ref()
                    .map(|resident| resident.graph.workgroup_size)
            })
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing resident tight graph"))?;
        self.runtime.ensure_pipeline(workgroup_size)?;
        let scopes = ErrorScopes::new(self.runtime.device());
        if let Some((mut graph, packed)) = fresh {
            graph.rows.release();
            self.activity.uploaded_bytes = graph.bytes;
            self.resident = Some(Resident::new(self.runtime.device(), graph, &packed));
        }
        if let Some(built) = build
            && let Some(resident) = self.resident.as_mut()
        {
            let bytes = resident.install_index(self.runtime.device(), &built);
            self.activity.uploaded_bytes += bytes;
            stats.uploaded_bytes += bytes;
            stats.resident_theory_bytes = resident.graph.bytes;
        }
        let status = context.compute_timing();
        if status == ComputeTimingStatus::Enabled && self.timer.is_none() {
            self.timer = Some(ComputeTimer::new(self.runtime.device()));
        }
        self.epoch = plan.epoch;
        let mut timing = BatchTiming::default();
        let outcome = self
            .resident
            .as_mut()
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing resident tight graph"))
            .and_then(|resident| {
                let pipeline = self.runtime.pipeline_for(resident.graph.workgroup_size)?;
                resident.dispatch(
                    &Execution {
                        device: self.runtime.device(),
                        queue: self.runtime.queue(),
                        pipeline,
                        timeout: limits.timeout,
                        cancellation,
                        timer: self.timer.as_ref(),
                    },
                    self.candidates.words(),
                    &plan,
                    &mut self.activity,
                    &mut timing,
                )
            });
        let result = self.runtime.complete(scopes, outcome).and_then(|decoded| {
            poll(cancellation)?;
            Ok(decoded)
        });
        if self.activity.submissions != 0 {
            self.reported = crate::timing::settle_call(
                status,
                &mut self.timer,
                result.is_ok(),
                &timing,
                self.runtime.queue().get_timestamp_period(),
            );
        }
        let decoded = result?;
        if let Some(resident) = self.resident.as_mut() {
            resident.complete_original_truth();
        }
        stats.routes = decoded.routes;
        stats.nodes_evaluated = decoded.nodes_evaluated;
        stats.work = decoded.work;
        self.last = Some(stats);
        Ok(decoded.checks)
    }

    fn prepare(
        &mut self,
        certificate: &TightPlan,
        candidates: &[Interpretation],
        limits: TightGpuLimits,
        cancellation: &Cancellation,
        size: crate::candidates::BatchSize,
    ) -> Result<PreparedCall, GpuError> {
        let timed = self
            .runtime
            .device()
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY);
        let (admitted_limits, timing_bytes) = reserve_timing(limits, timed)?;
        // Admission can prepare a numeric-row directory. Drop unrelated host
        // scratch first while leaving the existing device-refusal policy intact.
        if self.resident.as_ref().is_none_or(|resident| {
            !resident.graph.theory.same_instance(certificate.theory())
                || (size.is_exact() && !resident.matches_count(candidates.len()))
        }) {
            self.candidates.release();
        }
        let mut admission = Admission::new(
            certificate,
            &candidates[..size.initial()],
            admitted_limits,
            Packing {
                device: self.runtime.limits(),
                support: self.support,
                workgroup_sizing: self.kernel_options().workgroup_sizing,
            },
            self.resident
                .as_ref()
                .map(|resident| (&resident.graph, resident.matches_count(candidates.len()))),
            self.epoch,
            cancellation,
        )?;
        // Evict authored buffers before replacing them; deferred driver retirement
        // is explicitly outside the logical payload budget.
        crate::timing::readback_size(
            admission.plan.results,
            timed,
            self.runtime.limits().max_buffer_size,
        )?;
        if admission.fresh.is_some() {
            self.resident = None;
        }
        let packed = admission.pack(
            certificate,
            admitted_limits,
            self.runtime.limits(),
            cancellation,
        )?;
        self.select_prefix(&mut admission, size, admitted_limits, timed, cancellation)?;
        let updates = self.admit_updates(
            &mut admission,
            certificate,
            size,
            admitted_limits,
            timed,
            cancellation,
        )?;
        let Admission {
            fresh,
            mut plan,
            mut stats,
        } = admission;
        if stats.transport_allocated {
            self.candidates.release();
            if let Some(resident) = self.resident.as_mut() {
                resident.transport = None;
            }
        }
        let graph = fresh
            .as_ref()
            .or_else(|| self.resident.as_ref().map(|r| &r.graph))
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing tight packing graph"))?;
        if self
            .resident
            .as_ref()
            .is_some_and(Resident::original_truth_valid)
        {
            plan.reuse_original_truth(graph, admitted_limits.max_work_per_candidate);
        }
        let (availability, build) = bind_updates(
            &mut plan,
            graph,
            updates,
            admitted_limits.max_work_per_candidate,
        );
        planned_stats(&mut stats, &plan, availability, timed);
        stats.accounted_bytes = plan.pack(
            graph,
            &candidates[..plan.worlds as usize],
            &mut self.candidates,
            admitted_limits.max_batch_bytes,
            cancellation,
        )? + timing_bytes;
        Ok(PreparedCall {
            fresh: fresh.zip(packed),
            plan,
            stats,
            build,
        })
    }

    /// Decide the update route's state for this batch, after the stateless
    /// prefix is selected. A fresh theory, a declined route, a serial or narrow
    /// schedule, a prefix call offering fewer than `min_update_candidates`
    /// candidates and an index refused by its bounds bind no state. Without an
    /// index, one is built only at a batch holding reuse authority at the
    /// stateless count, and only when the route's reservation fits the work
    /// allowance. A batch then selects its layout with each slot's state: an
    /// exact call only when the whole count fits, a prefix call only when at
    /// least `min_update_candidates` fit. Otherwise the stateless plan stands,
    /// and a transport holding state releases it while keeping its truth and
    /// result authority; the next batch admitting the state allocates it
    /// zeroed.
    fn admit_updates(
        &mut self,
        admission: &mut Admission,
        certificate: &TightPlan,
        size: crate::candidates::BatchSize,
        limits: TightGpuLimits,
        timed: bool,
        cancellation: &Cancellation,
    ) -> Result<Updates, GpuError> {
        let stateless = |availability| Ok(Updates::Stateless(availability));
        let (None, Some(resident)) = (&admission.fresh, self.resident.as_mut()) else {
            return stateless(TightGpuUpdates::Pending);
        };
        if let Some(reason) = declined(resident.updates, &admission.plan, size, limits) {
            return stateless(TightGpuUpdates::Unavailable(reason));
        }
        let device = self.runtime.limits();
        let (layout, bounds) = match state_layout(resident, &admission.plan, device) {
            Ok(layout) => layout,
            Err(availability) => return stateless(availability),
        };
        let graph = &resident.graph;
        if let Some(bounds) = bounds
            && !admits_work(
                graph,
                admission.plan.levels,
                &bounds,
                limits.max_work_per_candidate,
            )
        {
            return stateless(TightGpuUpdates::Unavailable(
                TightGpuUpdateUnavailable::Work,
            ));
        }
        let epoch = admission.plan.epoch;
        let layout_plan = |count: usize| {
            let plan = Plan::with_state(graph, count, limits, device, false, epoch, Some(layout))?;
            crate::timing::readback_size(plan.results, timed, device.max_buffer_size)?;
            Ok(plan)
        };
        let selected = if size.is_exact() {
            layout_plan(admission.plan.worlds as usize)
        } else {
            size.select(limits.max_candidates, cancellation, layout_plan)
        };
        let plan = match selected {
            Ok(plan) if size.is_exact() || plan.worlds as usize >= limits.min_update_candidates => {
                plan
            }
            Err(error) if error.kind() != GpuErrorKind::Capacity => return Err(error),
            _ => {
                return stateless(TightGpuUpdates::Unavailable(
                    TightGpuUpdateUnavailable::Bytes,
                ));
            }
        };
        let (index, build) = match (resident.updates, bounds) {
            (Stage::Built(index), _) => (index, None),
            (_, Some(bounds)) => {
                let records = graph.pack_producers(certificate, cancellation)?;
                let built = update::build(
                    graph,
                    certificate,
                    &graph.owners,
                    bounds,
                    &records,
                    cancellation,
                )?;
                (built.index, Some(built))
            }
            _ => {
                return Err(GpuError::new(
                    GpuErrorKind::Device,
                    "missing tight update bounds",
                ));
            }
        };
        admission.stats = super::admission::batch_stats(
            graph,
            &plan,
            false,
            !resident.matches_count(plan.worlds as usize),
        );
        admission.plan = plan;
        Ok(Updates::State { index, build })
    }
    fn select_prefix(
        &self,
        admission: &mut Admission,
        size: crate::candidates::BatchSize,
        limits: TightGpuLimits,
        timed: bool,
        cancellation: &Cancellation,
    ) -> Result<(), GpuError> {
        if !size.is_exact() {
            let graph = admission
                .fresh
                .as_ref()
                .or_else(|| self.resident.as_ref().map(|resident| &resident.graph))
                .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing tight layout graph"))?;
            let plan = size.select(limits.max_candidates, cancellation, |count| {
                let plan = super::packing::Plan::new(
                    graph,
                    count,
                    limits,
                    self.runtime.limits(),
                    admission.fresh.is_some(),
                    admission.plan.epoch,
                )?;
                crate::timing::readback_size(
                    plan.results,
                    timed,
                    self.runtime.limits().max_buffer_size,
                )?;
                Ok(plan)
            })?;
            admission.stats = super::admission::batch_stats(
                graph,
                &plan,
                admission.fresh.is_some(),
                self.resident
                    .as_ref()
                    .is_none_or(|resident| !resident.matches_count(plan.worlds as usize)),
            );
            admission.plan = plan;
        }
        Ok(())
    }
}

/// Complete `stats` with what `plan` decides once its update state is bound:
/// its routes' reservation, the update route's availability, and its transport
/// and download, which under timing carry the 16-byte readback tail. The timer
/// is the oracle's, so its storage counts in the batch's bytes alone.
fn planned_stats(
    stats: &mut TightGpuBatchStats,
    plan: &Plan,
    availability: TightGpuUpdates,
    timed: bool,
) {
    let tail = if timed {
        crate::timing::READBACK_BYTES
    } else {
        0
    };
    stats.original_truth_reused = plan.original_truth_reused;
    stats.updates = availability;
    stats.resident_transport_bytes = plan.transport + tail;
    stats.downloaded_bytes += tail;
    stats.work = u64::from(plan.worlds) * u64::from(plan.allowance);
}

/// The reason a batch with this resident's `stage` and stateless `plan` cannot
/// use the update route, before any index or state is considered.
fn declined(
    stage: Stage,
    plan: &Plan,
    size: crate::candidates::BatchSize,
    limits: TightGpuLimits,
) -> Option<TightGpuUpdateUnavailable> {
    use TightGpuUpdateUnavailable::{
        CandidateCeiling, Declined, IndexBounds, NarrowSchedule, SerialSchedule,
    };
    if limits.min_update_candidates == 0 {
        return Some(Declined);
    }
    match stage {
        Stage::Serial => return Some(SerialSchedule),
        Stage::Narrow => return Some(NarrowSchedule),
        Stage::Refused => return Some(IndexBounds),
        Stage::Pending | Stage::Bounded(_) | Stage::Built(_) => {}
    }
    // A tighter work allowance can bypass a resident schedule.
    if plan.levels == 0 {
        return Some(SerialSchedule);
    }
    match size {
        crate::candidates::BatchSize::Prefix(offered)
            if offered.min(limits.max_candidates) < limits.min_update_candidates =>
        {
            Some(CandidateCeiling)
        }
        _ => None,
    }
}

/// Each slot's state layout and, before the index exists, its bounds. An index
/// is decided only at a batch holding reuse authority at the stateless count;
/// before one, the route is pending. A refusal by the bounds is kept, and the
/// owner column it no longer needs is released.
fn state_layout(
    resident: &mut Resident,
    plan: &Plan,
    device: &wgpu::Limits,
) -> Result<(StateLayout, Option<Bounds>), TightGpuUpdates> {
    if let Stage::Built(index) = resident.updates {
        let layout = StateLayout {
            slot_words: index.slot_words,
            index_bytes: 0,
            build_bytes: 0,
        };
        return Ok((layout, None));
    }
    if !(resident.original_truth_valid() && resident.matches_count(plan.worlds as usize)) {
        return Err(TightGpuUpdates::Pending);
    }
    let Some(bounds) = resident
        .updates
        .bounds(|| Bounds::new(&resident.graph, device))
    else {
        resident.graph.owners = Vec::new();
        resident.graph.owner_bytes = 0;
        return Err(TightGpuUpdates::Unavailable(
            TightGpuUpdateUnavailable::IndexBounds,
        ));
    };
    let layout = StateLayout {
        slot_words: bounds.slot_words,
        index_bytes: bounds.index_bytes(&resident.graph),
        build_bytes: bounds.build_bytes(&resident.graph),
    };
    Ok((layout, Some(bounds)))
}

/// Bind the batch's update state, once the dense charges are known.
fn bind_updates(
    plan: &mut Plan,
    graph: &Graph,
    updates: Updates,
    ceiling: u64,
) -> (TightGpuUpdates, Option<Built>) {
    match updates {
        Updates::Stateless(availability) => (availability, None),
        Updates::State { index, build } => {
            let availability = if plan.bind_state(graph, &index, ceiling) {
                TightGpuUpdates::Admitted {
                    count_bits: index.count_bits,
                    index_built: build.is_some(),
                }
            } else {
                TightGpuUpdates::Unavailable(TightGpuUpdateUnavailable::Work)
            };
            (availability, build)
        }
    }
}

/// Whether the update route's reservation fits: the larger of its worst case
/// at `bounds` and dense reuse with the count pass and the state's writes.
fn admits_work(graph: &Graph, levels: u32, bounds: &Bounds, ceiling: u64) -> bool {
    let dense = u64::from(graph.work)
        + u64::from(levels)
        + 2 * u64::from(graph.nodes)
        + u64::from(graph.edges)
        + 4
        + 3 * u64::from(graph.words)
        + 2;
    update::worst_case(
        graph,
        levels,
        bounds.count_bits,
        bounds.entries,
        bounds.atom_entries,
    )
    .is_some_and(|worst| worst.max(dense) <= ceiling.min(u64::from(u32::MAX)))
}

// Reserve optional observation storage before ordinary batch admission. This
// leaves graph/work policy unchanged and keeps all authored bytes in one limit.
fn reserve_timing(limits: TightGpuLimits, timed: bool) -> Result<(TightGpuLimits, u64), GpuError> {
    let bytes = if timed {
        crate::timing::OVERHEAD_BYTES
    } else {
        0
    };
    let admitted = TightGpuLimits {
        max_batch_bytes: limits.max_batch_bytes.checked_sub(bytes).ok_or_else(|| {
            GpuError::new(
                GpuErrorKind::Capacity,
                "tight timing exceeds batch byte ceiling",
            )
        })?,
        ..limits
    };
    Ok((admitted, bytes))
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
            40,
            limits.max_compute_workgroup_storage_size,
        ),
        (
            "storage bindings",
            8,
            limits.max_storage_buffers_per_shader_stage,
        ),
        (
            "uniform bindings",
            1,
            limits.max_uniform_buffers_per_shader_stage,
        ),
        ("bindings per group", 9, limits.max_bindings_per_bind_group),
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
mod tests;
