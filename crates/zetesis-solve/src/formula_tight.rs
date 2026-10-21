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
        control: &Cancellation,
        observations: &mut impl ExecutionSink,
    ) -> Result<Self, SolveError> {
        let oracle = match context {
            Some(context) => GpuTightOracle::from_context(context),
            None if options.stats => GpuTightOracle::new_selected_with_timing(
                GpuOptions::default(),
                crate::engine::selection(options.backend),
            ),
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
        let gpu_compute_timing =
            zetesis_backend::ComputeTiming::new(oracle.context().compute_timing());
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
                gpu_compute_timing,
                ..Default::default()
            }),
            settlement: crate::formula_execution::Settlement::new(
                control,
                TightGpuLimits::default().timeout,
            ),
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
) -> Result<crate::batch_executor::DeviceVerdicts, Failure> {
    // As for general propagation, one submission may take the whole slate
    // under either batching policy; the device's admission bounds its prefix.
    // The update state may not shrink that prefix below the batch size, the
    // slates' floor: a shorter prefix would split every slate into submissions
    // of alternating counts, which hold no reuse authority.
    let limits = TightGpuLimits {
        max_candidates: candidates.len(),
        max_batch_bytes: options.max_batch_bytes,
        max_work_per_candidate: u64::from(options.gpu_formula_work),
        min_update_candidates: options.batch_size.get(),
        ..Default::default()
    };
    let mut verdicts =
        crate::batch_executor::DeviceVerdicts::new(candidates.len()).map_err(Failure::Search)?;
    while verdicts.values.len() < candidates.len() {
        let pending = &candidates[verdicts.values.len()..];
        let result = phases.measure(SolvePhase::GpuHostOracle, || {
            oracle.check_prefix(plan, pending, limits, cancellation)
        });
        // A failed call can still report the batch before it, as unavailable.
        if let Some(report) = oracle.reported_compute_time() {
            statistics.gpu_compute_timing.record(report);
        }
        // The primitive submits one ordered batch, even if its readback fails.
        // Retain that receipt before propagating the original device error.
        let activity = oracle.activity();
        let submission = record_submission(
            statistics,
            (activity.submissions != 0).then_some(
                usize::try_from(activity.submitted_candidates)
                    .map_err(|_| Failure::Search(Incomplete::CounterOverflow))?,
            ),
        );
        // A validated readback reports the work performed, never more than was
        // reserved; a submission without one keeps its reservation.
        let attributed = if activity.submissions != 0
            && activity.completed_candidates == activity.submitted_candidates
        {
            activity.completed_work
        } else {
            activity.scheduled_work
        };
        let scheduled = add(statistics.gpu_scheduled_work.get_or_insert(0), attributed);
        let checks = result.map_err(|error| match error {
            TightGpuError::Stopped(stop) => Failure::Search(stop.into()),
            TightGpuError::Gpu(error) => Failure::Run(SolveError::Gpu(error)),
        })?;
        submission?;
        scheduled?;
        let count = checks.checks().len();
        if count == 0 || count > pending.len() {
            return Err(Failure::Run(SolveError::FormulaBatchShape {
                expected: pending.len(),
                actual: count,
            }));
        }
        verdicts.record_prefix(pending.len(), count);
        let checks = checks
            .into_checks(plan.theory(), &pending[..count])
            .map_err(|error| Failure::Run(SolveError::Gpu(error)))?;
        if let Some(batch) = oracle.last_batch_stats() {
            add(&mut statistics.gpu_batches, batch.dispatches)?;
            add(&mut statistics.gpu_candidates, batch.candidates)?;
            add(&mut statistics.gpu_work, batch.work)?;
            statistics.peak_accounted_bytes =
                statistics.peak_accounted_bytes.max(batch.accounted_bytes);
            super::formula_execution::record_workgroup_size(statistics, batch.workgroup_size);
            super::formula_execution::record_tight_routes(statistics, batch)?;
        }
        verdicts
            .values
            .extend(checks.into_iter().map(|check| verdict(check.verdict())));
    }
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
