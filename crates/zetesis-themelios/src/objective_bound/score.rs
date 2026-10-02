//! Read the retained eligibility DAG for one original-owner interpretation.

use super::{
    ObjectiveBoundError, ObjectiveBoundStatistics, ObjectivePlan, ObjectivePlanLimits, Work,
};
use std::fmt;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Node};
use zetesis_objective::{Limits, Score};

/// A score-only read of the prepared eligibility, without tuple evidence.
#[derive(Debug)]
pub struct ObjectiveScore {
    score: Score,
    work: u64,
}
impl ObjectiveScore {
    /// Transfer the complete score, retaining presence and inactive slots.
    #[must_use]
    pub fn into_score(self) -> Score {
        self.score
    }
    /// Accepted DAG and numeric reduction work for this read.
    #[must_use]
    pub const fn work(&self) -> u64 {
        self.work
    }
}

/// The original typed cause of an incomplete prepared score.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectiveScoreErrorKind {
    /// Equal atom counts do not authorize a different immutable theory.
    WrongTheory,
    /// Eligibility inspection, control or storage refused.
    Eligibility(ObjectiveBoundError),
    /// Numeric reduction refused without a complete score.
    Reduction(zetesis_objective::Error),
}

/// Incomplete prepared scoring with its complete accepted work prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectiveScoreError {
    kind: ObjectiveScoreErrorKind,
    work: u64,
}
impl ObjectiveScoreError {
    /// Structured cause; no successful score accompanies this error.
    #[must_use]
    pub const fn kind(self) -> ObjectiveScoreErrorKind {
        self.kind
    }
    /// Eligibility and reduction work accepted before the refusal.
    #[must_use]
    pub const fn work(self) -> u64 {
        self.work
    }
}
impl fmt::Display for ObjectiveScoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ObjectiveScoreErrorKind::WrongTheory => {
                formatter.write_str("prepared objective score belongs to a different theory")
            }
            ObjectiveScoreErrorKind::Eligibility(error) => error.fmt(formatter),
            ObjectiveScoreErrorKind::Reduction(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for ObjectiveScoreError {}

impl ObjectivePlan {
    /// Score a candidate using the already coalesced keys and exact eligibility.
    /// The candidate must retain this plan's original theory identity. Scoring
    /// does not verify stability or the caller's completed-catalog coverage.
    ///
    /// `None` declines this path when the complete possible population cannot
    /// establish the caller's binding/key/byte ceilings. The ordinary detailed
    /// evaluator can then apply those ceilings to just the selected model. No
    /// work is consumed by that decline; cancellation and identity are checked.
    /// The retained plan supplies numeric keys only, so contribution evidence
    /// always uses the ordinary evaluator.
    ///
    /// One unit is charged per DAG node, followed by the numeric reducer's work.
    /// Priority lookup uses logarithmic map probes; the reducer charges one
    /// logical priority visit, not each map comparison.
    /// Temporary storage is linear in retained nodes and priority slots, bounded
    /// by the successful plan's admission. There is no new key or tuple cache.
    ///
    /// # Errors
    /// Returns exact-owner, control, work, allocation or cost overflow failures
    /// with their complete accepted work prefix; no partial score is published.
    pub fn score(
        &self,
        candidate: &Interpretation,
        limits: Limits,
        cancellation: &Cancellation,
    ) -> Result<Option<ObjectiveScore>, ObjectiveScoreError> {
        let mut work = Work {
            cancellation,
            limits: ObjectivePlanLimits {
                max_work: limits.max_work,
                ..ObjectivePlanLimits::default()
            },
            template: None,
            statistics: ObjectiveBoundStatistics::default(),
        };
        work.charge(0).map_err(eligibility_error)?;
        if !self.original.same_instance(candidate.theory()) {
            return Err(ObjectiveScoreError {
                kind: ObjectiveScoreErrorKind::WrongTheory,
                work: 0,
            });
        }
        if self.statistics.bindings > limits.max_bindings
            || self.statistics.keys > limits.max_keys
            || self.statistics.key_bytes > limits.max_key_bytes
        {
            return Ok(None);
        }
        let mut values = work.reserve(self.nodes.len()).map_err(eligibility_error)?;
        for node in &self.nodes {
            work.tick().map_err(eligibility_error)?;
            let value = match *node {
                Node::False => false,
                Node::Atom(atom) => candidate.contains(atom),
                Node::And(left, right) => values[left] && values[right],
                Node::Or(left, right) => values[left] || values[right],
                Node::Implies(left, right) => !values[left] || values[right],
            };
            values.push(value);
        }
        let truth = values.as_slice();
        let reduced = zetesis_objective::reduce_costs(
            &self.objectives,
            |priority| {
                self.levels
                    .get(&priority)
                    .into_iter()
                    .flatten()
                    .map(move |element| truth[element.condition].then_some(element.weight))
            },
            limits.max_work - work.statistics.work,
            cancellation,
        )
        .map_err(|error| ObjectiveScoreError {
            work: work.statistics.work + error.statistics().work,
            kind: ObjectiveScoreErrorKind::Reduction(error),
        })?;
        Ok(Some(ObjectiveScore {
            work: work.statistics.work + reduced.work(),
            score: reduced.into_score(),
        }))
    }
}

fn eligibility_error(error: ObjectiveBoundError) -> ObjectiveScoreError {
    ObjectiveScoreError {
        work: error.statistics().work,
        kind: ObjectiveScoreErrorKind::Eligibility(error),
    }
}
