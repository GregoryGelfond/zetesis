//! Formula membership execution with bounded proposal, check and commit batches.

use crate::ExecutionObservation as Event;
use crate::execution_observation::ExecutionSink;

use std::sync::Arc;
use zetesis_ferraris::Interpretation;
use zetesis_sat::{Incomplete, StableModels};

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

/// Actual formula GPU work and explicitly retained candidate results.
#[derive(Clone, Debug, Default)]
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
    /// Complete tight-support scan work ceiling. Present only for that device
    /// primitive; propagation sweep limits do not apply to a support scan.
    pub tight_work_per_candidate: Option<u64>,
    /// Scheduled full-scan work, including unreturned submitted tight batches.
    /// Propagation has candidate-dependent work and leaves this absent.
    pub gpu_scheduled_work: Option<u64>,
    /// Batches actually submitted, including submissions without a decoded result.
    pub gpu_submitted_batches: u64,
    /// Candidate worlds actually submitted, including unreturned results.
    pub gpu_submitted_candidates: u64,
    /// Successfully dispatched and decoded GPU batches.
    pub gpu_batches: u64,
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
            error => SolveError::Executor(error),
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
        cancellation: &zetesis_cpu::Cancellation,
        phases: &Recorder,
    ) -> Option<Result<Interpretation, Failure>>;
    fn statistics(&self, models: &StableModels) -> Option<FormulaExecutionStatistics>;

    fn batch_execution(&self, _: &StableModels) -> Option<crate::BatchExecutionStatistics> {
        None
    }
}

pub(crate) enum Execution {
    Cpu,
    External {
        executor: Box<dyn crate::batch_executor::ErasedExecutor>,
        capabilities: crate::ExecutorCapabilities,
        operation: crate::MembershipOperation,
        queue: Box<crate::formula_queue::BatchQueue>,
    },
    Batched {
        queue: Box<crate::formula_queue::BatchQueue>,
    },
    #[cfg(feature = "gpu")]
    Hybrid {
        oracle: Box<zetesis_wgpu::GpuFormulaOracle>,
        queue: Box<crate::formula_queue::BatchQueue>,
        statistics: Box<FormulaExecutionStatistics>,
    },
    #[cfg(feature = "gpu")]
    Tight {
        oracle: Box<zetesis_wgpu::GpuTightOracle>,
        plan: Arc<zetesis_ferraris::TightPlan>,
        queue: Box<crate::formula_queue::BatchQueue>,
        statistics: Box<FormulaExecutionStatistics>,
    },
}

impl Execution {
    pub(crate) fn external(
        mut executor: Box<dyn crate::batch_executor::ErasedExecutor>,
        capabilities: crate::ExecutorCapabilities,
        plan: crate::MembershipPlan<'_>,
        options: &SolveConfig,
        cancellation: &zetesis_cpu::Cancellation,
        observations: &mut impl ExecutionSink,
    ) -> Result<Self, Failure> {
        cancellation
            .poll()
            .map_err(|stop| Failure::Search(stop.into()))?;
        executor.prepare(plan, cancellation)?;
        cancellation
            .poll()
            .map_err(|stop| Failure::Search(stop.into()))?;
        observations
            .record(Event::ExternalExecutor {
                capabilities,
                operation: plan.operation(),
            })
            .map_err(Failure::Run)?;
        Ok(Self::External {
            executor,
            capabilities,
            operation: plan.operation(),
            queue: Box::new(crate::formula_queue::BatchQueue::new(options).map_err(Failure::Run)?),
        })
    }

    pub(crate) fn with_resources(
        options: &SolveConfig,
        resources: &ExecutionResources,
        tight_plan: Option<Arc<zetesis_ferraris::TightPlan>>,
        observations: &mut impl ExecutionSink,
    ) -> Result<Self, SolveError> {
        if options.backend == Backend::Cpu {
            observations.record(Event::CpuFormula {
                oracle: options.oracle,
                grounder: options.grounder,
                search: options.search,
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
        Self::gpu(options, resources, tight_plan, observations)
    }

    #[cfg(not(feature = "gpu"))]
    fn gpu(
        _: &SolveConfig,
        _: &ExecutionResources,
        _: Option<Arc<zetesis_ferraris::TightPlan>>,
        _: &mut impl ExecutionSink,
    ) -> Result<Self, SolveError> {
        Err(SolveError::BackendUnavailable)
    }

    #[cfg(feature = "gpu")]
    fn gpu(
        options: &SolveConfig,
        resources: &ExecutionResources,
        tight_plan: Option<Arc<zetesis_ferraris::TightPlan>>,
        observations: &mut impl ExecutionSink,
    ) -> Result<Self, SolveError> {
        let context = resources
            .gpu_for(options.backend)
            .map_err(SolveError::Gpu)?;
        if let Some(plan) = tight_plan {
            return Self::tight(options, context, plan, observations);
        }
        let oracle = match (resources.formula_profile(), context) {
            (Some(profile), _) => zetesis_wgpu::GpuFormulaOracle::from_profile(profile),
            (None, Some(context)) => zetesis_wgpu::GpuFormulaOracle::from_context(context),
            (None, None) => zetesis_wgpu::GpuFormulaOracle::new_selected(
                zetesis_wgpu::GpuOptions::default(),
                crate::engine::selection(options.backend),
            ),
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
            search: options.search,
            batch_size: options.batch_size,
            completion_workers: options.completion_workers,
        })?;
        Ok(Self::Hybrid {
            oracle: Box::new(oracle),
            queue: Box::new(crate::formula_queue::BatchQueue::new(options)?),
            statistics: Box::new(FormulaExecutionStatistics {
                adapter,
                gpu_limits: Some(options.into()),
                gpu_residuals: Some(FormulaResidualStatistics::default()),
                ..Default::default()
            }),
        })
    }
}

impl MembershipExecution for Execution {
    fn statistics(&self, models: &StableModels) -> Option<FormulaExecutionStatistics> {
        match self {
            Self::Cpu | Self::External { .. } => None,
            Self::Batched { queue } => Some(batch_statistics(
                queue,
                models,
                FormulaExecutionStatistics::default(),
            )),
            #[cfg(feature = "gpu")]
            Self::Hybrid {
                queue, statistics, ..
            }
            | Self::Tight {
                queue, statistics, ..
            } => {
                let batch = models.batch_statistics();
                Some(FormulaExecutionStatistics {
                    cpu_residuals: batch.residuals,
                    gpu_decided: batch.propagated,
                    pending_candidates: batch.pending,
                    queued_models: queue.len(),
                    completion: queue.accounting(),
                    ..statistics.as_ref().clone()
                })
            }
        }
    }

    fn batch_execution(&self, models: &StableModels) -> Option<crate::BatchExecutionStatistics> {
        match self {
            Self::External {
                capabilities,
                operation,
                queue,
                ..
            } => Some(crate::BatchExecutionStatistics {
                capabilities: *capabilities,
                operation: *operation,
                batches: models.batch_statistics(),
                completion: queue.accounting(),
                queued_models: queue.len(),
            }),
            _ => None,
        }
    }

    fn next(
        &mut self,
        models: &mut StableModels,
        options: &SolveConfig,
        cancellation: &zetesis_cpu::Cancellation,
        phases: &Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        match self {
            Self::External {
                executor, queue, ..
            } => queue.next(models, options, cancellation, |batch| {
                executor.check(batch, cancellation)
            }),
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
                verdicts.resize(candidates.len(), zetesis_sat::BatchVerdict::Residual);
                batch.finish(verdicts).map_err(Failure::from)
            }),
            #[cfg(feature = "gpu")]
            Self::Hybrid {
                oracle,
                queue,
                statistics,
            } => queue.next(models, options, cancellation, |batch| {
                let verdicts = propagate(
                    oracle,
                    statistics,
                    batch.theory(),
                    batch.candidates(),
                    options,
                    cancellation,
                    phases,
                )?;
                batch.finish(verdicts).map_err(Failure::from)
            }),
            #[cfg(feature = "gpu")]
            Self::Tight {
                oracle,
                plan,
                queue,
                statistics,
            } => queue.next(models, options, cancellation, |batch| {
                let verdicts = super::formula_tight::check(
                    oracle,
                    plan,
                    statistics,
                    batch.candidates(),
                    options,
                    cancellation,
                    phases,
                )?;
                batch.finish(verdicts).map_err(Failure::from)
            }),
        }
    }
}

#[cfg(feature = "gpu")]
fn propagate(
    oracle: &mut zetesis_wgpu::GpuFormulaOracle,
    statistics: &mut FormulaExecutionStatistics,
    theory: &zetesis_ferraris::Theory,
    candidates: &[Interpretation],
    options: &SolveConfig,
    cancellation: &zetesis_cpu::Cancellation,
    phases: &Recorder,
) -> Result<Vec<zetesis_sat::BatchVerdict>, Failure> {
    let limits = device_limits(options);
    // The device API returns one ordered verdict per candidate. Reserve its
    // host conversion before dispatch so allocation failure cannot hide a
    // successfully decoded batch from the execution counters.
    let mut verdicts = Vec::new();
    verdicts
        .try_reserve_exact(candidates.len())
        .map_err(|_| Failure::Search(Incomplete::Allocation))?;
    let result = phases.measure(SolvePhase::GpuHostOracle, || {
        oracle.propagate_batch_with_cancellation(theory, candidates, limits, cancellation)
    });
    // Submission is observable even when mapping or decoding fails. Preserve
    // that operation's original error over a secondary accounting overflow.
    let submitted = record_submission(statistics, oracle.last_submission_candidates());
    let checks = result.map_err(|error| match error.interruption() {
        Some(stop) => Failure::Search(stop.into()),
        None => Failure::Run(SolveError::Gpu(error)),
    })?;
    submitted?;
    add(&mut statistics.gpu_batches, 1)?;
    add(
        &mut statistics.gpu_candidates,
        u64::try_from(checks.len()).map_err(|_| Failure::Search(Incomplete::CounterOverflow))?,
    )?;
    if let Some(batch) = oracle.last_batch_stats() {
        statistics.peak_accounted_bytes =
            statistics.peak_accounted_bytes.max(batch.accounted_bytes);
    }
    for check in checks {
        add(&mut statistics.gpu_work, u64::from(check.statistics().work))?;
        add(
            &mut statistics.gpu_rounds,
            u64::from(check.statistics().rounds),
        )?;
        verdicts.push(decoded_verdict(
            statistics.gpu_residuals.get_or_insert_default(),
            check.verdict(),
        )?);
    }
    Ok(verdicts)
}

#[cfg(feature = "gpu")]
fn decoded_verdict(
    residuals: &mut FormulaResidualStatistics,
    verdict: zetesis_wgpu::FormulaVerdict,
) -> Result<zetesis_sat::BatchVerdict, Failure> {
    use zetesis_sat::BatchVerdict;
    use zetesis_wgpu::{FormulaVerdict, ResidualReason};
    Ok(match verdict {
        FormulaVerdict::NotModel => BatchVerdict::NotModel,
        FormulaVerdict::NoProperSubset => BatchVerdict::NoProperSubset,
        FormulaVerdict::Residual(reason) => {
            let count = match reason {
                ResidualReason::FixedPoint => &mut residuals.fixed_point,
                ResidualReason::RoundLimit => &mut residuals.round_limit,
                ResidualReason::WorkLimit => &mut residuals.work_limit,
            };
            add(count, 1)?;
            BatchVerdict::Residual
        }
    })
}

#[cfg(feature = "gpu")]
fn device_limits(options: &SolveConfig) -> zetesis_wgpu::FormulaLimits {
    zetesis_wgpu::FormulaLimits {
        max_candidates: options.batch_size.get(),
        max_batch_bytes: options.max_batch_bytes,
        max_work_per_candidate: options.gpu_formula_work,
        max_rounds: options.gpu_formula_rounds,
        ..Default::default()
    }
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
        cancellation: &zetesis_cpu::Cancellation,
        phases: &Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        (**self).next(models, config, cancellation, phases)
    }
    fn statistics(&self, models: &StableModels) -> Option<FormulaExecutionStatistics> {
        (**self).statistics(models)
    }
    fn batch_execution(&self, models: &StableModels) -> Option<crate::BatchExecutionStatistics> {
        (**self).batch_execution(models)
    }
}

#[cfg(all(test, feature = "gpu"))]
#[path = "../tests/support/formula_resources.rs"]
mod resource_tests;

#[cfg(all(test, feature = "gpu"))]
#[path = "../tests/support/formula_residuals.rs"]
mod residual_tests;
