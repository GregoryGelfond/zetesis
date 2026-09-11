//! Ordinary-invocation GPU propagation with exact native residual completion.

use crate::ExecutionObservation as Event;
use crate::execution_observation::ExecutionSink;

use zetesis_ferraris::Interpretation;
use zetesis_sat::{Incomplete, StableModels};

use crate::phase_timing::Recorder;
#[cfg(feature = "gpu")]
use crate::phase_timing::SolvePhase;
use crate::{Backend, RunError, SolveConfig};

pub use crate::completion_accounting::CompletionAccounting;

/// Actual formula GPU work and explicitly retained candidate results.
#[derive(Clone, Debug, Default)]
pub struct FormulaExecutionStatistics {
    /// Physical adapter name and native API selected during this invocation.
    pub adapter: String,
    /// Bounded batch completion accounting; scalar cursor execution has no record.
    pub completion: CompletionAccounting,
    /// Successfully dispatched and decoded GPU batches.
    pub gpu_batches: u64,
    /// Candidate worlds returned by successful GPU dispatches.
    pub gpu_candidates: u64,
    /// Charged propagation work; not time or GPU instruction count.
    pub gpu_work: u64,
    /// Sum of completed per-candidate propagation sweeps.
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
    Run(RunError),
}

/// A stream of original, reduct-checked semantic models, before display or scoring.
pub(crate) trait MembershipExecution {
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
    pub(crate) fn new(
        options: &SolveConfig,
        observations: &mut impl ExecutionSink,
    ) -> Result<Self, RunError> {
        if matches!(options.backend, Backend::Auto | Backend::Cpu) {
            observations.record(Event::CpuFormula {
                oracle: options.oracle,
                grounder: options.grounder,
            })?;
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
        Self::gpu(options, observations)
    }

    #[cfg(not(feature = "gpu"))]
    fn gpu(_: &SolveConfig, _: &mut impl ExecutionSink) -> Result<Self, RunError> {
        Err(RunError::BackendUnavailable)
    }

    #[cfg(feature = "gpu")]
    fn gpu(options: &SolveConfig, observations: &mut impl ExecutionSink) -> Result<Self, RunError> {
        let oracle = zetesis_wgpu::GpuFormulaOracle::new_selected(
            zetesis_wgpu::GpuOptions::default(),
            crate::engine::selection(options.backend),
        )
        .map_err(RunError::Gpu)?;
        let adapter = format!(
            "{}, {}; vendor=0x{:04x}",
            oracle.info().name(),
            oracle.info().backend(),
            oracle.info().vendor_id()
        );
        observations.record(Event::DeviceFormula {
            adapter: oracle.info().metadata(),
            grounder: options.grounder,
            batch_size: options.batch_size,
            completion_workers: options.completion_workers,
        })?;
        Ok(Self::Hybrid {
            oracle: Box::new(oracle),
            queue: Box::new(crate::formula_queue::BatchQueue::new(options)?),
            statistics: Box::new(FormulaExecutionStatistics {
                adapter,
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
                propagate(oracle, statistics, theory, candidates, options, phases)
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
    phases: &Recorder,
) -> Result<Vec<zetesis_sat::BatchVerdict>, Failure> {
    let limits = zetesis_wgpu::FormulaLimits {
        max_candidates: options.batch_size.get(),
        max_batch_bytes: options.max_batch_bytes,
        max_work_per_candidate: u32::try_from(options.max_work).unwrap_or(u32::MAX),
        ..Default::default()
    };
    // The device API returns one ordered verdict per candidate. Reserve its
    // host conversion before dispatch so allocation failure cannot hide a
    // successfully decoded batch from the execution counters.
    let mut verdicts = Vec::new();
    verdicts
        .try_reserve_exact(candidates.len())
        .map_err(|_| Failure::Search(Incomplete::Allocation))?;
    let checks = phases
        .measure(SolvePhase::GpuHostOracle, || {
            oracle.propagate_batch(theory, candidates, limits)
        })
        .map_err(|e| Failure::Run(RunError::Gpu(e)))?;
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
