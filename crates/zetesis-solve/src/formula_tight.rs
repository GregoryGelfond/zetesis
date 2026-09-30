//! Device execution of a complete original-theory tight certificate.

use std::sync::Arc;

use crate::execution_observation::ExecutionSink;
use crate::formula_execution::{
    Execution, Failure, FormulaExecutionStatistics, add, record_submission,
};
use crate::phase_timing::{Recorder, SolvePhase};
use crate::{ExecutionObservation, SolveConfig, SolveError};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, TightPlan, TightVerdict};
use zetesis_sat::{BatchVerdict, Incomplete};
use zetesis_wgpu::{GpuContext, GpuOptions, GpuTightOracle, TightGpuError, TightGpuLimits};

impl Execution {
    pub(super) fn tight(
        options: &SolveConfig,
        context: Option<&GpuContext>,
        plan: Arc<TightPlan>,
        observations: &mut impl ExecutionSink,
    ) -> Result<Self, SolveError> {
        let oracle = match context {
            Some(context) => GpuTightOracle::from_context(context),
            None => GpuTightOracle::new_selected(
                GpuOptions::default(),
                crate::engine::selection(options.backend),
            ),
        }
        .map_err(SolveError::Gpu)?;
        observations.record(ExecutionObservation::DeviceTight {
            adapter: oracle.info().metadata(),
            grounder: options.grounder,
            search: options.search,
            batch_size: options.batch_size,
        })?;
        let adapter = format!(
            "{}, {}; vendor=0x{:04x}",
            oracle.info().name(),
            oracle.info().backend(),
            oracle.info().vendor_id(),
        );
        Ok(Self::Tight {
            oracle: Box::new(oracle),
            plan,
            queue: Box::new(crate::formula_queue::BatchQueue::for_complete_oracle(
                options,
            )?),
            statistics: Box::new(FormulaExecutionStatistics {
                adapter,
                tight_work_per_candidate: Some(u64::from(options.gpu_formula_work)),
                gpu_scheduled_work: Some(0),
                ..Default::default()
            }),
        })
    }
}

pub(crate) fn check(
    oracle: &mut GpuTightOracle,
    plan: &TightPlan,
    statistics: &mut FormulaExecutionStatistics,
    candidates: &[Interpretation],
    options: &SolveConfig,
    cancellation: &Cancellation,
    phases: &Recorder,
) -> Result<Vec<BatchVerdict>, Failure> {
    let limits = TightGpuLimits {
        max_candidates: options.batch_size.get(),
        max_batch_bytes: options.max_batch_bytes,
        max_work_per_candidate: u64::from(options.gpu_formula_work),
        ..Default::default()
    };
    let mut verdicts = Vec::new();
    verdicts
        .try_reserve_exact(candidates.len())
        .map_err(|_| Failure::Search(Incomplete::Allocation))?;
    let result = phases.measure(SolvePhase::GpuHostOracle, || {
        oracle.check_batch(plan, candidates, limits, cancellation)
    });
    // The primitive submits one ordered batch, even if its readback fails.
    // Retain that receipt before propagating the original device error.
    let activity = oracle.activity();
    let submission = record_submission(
        statistics,
        (activity.submissions != 0).then_some(candidates.len()),
    );
    let scheduled = add(
        statistics.gpu_scheduled_work.get_or_insert(0),
        activity.scheduled_work,
    );
    let checks = result.map_err(|error| match error {
        TightGpuError::Stopped(stop) => Failure::Search(stop.into()),
        TightGpuError::Gpu(error) => Failure::Run(SolveError::Gpu(error)),
    })?;
    submission?;
    scheduled?;
    if let Some(batch) = oracle.last_batch_stats() {
        add(&mut statistics.gpu_batches, batch.dispatches)?;
        add(&mut statistics.gpu_candidates, batch.candidates)?;
        add(&mut statistics.gpu_work, batch.work)?;
        statistics.peak_accounted_bytes =
            statistics.peak_accounted_bytes.max(batch.accounted_bytes);
    }
    verdicts.extend(checks.into_iter().map(|check| verdict(check.verdict())));
    Ok(verdicts)
}

fn verdict(verdict: TightVerdict) -> BatchVerdict {
    match verdict {
        TightVerdict::Stable => BatchVerdict::NoProperSubset,
        TightVerdict::NotModel { .. } => BatchVerdict::NotModel,
        // Complete producer coverage makes failed support a reduct refutation
        // (TightPlans.stable_supported). The primitive publishes no deletion
        // witness, hence its Residual name; this protocol accepts that proof.
        TightVerdict::Residual { .. } => BatchVerdict::Refuted,
    }
}

#[cfg(test)]
mod tests;
