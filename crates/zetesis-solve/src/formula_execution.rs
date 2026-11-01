//! Formula membership execution with bounded proposal, check and commit batches.

use crate::ExecutionObservation as Event;
use crate::execution_observation::ExecutionSink;

use std::sync::Arc;
use zetesis_ferraris::Interpretation;
use zetesis_stable::{Incomplete, StableModels};

use crate::phase_timing::Recorder;
#[cfg(feature = "gpu")]
use crate::phase_timing::SolvePhase;
use crate::{Backend, ExecutionResources, SolveConfig, SolveError};

pub use crate::completion_accounting::CompletionAccounting;

/// Effective per-candidate device propagation limits; CPU search has separate units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormulaDeviceLimits {
    /// Charged original-truth, initialization and propagation work.
    pub work_per_candidate: u32,
    /// Maximum propagation sweeps after original-truth setup.
    pub rounds_per_candidate: u32,
}

impl From<&SolveConfig> for FormulaDeviceLimits {
    fn from(options: &SolveConfig) -> Self {
        Self {
            work_per_candidate: options.gpu_formula_work,
            rounds_per_candidate: options.gpu_formula_rounds,
        }
    }
}

/// Decoded general-device propagation outcomes that still require exact
/// membership. These precede CPU completion and commit: a later failure does
/// not erase them, and an unreturned device batch contributes no decoded reason.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FormulaResidualStatistics {
    /// A complete no-change sweep left the proper-subset query unresolved.
    pub fixed_point: u64,
    /// The configured full-sweep ceiling was reached.
    pub round_limit: u64,
    /// The next full sweep could not fit its device work allowance.
    pub work_limit: u64,
}

/// Decoded tight-support device slots by the route that evaluated them, summed
/// over the decoded batches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct TightRouteStatistics {
    /// Slots without reuse authority: complete evaluation.
    pub complete: u64,
    /// Slots that reused a retained evaluation and re-evaluated every node.
    pub dense_reuse: u64,
    /// Dense-reuse slots that first counted their change and found it beyond
    /// the update route's threshold; included in `dense_reuse`.
    pub beyond_threshold: u64,
    /// Slots that updated a retained evaluation to their new candidate.
    pub update: u64,
    /// Nodes the update slots re-evaluated.
    pub nodes_evaluated: u64,
}

/// Whether the tight-support update route was available to a batch, and why
/// not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum TightUpdateAvailability {
    /// No decoded batch has held the reuse authority the route needs before
    /// it builds the theory's readers index, and the last decoded batch's
    /// schedule and limits did not already decline the route.
    #[default]
    Pending,
    /// The per-slot state was bound and the route allowed.
    Admitted,
    /// Original truth was evaluated serially.
    SerialSchedule,
    /// Dense evaluation takes fewer workgroup strides per level than the
    /// route's engagement factor, below which it is not expected to pay.
    NarrowSchedule,
    /// The batch offered fewer candidates than the batch size.
    CandidateCeiling,
    /// The theory exceeds the readers index's intrinsic bounds.
    IndexBounds,
    /// The state-inclusive layout did not fit the batch's byte allowance.
    Bytes,
    /// The route's reservation did not fit the device work allowance.
    Work,
    /// The route was declined.
    Declined,
    /// The device oracle's runtime guard declined the route for the batch,
    /// after a recent batch's slowest update path exceeded dense
    /// evaluation's.
    GuardDeclined,
}

impl TightUpdateAvailability {
    /// The serialized name: `pending`, `admitted`, `serial_schedule`,
    /// `narrow_schedule`, `candidate_ceiling`, `index_bounds`, `bytes`, `work`,
    /// `declined` or `guard_declined`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Admitted => "admitted",
            Self::SerialSchedule => "serial_schedule",
            Self::NarrowSchedule => "narrow_schedule",
            Self::CandidateCeiling => "candidate_ceiling",
            Self::IndexBounds => "index_bounds",
            Self::Bytes => "bytes",
            Self::Work => "work",
            Self::Declined => "declined",
            Self::GuardDeclined => "guard_declined",
        }
    }
}

/// The tight-support update route's availability in the run's steady state:
/// as the last decoded batch that held reuse authority reported it, since a
/// batch without authority cannot update whatever its limits. Until a batch
/// holds authority, as the last decoded batch reported it, which names the
/// reason whenever that batch's schedule or limits already decline the route.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct TightUpdateStatistics {
    /// Whether the route was available, and why not.
    pub availability: TightUpdateAvailability,
    /// Bits of each per-head support count, while the route was admitted.
    pub count_bits: Option<u32>,
    /// A decoded batch held reuse authority, so the availability is the run's
    /// steady state rather than the report of a batch without authority.
    pub steady_state: bool,
    /// Decoded batches the device oracle's runtime guard declined the route
    /// for, so a run that declined and then re-engaged shows both.
    pub guard_declined_batches: u64,
}

/// Actual formula GPU work and explicitly retained candidate results.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct FormulaExecutionStatistics {
    /// Physical adapter name and native API selected during this invocation.
    pub adapter: String,
    /// Bounded batch completion accounting; scalar cursor execution has no record.
    pub completion: CompletionAccounting,
    /// Effective general propagation limits, absent for CPU and tight support.
    pub gpu_limits: Option<FormulaDeviceLimits>,
    /// Decoded residual reasons for the general GPU primitive. Absent for CPU
    /// and tight-support execution; distinct from committed `cpu_residuals`.
    pub gpu_residuals: Option<FormulaResidualStatistics>,
    /// Tight-support work ceiling per candidate. Present only for that device
    /// primitive; propagation sweep limits do not apply to a support scan.
    pub tight_work_per_candidate: Option<u64>,
    /// Work of submitted tight batches, including unreturned ones: what a
    /// validated readback reports performed, or else the submission's
    /// reservation, each candidate's allowance for the routes it may take.
    /// Propagation has candidate-dependent work and leaves this absent.
    pub gpu_scheduled_work: Option<u64>,
    /// Decoded tight-support slots by route; absent before the first decoded
    /// tight batch and for other primitives.
    pub gpu_tight_routes: Option<TightRouteStatistics>,
    /// The tight-support update route's steady-state availability, or the last
    /// decoded batch's while none has held reuse authority; absent before the
    /// first decoded tight batch and for other primitives.
    pub gpu_tight_updates: Option<TightUpdateStatistics>,
    /// Batches actually submitted, including submissions without a decoded result.
    pub gpu_submitted_batches: u64,
    /// Candidate worlds actually submitted, including unreturned results.
    pub gpu_submitted_candidates: u64,
    /// Successfully dispatched and decoded GPU batches: device submissions,
    /// of which one candidate slate can take several.
    pub gpu_batches: u64,
    /// Candidate slates a GPU formula session committed with at least one
    /// candidate. A slate is the candidates the stream proposes together for
    /// one membership check and commit; the device checks it in one or more
    /// submissions. Zero for CPU routes.
    pub gpu_slates: u64,
    /// The largest committed slate's candidate count: at most the batch size
    /// under fixed batching, possibly more under adaptive batching. Absent
    /// before the first commit and for CPU routes.
    pub gpu_largest_slate: Option<usize>,
    /// Selections that returned adaptive slates to the batch size because the
    /// previous slate's answers tightened the objective bound. Every such
    /// selection counts, including one already at the batch size and the
    /// session's last selection, so a program reports one reset per slate
    /// whose answers installed a bound. Always zero under fixed batching and
    /// for CPU routes.
    pub gpu_slate_resets: u64,
    /// Slates a GPU enumeration proposed while their predecessor's last
    /// submission was checked on the device. Zero for optimizing searches and
    /// for CPU routes.
    pub gpu_lookahead_slates: u64,
    /// The last decision on proposing a slate ahead: engaged, or declined with
    /// its reason, so a run shows why it stayed serial. Absent for CPU routes
    /// and before a GPU session's first slate.
    pub gpu_lookahead: Option<GpuLookahead>,
    /// Device compute of the submissions checked while the host proposed the
    /// next slate, in nanoseconds: the measured intervals of
    /// [`Self::gpu_compute_timing`] that belong to those submissions, each
    /// paired with its submission's finish wait in
    /// [`Self::gpu_lookahead_finish_wait_ns`].
    pub gpu_lookahead_device_ns: u64,
    /// The finish's waits on the same submissions, in nanoseconds: each from
    /// the end of the production run beside it to its observed completion,
    /// before its readback is mapped and decoded. The device time hidden
    /// behind production is the difference of the two paired sums,
    /// [`Self::gpu_lookahead_device_ns`] less this, over the overlapped
    /// submissions with a measured interval.
    pub gpu_lookahead_finish_wait_ns: u64,
    /// Overlapped submissions whose interval the timer refused or never
    /// resolved. Neither paired sum includes them, so the hidden time covers
    /// only the submissions measured; zero for CPU routes.
    pub gpu_lookahead_unmeasured: u64,
    /// Elapsed time from the device execution's construction to the release of
    /// its first answer, in nanoseconds; reported, never a criterion. Absent
    /// before the first answer and for CPU routes.
    pub first_answer_ns: Option<u64>,
    /// Candidate worlds returned by successful GPU dispatches.
    pub gpu_candidates: u64,
    /// Charged primitive work from successfully decoded results; not time or
    /// GPU instruction count. Work in an interrupted unreturned batch is unknown.
    pub gpu_work: u64,
    /// Sum of completed per-candidate propagation sweeps in decoded results.
    /// Sweeps in an interrupted unreturned batch are unknown.
    pub gpu_rounds: u64,
    /// Committed candidates completed by exact native CPU residual search.
    pub cpu_residuals: u64,
    /// Successfully committed candidates decided by the selected GPU primitive.
    pub gpu_decided: u64,
    /// Proposed candidates whose membership has not been committed.
    pub pending_candidates: usize,
    /// Verified stable interpretations still queued before scoring/output.
    pub queued_models: usize,
    /// Conservative peak authored allocation accounting reported by the device API.
    pub peak_accounted_bytes: u64,
    /// The workgroup size, in invocations per candidate, that the decoded
    /// device batches dispatched; absent before the first decoded batch. One
    /// value describes the solve: its execution owns one oracle checking one
    /// theory instance, whose retained graph fixes the size.
    pub gpu_workgroup_size: Option<u32>,
    /// Optional device compute-pass time of the decoded batches, each reported
    /// once from its own interval: measured, unavailable, or unresolved while
    /// the execution has not settled or when its settlement could not read it. Separate from host-oracle, transfer,
    /// waiting and exact residual time; a settlement's resolve-only submission
    /// is counted as a flush, not a batch.
    pub gpu_compute_timing: zetesis_backend::ComputeTiming,
}

/// Whether a GPU enumeration proposed its next slate while the device checked
/// the current one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GpuLookahead {
    /// The next slate's production ran while the current slate's last
    /// submission was checked.
    Engaged,
    /// The next slate was proposed after the current one committed.
    Declined(GpuLookaheadDeclined),
}

/// Why a GPU session did not propose its next slate while the device checked
/// the current one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GpuLookaheadDeclined {
    /// The session selects optimal answers, whose bound a slate produced ahead
    /// would not see.
    Optimizing,
    /// The answers still requested fit the slate under check, so a slate
    /// proposed ahead would not be used.
    RequestedAnswers,
    /// The candidate stream declined it.
    Stream(zetesis_stable::LookaheadDeclined),
}

impl GpuLookahead {
    /// A stable snake-case name for reports: `engaged`, or the reason it
    /// was declined.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Engaged => "engaged",
            Self::Declined(reason) => reason.label(),
        }
    }
}

impl GpuLookaheadDeclined {
    /// A stable snake-case name for reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Optimizing => "optimizing",
            Self::RequestedAnswers => "requested_answers",
            Self::Stream(reason) => reason.label(),
        }
    }
}

pub(crate) enum Failure {
    Search(Incomplete),
    Run(SolveError),
}

impl From<crate::ExecutorError> for Failure {
    fn from(error: crate::ExecutorError) -> Self {
        Self::Run(match error {
            crate::ExecutorError::Shape { expected, actual } => {
                SolveError::FormulaBatchShape { expected, actual }
            }
            crate::ExecutorError::ForeignBatch => {
                SolveError::Executor(crate::ExecutorError::ForeignBatch)
            }
        })
    }
}

/// A stream of original, reduct-checked semantic models, before display or scoring.
pub(crate) trait MembershipExecution {
    /// Return `None` only after `models.exhausted()` establishes complete
    /// candidate coverage. A cooperative stop is an explicit `Failure::Search`;
    /// an execution failure must not masquerade as end of enumeration.
    fn next(
        &mut self,
        models: &mut StableModels,
        options: &SolveConfig,
        cancellation: &zetesis_algebra::Cancellation,
        phases: &Recorder,
    ) -> Option<Result<Interpretation, Failure>>;
    fn statistics(&self, models: &StableModels) -> Option<FormulaExecutionStatistics>;
    /// Record, before a pull, whether the session enumerates answer sets and
    /// how many it still owes (`None` when unbounded). A device execution
    /// proposes the next slate while the current one is checked only for an
    /// enumeration whose owed answers exceed the slate under check. Other
    /// executions ignore it.
    fn demand(&mut self, _enumerates: bool, _owed: Option<usize>) {}
    /// Receive the solve's learned store, which the session's streams share.
    /// Rows an execution learns while completing candidates would reach the
    /// streams through it; no execution learns any yet.
    fn learned_store(&mut self, _store: std::sync::Arc<zetesis_stable::LearnedStore>) {}
    /// Read the last completed device batch's compute-pass interval before the
    /// execution's final snapshot. A device execution observes the control and
    /// waits at most the time it kept from construction, so every path into a
    /// settlement reaches them. Settling an execution without an unresolved
    /// device batch does nothing; a settlement refused by a busy or unhealthy
    /// context leaves the batch unresolved, and the next settlement retries it.
    /// At most one flush submits per execution, because a submitted flush either
    /// reads the batch or retires the timer. A settlement never changes the
    /// solve's outcome.
    fn settle(&mut self, _phases: &Recorder) {}
}

/// What a device execution's settlement needs, kept from construction: the
/// owning session's control and the wait its batches are given, the device
/// limits' default timeout.
#[cfg(feature = "gpu")]
pub(crate) struct Settlement {
    control: zetesis_algebra::Cancellation,
    wait: std::time::Duration,
}

#[cfg(feature = "gpu")]
impl Settlement {
    pub(crate) fn new(control: &zetesis_algebra::Cancellation, wait: std::time::Duration) -> Self {
        Self {
            control: control.clone(),
            wait,
        }
    }
}

pub(crate) enum Execution {
    Cpu,
    Batched {
        queue: Box<crate::formula_queue::BatchQueue>,
    },
    #[cfg(feature = "gpu")]
    Hybrid {
        oracle: Box<zetesis_wgpu::GpuFormulaOracle>,
        queue: Box<crate::formula_queue::BatchQueue>,
        statistics: Box<FormulaExecutionStatistics>,
        settlement: Settlement,
    },
    #[cfg(feature = "gpu")]
    Tight {
        oracle: Box<zetesis_wgpu::GpuTightOracle>,
        plan: Arc<zetesis_ferraris::TightPlan>,
        queue: Box<crate::formula_queue::BatchQueue>,
        statistics: Box<FormulaExecutionStatistics>,
        settlement: Settlement,
    },
}

impl Execution {
    /// Choose the execution route. A device execution keeps `control`, the
    /// owning session's, for its settlement.
    pub(crate) fn with_resources(
        options: &SolveConfig,
        resources: &ExecutionResources,
        tight_plan: Option<Arc<zetesis_ferraris::TightPlan>>,
        control: &zetesis_algebra::Cancellation,
        observations: &mut impl ExecutionSink,
    ) -> Result<Self, SolveError> {
        if options.backend == Backend::Cpu {
            observations.record(Event::CpuFormula {
                oracle: options.oracle,
                grounder: options.grounder,
            })?;
            if let Some(workers) = options.region_workers() {
                // The workers decide their leaves themselves; the batched
                // protocol would check them again.
                observations.record(Event::ParallelRegions { workers })?;
                return Ok(Self::Cpu);
            }
            if options.completion_workers.get() > 1 {
                observations.record(Event::ExactCompletion {
                    workers: options.completion_workers,
                    max_scratch_bytes: options.max_completion_scratch_bytes,
                })?;
                return Ok(Self::Batched {
                    queue: Box::new(crate::formula_queue::BatchQueue::new(options)?),
                });
            }
            return Ok(Self::Cpu);
        }
        Self::gpu(options, resources, tight_plan, control, observations)
    }

    #[cfg(not(feature = "gpu"))]
    fn gpu(
        _: &SolveConfig,
        _: &ExecutionResources,
        _: Option<Arc<zetesis_ferraris::TightPlan>>,
        _: &zetesis_algebra::Cancellation,
        _: &mut impl ExecutionSink,
    ) -> Result<Self, SolveError> {
        Err(SolveError::BackendUnavailable)
    }

    #[cfg(feature = "gpu")]
    fn gpu(
        options: &SolveConfig,
        resources: &ExecutionResources,
        tight_plan: Option<Arc<zetesis_ferraris::TightPlan>>,
        control: &zetesis_algebra::Cancellation,
        observations: &mut impl ExecutionSink,
    ) -> Result<Self, SolveError> {
        let source = resources.device_source(options).map_err(SolveError::Gpu)?;
        if let Some(plan) = tight_plan {
            return Self::tight(options, source, plan, control, observations);
        }
        // A supplied profile is used exactly, with its shader and pipelines.
        let oracle = match resources.formula_profile() {
            Some(profile) => zetesis_wgpu::GpuFormulaOracle::from_profile(profile),
            None => zetesis_wgpu::GpuFormulaOracle::from_source(source),
        }
        .map_err(SolveError::Gpu)?;
        let adapter = format!(
            "{}, {}; vendor=0x{:04x}",
            oracle.info().name(),
            oracle.info().backend(),
            oracle.info().vendor_id()
        );
        observations.record(Event::DeviceFormula {
            adapter: oracle.info().metadata(),
            projection: oracle.projection(),
            grounder: options.grounder,
            batch_size: options.batch_size,
            completion_workers: options.completion_workers,
        })?;
        let gpu_compute_timing =
            zetesis_backend::ComputeTiming::new(oracle.context().compute_timing());
        Ok(Self::Hybrid {
            oracle: Box::new(oracle),
            queue: Box::new(crate::formula_queue::BatchQueue::for_joint_oracle(options)?),
            statistics: Box::new(FormulaExecutionStatistics {
                adapter,
                gpu_limits: Some(options.into()),
                gpu_residuals: Some(FormulaResidualStatistics::default()),
                gpu_compute_timing,
                ..Default::default()
            }),
            // The wait every slate's submissions use, whatever the slate size.
            settlement: Settlement::new(
                control,
                slate_limits(options, options.batch_size.get()).timeout,
            ),
        })
    }
}

impl MembershipExecution for Execution {
    fn demand(&mut self, enumerates: bool, owed: Option<usize>) {
        #[cfg(feature = "gpu")]
        if let Self::Hybrid { queue, .. } | Self::Tight { queue, .. } = self {
            queue.set_demand(crate::formula_queue::Demand { enumerates, owed });
        }
        #[cfg(not(feature = "gpu"))]
        let _ = (enumerates, owed);
    }

    fn statistics(&self, models: &StableModels) -> Option<FormulaExecutionStatistics> {
        match self {
            Self::Cpu => None,
            Self::Batched { queue } => Some(batch_statistics(
                queue,
                models,
                FormulaExecutionStatistics::default(),
            )),
            #[cfg(feature = "gpu")]
            Self::Hybrid {
                oracle,
                queue,
                statistics,
                ..
            } => Some(device_statistics(
                models,
                queue,
                statistics,
                oracle.compute_time_unresolved(),
            )),
            #[cfg(feature = "gpu")]
            Self::Tight {
                oracle,
                queue,
                statistics,
                ..
            } => Some(device_statistics(
                models,
                queue,
                statistics,
                oracle.compute_time_unresolved(),
            )),
        }
    }

    fn settle(&mut self, phases: &Recorder) {
        match self {
            Self::Cpu | Self::Batched { .. } => {
                let _ = phases;
            }
            #[cfg(feature = "gpu")]
            Self::Hybrid {
                oracle,
                queue,
                statistics,
                settlement,
            } => {
                let flushed = if oracle.compute_time_unresolved() {
                    // A failed flush leaves its fault in the context's health.
                    let _ = phases.measure(SolvePhase::GpuHostOracle, || {
                        oracle.flush_compute_time(settlement.wait, &settlement.control)
                    });
                    record_flush(statistics, oracle.reported_compute_time());
                    oracle.reported_compute_time()
                } else {
                    None
                };
                // The last submission's interval arrives only with the flush;
                // without one, an overlapped last submission is unmeasured.
                queue.flushed(flushed);
            }
            #[cfg(feature = "gpu")]
            Self::Tight {
                oracle,
                queue,
                statistics,
                settlement,
                ..
            } => {
                let flushed = if oracle.compute_time_unresolved() {
                    // A failed flush leaves its fault in the context's health.
                    let _ = phases.measure(SolvePhase::GpuHostOracle, || {
                        oracle.flush_compute_time(settlement.wait, &settlement.control)
                    });
                    record_flush(statistics, oracle.reported_compute_time());
                    oracle.reported_compute_time()
                } else {
                    None
                };
                // The last submission's interval arrives only with the flush;
                // without one, an overlapped last submission is unmeasured.
                queue.flushed(flushed);
            }
        }
    }

    fn next(
        &mut self,
        models: &mut StableModels,
        options: &SolveConfig,
        cancellation: &zetesis_algebra::Cancellation,
        phases: &Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        match self {
            Self::Cpu => {
                let _ = options;
                let _ = cancellation;
                let _ = phases;
                models.next().map(|result| result.map_err(Failure::Search))
            }
            Self::Batched { queue } => queue.next(models, options, cancellation, |batch| {
                let candidates = batch.candidates();
                let mut verdicts = Vec::new();
                verdicts
                    .try_reserve_exact(candidates.len())
                    .map_err(|_| Failure::Search(Incomplete::Allocation))?;
                verdicts.resize(candidates.len(), zetesis_stable::BatchVerdict::Residual);
                batch.finish(verdicts).map_err(Failure::from)
            }),
            #[cfg(feature = "gpu")]
            Self::Hybrid {
                oracle,
                queue,
                statistics,
                ..
            } => {
                if !queue.enumerates() {
                    queue.decline_optimizing();
                }
                let mut device = FormulaDevice {
                    oracle,
                    statistics,
                    options,
                };
                queue.next_device(models, options, cancellation, phases, &mut device)
            }
            #[cfg(feature = "gpu")]
            Self::Tight {
                oracle,
                plan,
                queue,
                statistics,
                ..
            } => {
                if queue.enumerates() {
                    let mut device = super::formula_tight::TightDevice {
                        oracle,
                        plan,
                        statistics,
                        options,
                    };
                    return queue.next_device(models, options, cancellation, phases, &mut device);
                }
                queue.decline_optimizing();
                queue.next(models, options, cancellation, |batch| {
                    let verdicts = super::formula_tight::check(
                        oracle,
                        plan,
                        statistics,
                        batch.candidates(),
                        options,
                        cancellation,
                        phases,
                    )?;
                    verdicts.finish(batch).map_err(Failure::from)
                })
            }
        }
    }
}

#[cfg(feature = "gpu")]
fn decoded_verdict(
    residuals: &mut FormulaResidualStatistics,
    verdict: zetesis_wgpu::FormulaVerdict,
) -> Result<
    (
        zetesis_stable::BatchVerdict,
        Option<zetesis_algebra::region::Region>,
    ),
    Failure,
> {
    use zetesis_stable::BatchVerdict;
    use zetesis_wgpu::{FormulaVerdict, ResidualReason};
    Ok(match verdict {
        FormulaVerdict::NotModel => (BatchVerdict::NotModel, None),
        FormulaVerdict::NoProperSubset => (BatchVerdict::NoProperSubset, None),
        FormulaVerdict::Residual(reason, root) => {
            let count = match reason {
                ResidualReason::FixedPoint => &mut residuals.fixed_point,
                ResidualReason::RoundLimit => &mut residuals.round_limit,
                ResidualReason::WorkLimit => &mut residuals.work_limit,
            };
            add(count, 1)?;
            (BatchVerdict::Residual, Some(root))
        }
    })
}

/// Copy a GPU formula queue's slate statistics into `statistics`.
#[cfg(feature = "gpu")]
pub(crate) fn record_slates(
    queue: &crate::formula_queue::BatchQueue,
    statistics: &mut FormulaExecutionStatistics,
) {
    statistics.gpu_slates = queue.slates();
    statistics.gpu_largest_slate = queue.largest_slate().map(std::num::NonZeroUsize::get);
    statistics.gpu_slate_resets = queue.slate_resets();
    let lookahead = queue.lookahead();
    statistics.gpu_lookahead_slates = lookahead.slates;
    statistics.gpu_lookahead = lookahead.decision;
    statistics.gpu_lookahead_device_ns = lookahead.device_ns;
    statistics.gpu_lookahead_finish_wait_ns = lookahead.finish_wait_ns;
    statistics.gpu_lookahead_unmeasured = lookahead.unmeasured;
    statistics.first_answer_ns = queue.first_answer_ns();
}

/// Device limits for a slate of `offered` candidates. Under either batching
/// policy one submission may take the whole slate: the policy decides the
/// slate, and the device returns the largest prefix it admits under the byte
/// allowance and its granted limits. The checker submits the rest of a slate
/// in further submissions. Under fixed batching a slate never exceeds the
/// batch size, so no submission does either.
#[cfg(feature = "gpu")]
fn slate_limits(options: &SolveConfig, offered: usize) -> zetesis_wgpu::FormulaLimits {
    zetesis_wgpu::FormulaLimits {
        max_candidates: offered,
        max_batch_bytes: options.max_batch_bytes,
        max_work_per_candidate: options.gpu_formula_work,
        max_rounds: options.gpu_formula_rounds,
        ..Default::default()
    }
}

/// A device execution's snapshot: its retained statistics with the stream's
/// current batch and slate accounting, and its last completed batch counted
/// unresolved while that batch's interval has not been read.
#[cfg(feature = "gpu")]
fn device_statistics(
    models: &StableModels,
    queue: &crate::formula_queue::BatchQueue,
    statistics: &FormulaExecutionStatistics,
    timing_unresolved: bool,
) -> FormulaExecutionStatistics {
    let batch = models.batch_statistics();
    let mut snapshot = FormulaExecutionStatistics {
        cpu_residuals: batch.residuals,
        gpu_decided: batch.propagated,
        pending_candidates: batch.pending,
        queued_models: queue.len(),
        completion: queue.accounting(),
        ..statistics.clone()
    };
    snapshot.gpu_compute_timing.unresolved_batches = u64::from(timing_unresolved);
    record_slates(queue, &mut snapshot);
    snapshot
}

/// Record a settlement flush: a flush that submitted always reports its batch.
#[cfg(feature = "gpu")]
fn record_flush(
    statistics: &mut FormulaExecutionStatistics,
    report: Option<zetesis_backend::BatchComputeTime>,
) {
    if let Some(report) = report {
        statistics.gpu_compute_timing.record(report);
        statistics.gpu_compute_timing.record_flush_submission();
    }
}

/// Add a decoded tight batch's routes and fold in its update availability.
#[cfg(feature = "gpu")]
pub(super) fn record_tight_routes(
    statistics: &mut FormulaExecutionStatistics,
    batch: &zetesis_wgpu::TightGpuBatchStats,
) -> Result<(), Failure> {
    let routes = statistics
        .gpu_tight_routes
        .get_or_insert_with(Default::default);
    add(&mut routes.complete, batch.routes.complete)?;
    add(&mut routes.dense_reuse, batch.routes.dense_reuse)?;
    add(&mut routes.beyond_threshold, batch.routes.beyond_threshold)?;
    add(&mut routes.update, batch.routes.update)?;
    add(&mut routes.nodes_evaluated, batch.nodes_evaluated)?;
    record_update_availability(
        statistics
            .gpu_tight_updates
            .get_or_insert_with(Default::default),
        batch.original_truth_reused,
        batch.updates,
    )
}

/// Fold one decoded batch's update availability into the run's. A batch
/// holding reuse authority sets the steady state, which a later batch without
/// authority leaves standing: such a batch cannot update whatever its limits.
/// Until a batch holds authority, each batch's report replaces the last. Every
/// batch the runtime guard declined is counted, whatever its authority.
#[cfg(feature = "gpu")]
fn record_update_availability(
    updates: &mut TightUpdateStatistics,
    authority: bool,
    report: zetesis_wgpu::TightGpuUpdates,
) -> Result<(), Failure> {
    use zetesis_wgpu::{TightGpuUpdateUnavailable as Reason, TightGpuUpdates};
    if report == TightGpuUpdates::Unavailable(Reason::GuardDeclined) {
        add(&mut updates.guard_declined_batches, 1)?;
    }
    if updates.steady_state && !authority {
        return Ok(());
    }
    updates.steady_state = authority;
    (updates.availability, updates.count_bits) = match report {
        TightGpuUpdates::Admitted { count_bits, .. } => {
            (TightUpdateAvailability::Admitted, Some(count_bits))
        }
        TightGpuUpdates::Unavailable(reason) => (
            match reason {
                Reason::SerialSchedule => TightUpdateAvailability::SerialSchedule,
                Reason::NarrowSchedule => TightUpdateAvailability::NarrowSchedule,
                Reason::CandidateCeiling => TightUpdateAvailability::CandidateCeiling,
                Reason::IndexBounds => TightUpdateAvailability::IndexBounds,
                Reason::Bytes => TightUpdateAvailability::Bytes,
                Reason::Work => TightUpdateAvailability::Work,
                Reason::Declined => TightUpdateAvailability::Declined,
                Reason::GuardDeclined => TightUpdateAvailability::GuardDeclined,
                // Every reason the library names is mapped above. One it
                // adds later reports `pending`, which names no reason,
                // until this statistic names it too.
                _ => TightUpdateAvailability::Pending,
            },
            None,
        ),
        _ => (TightUpdateAvailability::Pending, None),
    };
    Ok(())
}

/// Record the workgroup size a decoded batch dispatched: the size of the one
/// retained graph that every batch of this execution's oracle dispatches.
#[cfg(feature = "gpu")]
pub(super) fn record_workgroup_size(statistics: &mut FormulaExecutionStatistics, size: u32) {
    statistics.gpu_workgroup_size = Some(size);
}

#[cfg(feature = "gpu")]
pub(super) fn record_submission(
    statistics: &mut FormulaExecutionStatistics,
    candidates: Option<usize>,
) -> Result<(), Failure> {
    let Some(candidates) = candidates else {
        return Ok(());
    };
    let candidates =
        u64::try_from(candidates).map_err(|_| Failure::Search(Incomplete::CounterOverflow))?;
    // Commit both counters together, retaining the previous coherent receipt
    // when their representation is exhausted.
    let batches = statistics.gpu_submitted_batches.checked_add(1);
    let candidates = statistics.gpu_submitted_candidates.checked_add(candidates);
    let (Some(batches), Some(candidates)) = (batches, candidates) else {
        return Err(Failure::Search(Incomplete::CounterOverflow));
    };
    statistics.gpu_submitted_batches = batches;
    statistics.gpu_submitted_candidates = candidates;
    Ok(())
}

#[cfg(feature = "gpu")]
pub(super) fn add(counter: &mut u64, value: u64) -> Result<(), Failure> {
    *counter = counter
        .checked_add(value)
        .ok_or(Failure::Search(Incomplete::CounterOverflow))?;
    Ok(())
}

fn batch_statistics(
    queue: &crate::formula_queue::BatchQueue,
    models: &StableModels,
    mut statistics: FormulaExecutionStatistics,
) -> FormulaExecutionStatistics {
    let batch = models.batch_statistics();
    statistics.cpu_residuals = batch.residuals;
    statistics.pending_candidates = batch.pending;
    statistics.queued_models = queue.len();
    statistics.completion = queue.accounting();
    statistics
}

impl<E: MembershipExecution + ?Sized> MembershipExecution for &mut E {
    fn next(
        &mut self,
        models: &mut StableModels,
        config: &SolveConfig,
        cancellation: &zetesis_algebra::Cancellation,
        phases: &Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        (**self).next(models, config, cancellation, phases)
    }
    fn statistics(&self, models: &StableModels) -> Option<FormulaExecutionStatistics> {
        (**self).statistics(models)
    }
    fn demand(&mut self, enumerates: bool, owed: Option<usize>) {
        (**self).demand(enumerates, owed);
    }
}

/// General propagation's device side of a GPU enumeration's two-phase slates,
/// recording each submission's receipts as the closure form's checker does.
#[cfg(feature = "gpu")]
pub(crate) struct FormulaDevice<'a> {
    oracle: &'a mut zetesis_wgpu::GpuFormulaOracle,
    statistics: &'a mut FormulaExecutionStatistics,
    options: &'a SolveConfig,
}

#[cfg(feature = "gpu")]
impl crate::formula_queue::device::SlateDevice for FormulaDevice<'_> {
    const REGIONAL: bool = true;

    fn record_slate_bytes(&mut self, total: u64) {
        self.statistics.peak_accounted_bytes = self.statistics.peak_accounted_bytes.max(total);
    }

    fn retained_batch_bytes(&self) -> Result<u64, Failure> {
        self.oracle.retained_batch_bytes().map_err(gpu_failure)
    }

    type Prepared<'d>
        = zetesis_wgpu::PreparedFormulaSubmission<'d>
    where
        Self: 'd;
    type Checked =
        Result<zetesis_wgpu::CheckedSlate<zetesis_wgpu::FormulaCheck>, zetesis_wgpu::GpuError>;

    fn prepare<'d>(
        &'d mut self,
        slate: zetesis_ferraris::SlateRef<'_>,
        offset: usize,
        max_bytes: u64,
        cancellation: &zetesis_algebra::Cancellation,
    ) -> Result<Self::Prepared<'d>, Failure> {
        let mut limits = slate_limits(self.options, slate.candidates().len());
        limits.max_batch_bytes = max_bytes;
        self.oracle
            .prepare(slate, offset, limits, cancellation)
            .map_err(gpu_failure)
    }

    fn decode(
        &mut self,
        checked: Self::Checked,
        theory: &zetesis_ferraris::Theory,
        slate: zetesis_ferraris::SlateId,
        offset: usize,
        offered: usize,
        rows: &mut crate::formula_queue::device::CheckedRows,
    ) -> Result<crate::formula_queue::device::Decoded, Failure> {
        // A failed call can still report the batch before it, as unavailable.
        let compute = self.oracle.reported_compute_time();
        if let Some(report) = compute {
            self.statistics.gpu_compute_timing.record(report);
        }
        // Submission is observable even when mapping or decoding fails. Preserve
        // that operation's original error over a secondary accounting overflow.
        let submitted =
            record_submission(self.statistics, self.oracle.last_submission_candidates());
        let checks = checked.map_err(gpu_failure)?;
        submitted?;
        let count = checks.count();
        if count == 0 || count > offered {
            return Err(Failure::Run(SolveError::FormulaBatchShape {
                expected: offered,
                actual: count,
            }));
        }
        let finish_wait = checks.finish_wait();
        let checks = checks
            .into_checks(theory, slate, offset)
            .map_err(|error| Failure::Run(SolveError::Gpu(error)))?;
        add(&mut self.statistics.gpu_batches, 1)?;
        add(
            &mut self.statistics.gpu_candidates,
            u64::try_from(checks.len())
                .map_err(|_| Failure::Search(Incomplete::CounterOverflow))?,
        )?;
        if let Some(batch) = self.oracle.last_batch_stats() {
            self.statistics.peak_accounted_bytes = self
                .statistics
                .peak_accounted_bytes
                .max(batch.accounted_bytes);
            record_workgroup_size(self.statistics, batch.workgroup_size);
        }
        for check in checks {
            add(
                &mut self.statistics.gpu_work,
                u64::from(check.statistics().work),
            )?;
            add(
                &mut self.statistics.gpu_rounds,
                u64::from(check.statistics().rounds),
            )?;
            let (verdict, root) = decoded_verdict(
                self.statistics.gpu_residuals.get_or_insert_default(),
                check.into_verdict(),
            )?;
            if let Some(root) = root {
                rows.projected(root)?;
            } else {
                rows.classified(verdict)?;
            }
        }
        Ok(crate::formula_queue::device::Decoded {
            compute,
            finish_wait,
        })
    }
}

#[cfg(feature = "gpu")]
impl crate::formula_queue::device::PreparedSubmission
    for zetesis_wgpu::PreparedFormulaSubmission<'_>
{
    type Checked =
        Result<zetesis_wgpu::CheckedSlate<zetesis_wgpu::FormulaCheck>, zetesis_wgpu::GpuError>;

    fn accounted_bytes(&self) -> u64 {
        Self::accounted_bytes(self)
    }

    fn count(&self) -> usize {
        Self::count(self)
    }

    fn check(self, cancellation: &zetesis_algebra::Cancellation) -> Self::Checked {
        Self::check(self, cancellation)
    }

    fn check_overlapping<R>(
        self,
        cancellation: &zetesis_algebra::Cancellation,
        during: impl FnOnce() -> R,
    ) -> (Self::Checked, Option<R>) {
        Self::check_overlapping(self, cancellation, during)
    }
}

/// A device operation's failure: a stop of the control is the search's, any
/// other is the run's.
#[cfg(feature = "gpu")]
fn gpu_failure(error: zetesis_wgpu::GpuError) -> Failure {
    match error.interruption() {
        Some(stop) => Failure::Search(stop.into()),
        None => Failure::Run(SolveError::Gpu(error)),
    }
}

#[cfg(all(test, feature = "gpu"))]
mod tests;
