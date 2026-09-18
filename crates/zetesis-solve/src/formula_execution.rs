//! Ordinary-invocation GPU propagation with exact native residual completion.

use crate::ExecutionObservation as Event;
use crate::execution_observation::ExecutionSink;

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

/// Actual formula GPU work and explicitly retained candidate results.
#[derive(Clone, Debug, Default)]
pub struct FormulaExecutionStatistics {
    /// Physical adapter name and native API selected during this invocation.
    pub adapter: String,
    /// Bounded batch completion accounting; scalar cursor execution has no record.
    pub completion: CompletionAccounting,
    /// Effective device limits, absent when no GPU formula executor was created.
    pub gpu_limits: Option<FormulaDeviceLimits>,
    /// Batches actually submitted, including submissions without a decoded result.
    pub gpu_submitted_batches: u64,
    /// Candidate worlds actually submitted, including unreturned results.
    pub gpu_submitted_candidates: u64,
    /// Successfully dispatched and decoded GPU batches.
    pub gpu_batches: u64,
    /// Candidate worlds returned by successful GPU dispatches.
    pub gpu_candidates: u64,
    /// Charged propagation work from successfully decoded results; not time or
    /// GPU instruction count. Work in an interrupted unreturned batch is unknown.
    pub gpu_work: u64,
    /// Sum of completed per-candidate propagation sweeps in decoded results.
    /// Sweeps in an interrupted unreturned batch are unknown.
    pub gpu_rounds: u64,
    /// Committed candidates completed by exact native CPU residual search.
    pub cpu_residuals: u64,
    /// Successfully committed candidates decided by GPU propagation.
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

/// A stream of original, reduct-checked semantic models, before display or scoring.
pub(crate) trait MembershipExecution {
    /// Return `None` only after `models.exhausted()` establishes complete
    /// candidate coverage. A cooperative stop is an explicit `Failure::Search`;
    /// an execution failure must not masquerade as end of enumeration.
    fn next(
        &mut self,
        models: &mut StableModels,
        options: &SolveConfig,
        control: &zetesis_cpu::Control,
        phases: &Recorder,
    ) -> Option<Result<Interpretation, Failure>>;
    fn statistics(&self, models: &StableModels) -> Option<FormulaExecutionStatistics>;
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
    },
}

impl Execution {
    pub(crate) fn with_resources(
        options: &SolveConfig,
        resources: &ExecutionResources,
        observations: &mut impl ExecutionSink,
    ) -> Result<Self, SolveError> {
        if matches!(options.backend, Backend::Auto | Backend::Cpu) {
            observations.record(Event::CpuFormula {
                oracle: options.oracle,
                grounder: options.grounder,
                search: options.search,
            })?;
            if options.search == crate::SearchMethod::Regions && options.workers.get() > 1 {
                // The workers decide their leaves themselves; the batched
                // protocol would check them again.
                observations.record(Event::ParallelRegions {
                    workers: options.workers,
                })?;
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
        Self::gpu(options, resources, observations)
    }

    #[cfg(not(feature = "gpu"))]
    fn gpu(
        _: &SolveConfig,
        _: &ExecutionResources,
        _: &mut impl ExecutionSink,
    ) -> Result<Self, SolveError> {
        Err(SolveError::BackendUnavailable)
    }

    #[cfg(feature = "gpu")]
    fn gpu(
        options: &SolveConfig,
        resources: &ExecutionResources,
        observations: &mut impl ExecutionSink,
    ) -> Result<Self, SolveError> {
        let context = resources
            .gpu_for(options.backend)
            .map_err(SolveError::Gpu)?;
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
            batch_size: options.batch_size,
            completion_workers: options.completion_workers,
        })?;
        Ok(Self::Hybrid {
            oracle: Box::new(oracle),
            queue: Box::new(crate::formula_queue::BatchQueue::new(options)?),
            statistics: Box::new(FormulaExecutionStatistics {
                adapter,
                gpu_limits: Some(options.into()),
                ..Default::default()
            }),
        })
    }
}

impl MembershipExecution for Execution {
    fn statistics(&self, models: &StableModels) -> Option<FormulaExecutionStatistics> {
        match self {
            Self::Cpu => {
                let _ = models;
                None
            }
            Self::Batched { queue } => Some(batch_statistics(
                queue,
                models,
                FormulaExecutionStatistics::default(),
            )),
            #[cfg(feature = "gpu")]
            Self::Hybrid {
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

    fn next(
        &mut self,
        models: &mut StableModels,
        options: &SolveConfig,
        control: &zetesis_cpu::Control,
        phases: &Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        match self {
            Self::Cpu => {
                let _ = options;
                let _ = control;
                let _ = phases;
                models.next().map(|result| result.map_err(Failure::Search))
            }
            Self::Batched { queue } => queue.next(models, options, control, |_, candidates| {
                let mut verdicts = Vec::new();
                verdicts
                    .try_reserve_exact(candidates.len())
                    .map_err(|_| Failure::Search(Incomplete::Allocation))?;
                verdicts.resize(candidates.len(), zetesis_sat::BatchVerdict::Residual);
                Ok(verdicts)
            }),
            #[cfg(feature = "gpu")]
            Self::Hybrid {
                oracle,
                queue,
                statistics,
            } => queue.next(models, options, control, |theory, candidates| {
                propagate(
                    oracle, statistics, theory, candidates, options, control, phases,
                )
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
    control: &zetesis_cpu::Control,
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
        oracle.propagate_batch_with_control(theory, candidates, limits, control)
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
        verdicts.push(match check.verdict() {
            zetesis_wgpu::FormulaVerdict::NotModel => zetesis_sat::BatchVerdict::NotModel,
            zetesis_wgpu::FormulaVerdict::NoProperSubset => {
                zetesis_sat::BatchVerdict::NoProperSubset
            }
            zetesis_wgpu::FormulaVerdict::Residual(_) => zetesis_sat::BatchVerdict::Residual,
        });
    }
    Ok(verdicts)
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
fn record_submission(
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
fn add(counter: &mut u64, value: u64) -> Result<(), Failure> {
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
        control: &zetesis_cpu::Control,
        phases: &Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        (**self).next(models, config, control, phases)
    }
    fn statistics(&self, models: &StableModels) -> Option<FormulaExecutionStatistics> {
        (**self).statistics(models)
    }
}

#[cfg(all(test, feature = "gpu"))]
#[path = "../tests/support/formula_resources.rs"]
mod resource_tests;
