//! Candidate-dependent proper-subset queries of the frozen reduct, over reusable
//! worker allocations.

mod state;
pub(crate) use state::State;

use std::mem::size_of;

use zetesis_ferraris::{
    EvaluationError, EvaluationLimits, EvaluationWorkspace, FormulaEvaluation, Interpretation,
};

use crate::timing::{self, Phase};
use crate::{Cancellation, Incomplete, Limits, Statistics};

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
    /// What the queries' inner preparations found and cost.
    pub inner_preparation: InnerPreparationStatistics,
}

/// Cumulative receipts of the candidates' inner preparations: the parity rows
/// and units of each candidate's countermodel class, probed from its closed
/// query root before its walk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct InnerPreparationStatistics {
    /// Candidates whose query root closed without refutation and were offered a preparation.
    pub attempts: u64,
    /// Preparations that installed units or rows.
    pub prepared: u64,
    /// Preparations the lease declined; their walk proceeded without one.
    pub declined: u64,
    /// Guards whose probe reached the candidate: atoms cut at the query root.
    pub saturation_units: u64,
    /// Parity groups proposed.
    pub groups: u64,
    /// Probes run.
    pub probes: u64,
    /// Patterns whose probe was refuted.
    pub refuted_patterns: u64,
    /// Parity rows loaded.
    pub rows: u64,
    /// Probed groups whose allowed patterns are not affine.
    pub inexact_hulls: u64,
    /// Preparations whose rows refuted the query root: candidates decided stable there.
    pub root_refuted: u64,
    /// Work charged to the search by the preparations.
    pub work: u64,
}

impl InnerPreparationStatistics {
    /// Record one attempt's receipt.
    ///
    /// # Errors
    /// A counter beyond its width.
    pub(crate) fn record(
        &mut self,
        attempt: &zetesis_ferraris::FrozenPreparationStatistics,
    ) -> Result<(), Incomplete> {
        use zetesis_ferraris::PreparationStatus;
        let (prepared, declined) = match attempt.status {
            PreparationStatus::Prepared => (1, 0),
            PreparationStatus::WorkDeclined | PreparationStatus::StorageDeclined => (0, 1),
            PreparationStatus::NotPrepared | PreparationStatus::Incomplete => (0, 0),
        };
        self.merge(&Self {
            attempts: 1,
            prepared,
            declined,
            saturation_units: attempt.saturation_units,
            groups: attempt.groups,
            probes: attempt.probes,
            refuted_patterns: attempt.refuted_patterns,
            rows: attempt.parity.rows,
            inexact_hulls: attempt.inexact_hulls,
            root_refuted: u64::from(attempt.root_refuted),
            work: attempt.preparation_work,
        })
    }

    /// Add another receipt's counts.
    ///
    /// # Errors
    /// A counter beyond its width.
    pub(crate) fn merge(&mut self, other: &Self) -> Result<(), Incomplete> {
        for (counter, value) in [
            (&mut self.attempts, other.attempts),
            (&mut self.prepared, other.prepared),
            (&mut self.declined, other.declined),
            (&mut self.saturation_units, other.saturation_units),
            (&mut self.groups, other.groups),
            (&mut self.probes, other.probes),
            (&mut self.refuted_patterns, other.refuted_patterns),
            (&mut self.rows, other.rows),
            (&mut self.inexact_hulls, other.inexact_hulls),
            (&mut self.root_refuted, other.root_refuted),
            (&mut self.work, other.work),
        ] {
            *counter = counter
                .checked_add(value)
                .ok_or(Incomplete::CounterOverflow)?;
        }
        Ok(())
    }

    /// The sum of two receipts, saturating at the counters' width.
    #[must_use]
    pub(crate) const fn saturating_add(self, other: Self) -> Self {
        Self {
            attempts: self.attempts.saturating_add(other.attempts),
            prepared: self.prepared.saturating_add(other.prepared),
            declined: self.declined.saturating_add(other.declined),
            saturation_units: self.saturation_units.saturating_add(other.saturation_units),
            groups: self.groups.saturating_add(other.groups),
            probes: self.probes.saturating_add(other.probes),
            refuted_patterns: self.refuted_patterns.saturating_add(other.refuted_patterns),
            rows: self.rows.saturating_add(other.rows),
            inexact_hulls: self.inexact_hulls.saturating_add(other.inexact_hulls),
            root_refuted: self.root_refuted.saturating_add(other.root_refuted),
            work: self.work.saturating_add(other.work),
        }
    }
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
    pub(crate) fn finish_check<T>(
        &self,
        result: Result<T, Incomplete>,
        max_bytes: u64,
        statistics: &mut Statistics,
    ) -> Result<T, Incomplete> {
        let retained = self.retained_bytes();
        statistics.reduct.peak_workspace_bytes =
            statistics.reduct.peak_workspace_bytes.max(retained);
        result.and_then(|verdict| {
            bound(retained, max_bytes)?;
            Ok(verdict)
        })
    }

    /// Lend the region query's worklists to `narrow`, a narrowing of one
    /// region of a query another worker published, under the ceiling an
    /// inline query's worklists grow under: a workspace already beyond it is
    /// refused first, and its capacity afterwards is recorded and admitted as
    /// [`Self::finish_check`] admits a query's.
    pub(crate) fn lend_narrowing<T>(
        &mut self,
        max_bytes: u64,
        statistics: &mut Statistics,
        narrow: impl FnOnce(
            &mut zetesis_ferraris::NarrowingScratch,
            &mut Statistics,
        ) -> Result<T, Incomplete>,
    ) -> Result<T, Incomplete> {
        bound(self.retained_bytes(), max_bytes)?;
        let result = narrow(&mut self.narrowing, statistics);
        self.finish_check(result, max_bytes, statistics)
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
