//! Device execution of a complete original-theory tight certificate.

use std::sync::Arc;

use crate::execution_observation::ExecutionSink;
use crate::formula_execution::{
    Execution, Failure, FormulaExecutionStatistics, add, record_submission,
};
use crate::formula_queue::device::take_slate_prefix;
use crate::phase_timing::{Recorder, SolvePhase};
use crate::{ExecutionObservation, SolveConfig, SolveError};
use zetesis_algebra::Cancellation;
use zetesis_ferraris::{Interpretation, TightPlan, TightVerdict};
use zetesis_stable::{BatchVerdict, Incomplete};
use zetesis_wgpu::{GpuDeviceSource, GpuTightOracle, TightGpuError, TightGpuLimits};

impl Execution {
    pub(super) fn tight(
        options: &SolveConfig,
        source: GpuDeviceSource<'_>,
        plan: Arc<TightPlan>,
        control: &Cancellation,
        observations: &mut impl ExecutionSink,
    ) -> Result<Self, SolveError> {
        let oracle = GpuTightOracle::from_source(source).map_err(SolveError::Gpu)?;
        observations.record(ExecutionObservation::DeviceTight {
            adapter: oracle.info().metadata(),
            grounder: options.grounder,
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
    let limits = limits(options, candidates.len());
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
        let checks = result.map_err(tight_failure)?;
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

/// Tight support's device side of a GPU enumeration's two-phase slates,
/// recording each submission's receipts as [`check`] does.
pub(crate) struct TightDevice<'a> {
    pub(crate) oracle: &'a mut GpuTightOracle,
    pub(crate) plan: &'a TightPlan,
    pub(crate) statistics: &'a mut FormulaExecutionStatistics,
    pub(crate) options: &'a SolveConfig,
}

impl crate::formula_queue::device::SlateDevice for TightDevice<'_> {
    type Prepared<'d>
        = zetesis_wgpu::PreparedTightSubmission<'d>
    where
        Self: 'd;
    type Checked = Result<zetesis_wgpu::CheckedSlate<zetesis_wgpu::TightGpuCheck>, TightGpuError>;

    fn prepare<'d>(
        &'d mut self,
        slate: zetesis_ferraris::SlateRef<'_>,
        offset: usize,
        _max_bytes: u64,
        cancellation: &Cancellation,
    ) -> Result<Self::Prepared<'d>, Failure> {
        let limits = limits(self.options, slate.candidates().len());
        self.oracle
            .prepare(self.plan, slate, offset, limits, cancellation)
            .map_err(tight_failure)
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
        // The primitive submits one ordered batch, even if its readback fails.
        // Retain that receipt before propagating the original device error.
        let activity = self.oracle.activity();
        let submission = record_submission(
            self.statistics,
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
        let scheduled = add(
            self.statistics.gpu_scheduled_work.get_or_insert(0),
            attributed,
        );
        let checks = checked.map_err(tight_failure)?;
        submission?;
        scheduled?;
        let (checks, finish_wait) = take_slate_prefix(checks, theory, slate, offset, offered)?;
        if let Some(batch) = self.oracle.last_batch_stats() {
            add(&mut self.statistics.gpu_batches, batch.dispatches)?;
            add(&mut self.statistics.gpu_candidates, batch.candidates)?;
            add(&mut self.statistics.gpu_work, batch.work)?;
            self.statistics.peak_accounted_bytes = self
                .statistics
                .peak_accounted_bytes
                .max(batch.accounted_bytes);
            super::formula_execution::record_workgroup_size(self.statistics, batch.workgroup_size);
            super::formula_execution::record_tight_routes(self.statistics, batch)?;
        }
        let mut verdicts = Vec::new();
        verdicts
            .try_reserve_exact(checks.len())
            .map_err(|_| Failure::Search(Incomplete::Allocation))?;
        verdicts.extend(checks.into_iter().map(|check| verdict(check.verdict())));
        for verdict in verdicts {
            rows.classified(verdict)?;
        }
        Ok(crate::formula_queue::device::Decoded {
            compute,
            finish_wait,
        })
    }
}

impl crate::formula_queue::device::PreparedSubmission
    for zetesis_wgpu::PreparedTightSubmission<'_>
{
    type Checked = Result<zetesis_wgpu::CheckedSlate<zetesis_wgpu::TightGpuCheck>, TightGpuError>;

    fn count(&self) -> usize {
        Self::count(self)
    }

    fn check(self, cancellation: &Cancellation) -> Self::Checked {
        Self::check(self, cancellation)
    }

    fn check_overlapping<R>(
        self,
        cancellation: &Cancellation,
        during: impl FnOnce() -> R,
    ) -> (Self::Checked, Option<R>) {
        Self::check_overlapping(self, cancellation, during)
    }
}

/// A tight device operation's failure: a stop of the control is the search's,
/// any other is the run's.
fn tight_failure(error: TightGpuError) -> Failure {
    match error {
        TightGpuError::Stopped(stop) => Failure::Search(stop.into()),
        TightGpuError::Gpu(error) => Failure::Run(SolveError::Gpu(error)),
    }
}

/// The device limits of one tight check over a slate of `candidates`, which
/// the one-phase check and the two-phase preparation both take.
///
/// As for general propagation, one submission may take the whole slate under
/// either batching policy; the device's admission bounds its prefix. The update
/// state may not shrink that prefix below the batch size, the slates' floor: a
/// shorter prefix would split every slate into submissions of alternating
/// counts, which hold no reuse authority. The byte ceiling is the tight batch's
/// own, [`SolveConfig::max_tight_batch_bytes`], which [`crate::Resources`]
/// derives as twice the other device batches' share, so that the state reaches
/// that floor under smaller allowances.
fn limits(options: &SolveConfig, candidates: usize) -> TightGpuLimits {
    TightGpuLimits {
        max_candidates: candidates,
        max_batch_bytes: options.max_tight_batch_bytes,
        max_work_per_candidate: u64::from(options.gpu_formula_work),
        min_update_candidates: options.batch_size.get(),
        ..Default::default()
    }
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
