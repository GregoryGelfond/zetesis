//! Candidate-dependent proper-subset queries of the frozen reduct, over reusable
//! worker allocations.

mod state;
pub(crate) use state::State;

use std::mem::size_of;

use zetesis_ferraris::{
    EvaluationError, EvaluationLimits, EvaluationWorkspace, FormulaEvaluation, Interpretation,
};

use crate::timing::{self, Phase};
use crate::{Cancellation, Check, Incomplete, Limits, Statistics};

/// Default ceiling for one query workspace's retained capacity.
pub(crate) const DEFAULT_BYTES: u64 = 64 * 1024 * 1024;

/// Cumulative actual reduct-query activity within an enumeration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ReductStatistics {
    /// Actual original-node/root evaluation work of entered queries.
    /// Proposal and independent returned-witness evaluations are separate calls.
    pub original_work: u64,
    /// Entered attempts to classify and close a candidate's positive reduct.
    pub frozen_positive_attempts: u64,
    /// Attempts completed by positive closure, without a subset-region query.
    pub frozen_positive_completed: u64,
    /// Frozen-positive incidence construction, included in frozen-positive work.
    pub frozen_positive_preparation_work: u64,
    /// Charged frozen-positive preparation and checking; included in search work.
    pub frozen_positive_work: u64,
    /// Largest actual single-worker retained capacity observed on query return,
    /// including failure. Temporary witness vectors are excluded; this is not
    /// the aggregate completion peak or process RSS.
    pub peak_workspace_bytes: u128,
    /// What the proper-subset queries walked as region trees.
    pub regions: crate::RegionCounts,
}

/// Reusable worker allocations for proper-subset queries.
///
/// Before each entered query, original truth is recomputed into the evaluation
/// workspace; the region query's worklists and the frozen-positive workspace are
/// reused. No candidate-derived assignment is reused.
#[derive(Debug, Default)]
pub(crate) struct ReductWorkspace {
    evaluation: EvaluationWorkspace,
    /// The worklists every region query of this workspace reuses.
    narrowing: zetesis_ferraris::NarrowingScratch,
    positive: zetesis_ferraris::FrozenPositiveWorkspace,
}

/// Disjoint original truth and query scratch, with the other retained bytes
/// charged before an optional frozen-positive workspace is allowed to grow.
pub(crate) struct RegionEvaluation<'a> {
    pub(crate) truth: FormulaEvaluation<'a>,
    pub(crate) narrowing: &'a mut zetesis_ferraris::NarrowingScratch,
    pub(crate) positive: &'a mut zetesis_ferraris::FrozenPositiveWorkspace,
    pub(crate) positive_other_bytes: u128,
}

impl ReductWorkspace {
    /// Evaluate the candidate's original truth into this workspace: the
    /// truth as `evaluate_truth` gives it, under this workspace's ceiling
    /// and with its evaluation workspace, lending the region query's
    /// worklists beside it.
    pub(crate) fn evaluate<'a>(
        &'a mut self,
        candidate: &'a Interpretation,
        limits: Limits,
        cancellation: &Cancellation,
        statistics: &mut Statistics,
    ) -> Result<RegionEvaluation<'a>, Incomplete> {
        let max_bytes = limits.max_reduct_bytes;
        bound(self.retained_bytes(), max_bytes)?;
        let other_bytes = self.retained_bytes() - self.evaluation.retained_bytes();
        let (truth, evaluation_bytes) = evaluate_truth(
            &mut self.evaluation,
            other_bytes,
            max_bytes,
            candidate,
            limits,
            cancellation,
            statistics,
        )?;
        let positive_other_bytes = other_bytes - self.positive.retained_bytes() + evaluation_bytes;
        Ok(RegionEvaluation {
            truth,
            narrowing: &mut self.narrowing,
            positive: &mut self.positive,
            positive_other_bytes,
        })
    }

    /// Record the capacity retained by a region query, including a failed
    /// attempt's growth, and admit it before returning a successful verdict.
    /// An earlier query failure keeps its original typed cause.
    pub(crate) fn finish_check(
        &self,
        result: Result<Check, Incomplete>,
        max_bytes: u64,
        statistics: &mut Statistics,
    ) -> Result<Check, Incomplete> {
        let retained = self.retained_bytes();
        statistics.reduct.peak_workspace_bytes =
            statistics.reduct.peak_workspace_bytes.max(retained);
        result.and_then(|verdict| {
            bound(retained, max_bytes)?;
            Ok(verdict)
        })
    }

    /// Actual retained worker storage: the workspace header and the actual
    /// capacities of its evaluation, worklists and frozen-positive workspace.
    /// Allocator metadata, allocation overlap, transient witness vectors and
    /// thread stacks are excluded.
    pub(crate) fn retained_bytes(&self) -> u128 {
        header_bytes()
            + self.evaluation.retained_bytes()
            + self.narrowing.retained_bytes()
            + self.positive.retained_bytes()
    }
}

/// Evaluate the candidate's original truth into the evaluation workspace
/// under the workspace's retained-storage ceiling, charging the work to the
/// reduct receipts. The truth is the frozen mask of the candidate's reduct;
/// the second value is the evaluation's retained storage.
fn evaluate_truth<'a>(
    evaluation: &'a mut EvaluationWorkspace,
    other_bytes: u128,
    max_bytes: u64,
    candidate: &'a Interpretation,
    limits: Limits,
    cancellation: &Cancellation,
    statistics: &mut Statistics,
) -> Result<(FormulaEvaluation<'a>, u128), Incomplete> {
    let evaluation_limit =
        usize::try_from(u128::from(max_bytes) - other_bytes).unwrap_or(usize::MAX);
    let started = timing::start(statistics.phase_timings.as_ref());
    let evaluated = evaluation.evaluate(
        candidate,
        EvaluationLimits {
            max_work: limits.max_verification_work,
            max_bytes: evaluation_limit,
        },
        cancellation,
    );
    timing::finish(
        &mut statistics.phase_timings,
        Phase::OriginalValidation,
        started,
    );
    statistics.reduct.original_work = statistics
        .reduct
        .original_work
        .checked_add(evaluated.work)
        .ok_or(Incomplete::CounterOverflow)?;
    let truth = evaluated.result.map_err(|error| match error {
        EvaluationError::Stopped(stop) => Incomplete::from(stop),
        EvaluationError::Storage { required, .. } => Incomplete::ReductStorage {
            required: other_bytes + required,
            limit: u128::from(max_bytes),
        },
    })?;
    Ok((truth, evaluated.retained_bytes))
}

fn header_bytes() -> u128 {
    // Nested workspace methods already include their headers.
    (size_of::<ReductWorkspace>()
        - size_of::<EvaluationWorkspace>()
        - size_of::<zetesis_ferraris::FrozenPositiveWorkspace>()) as u128
}

fn bound(required: u128, limit: u64) -> Result<(), Incomplete> {
    if required > u128::from(limit) {
        Err(Incomplete::ReductStorage {
            required,
            limit: u128::from(limit),
        })
    } else {
        Ok(())
    }
}
