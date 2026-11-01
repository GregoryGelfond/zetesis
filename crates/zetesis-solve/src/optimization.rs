//! Bounded incumbent retention; optimality requires complete relevant search.

use std::cmp::Ordering;
use std::fmt;

use zetesis_algebra::Cancellation;
use zetesis_algebra::reduction::Minimum;
use zetesis_algebra::reduction::{CheckedSum, Reduction as _};
use zetesis_core::Model;
use zetesis_core::retention::{ModelRetention, RetentionError};
use zetesis_ferraris::Interpretation;
use zetesis_objective::{ObjectiveProgram, Score};
use zetesis_themelios::objective_bound::ObjectivePlan;

use crate::{Interruption, SolveConfig};

/// A completely evaluated incumbent, not by itself an optimality certificate.
#[derive(Clone, Debug)]
pub struct Optimization {
    /// Best retained cost, at descending priorities. A storage interruption may
    /// preserve an earlier model even after a better score was evaluated.
    pub score: Score,
    /// Number of full stable models found with this cost, including hidden
    /// ties. After exhaustion it counts every model at this cost unless
    /// [`Self::ties_pruned`].
    pub tied_models: u64,
    /// A strict bound at this cost pruned its other ties, as the default
    /// [`crate::OptimumSearch`] does for a request of one answer. Then
    /// [`Self::tied_models`] counts only the models at this cost that the
    /// search met, at least one, and exhaustion proves this cost optimal
    /// without establishing how many models share it.
    pub ties_pruned: bool,
    /// Number of stable models whose objective was completely evaluated.
    pub scored_models: u64,
    /// Cumulative charged objective preparation and evaluation work.
    pub work: u64,
}

/// A bounded incumbent store could not retain the requested models completely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptimizationStop {
    /// Retaining another tied incumbent would exceed the configured model limit.
    Models,
    /// Retained full-model atoms would exceed the configured ceiling.
    Atoms,
    /// Retained catalog, selection and best-score payload would exceed the ceiling.
    Bytes,
    /// Count or size arithmetic overflowed.
    Overflow,
    /// Fallible reservation of a retained model slot or catalog-index entry failed.
    Allocation,
}
impl fmt::Display for OptimizationStop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "incumbent retention stopped: {self:?}")
    }
}
impl std::error::Error for OptimizationStop {}

impl From<RetentionError> for OptimizationStop {
    fn from(error: RetentionError) -> Self {
        match error {
            RetentionError::Bytes { .. } => Self::Bytes,
            RetentionError::Overflow => Self::Overflow,
            RetentionError::Allocation => Self::Allocation,
        }
    }
}

/// Borrowed cost value, distinct from the stored Score's structural equality.
/// Missing priorities denote zero and objective presence does not affect rank.
/// Comparison borrows the original slots; it never normalizes or copies them.
#[derive(Clone, Copy, Debug)]
struct ScoreValue<'a>(&'a Score);

impl PartialEq for ScoreValue<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for ScoreValue<'_> {}

impl PartialOrd for ScoreValue<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScoreValue<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.compare_costs(other.0)
    }
}

#[derive(Default)]
pub(crate) struct Incumbents {
    best: Option<Optimization>,
    models: Vec<Model>,
    atoms: usize,
    retention: ModelRetention,
    work: u64,
    scored: u64,
}
impl Incumbents {
    /// Charge the one preparation attempt before any diagnostic or score read.
    pub(crate) fn prepare(&mut self, work: u64) -> Result<(), Interruption> {
        self.charge(work)
    }

    fn charge(&mut self, work: u64) -> Result<(), Interruption> {
        self.work = self
            .work
            .checked_add(work)
            .ok_or(Interruption::Incumbent(OptimizationStop::Overflow))?;
        if let Some(best) = &mut self.best {
            best.work = self.work;
        }
        Ok(())
    }

    /// Score without retaining the model. Prepared reads and detailed fallback
    /// share one cumulative account, including every refused prefix.
    pub(crate) fn evaluate(
        &mut self,
        program: &ObjectiveProgram,
        model: &Model,
        options: &SolveConfig,
        cancellation: &Cancellation,
        prepared: Option<(&ObjectivePlan, &Interpretation)>,
    ) -> Result<Score, Interruption> {
        let limits = zetesis_objective::Limits {
            max_work: options.max_objective_work.saturating_sub(self.work),
            max_bindings: options.max_objective_bindings,
            max_keys: options.max_objective_keys,
            max_key_bytes: options.max_objective_key_bytes,
        };
        let score = if let Some((plan, interpretation)) = prepared {
            match plan.score(interpretation, limits, cancellation) {
                Ok(Some(score)) => {
                    self.charge(score.work())?;
                    Some(score.into_score())
                }
                Ok(None) => None,
                Err(error) => {
                    self.charge(error.work())?;
                    return Err(Interruption::PreparedObjective(error));
                }
            }
        } else {
            None
        };
        let score = if let Some(score) = score {
            score
        } else {
            match zetesis_objective::evaluate(program, model, limits, cancellation) {
                Ok(evaluation) => {
                    self.charge(evaluation.statistics().work)?;
                    evaluation.into_score()
                }
                Err(error) => {
                    self.charge(error.statistics().work)?;
                    return Err(Interruption::Objective(error));
                }
            }
        };
        self.scored = CheckedSum
            .combine(Some(self.scored), Some(1))
            .ok_or(Interruption::Incumbent(OptimizationStop::Overflow))?;
        if let Some(best) = &mut self.best {
            best.scored_models = self.scored;
        }
        Ok(score)
    }

    pub(crate) fn consider(
        &mut self,
        program: &ObjectiveProgram,
        model: Model,
        options: &SolveConfig,
        cancellation: &Cancellation,
        prepared: Option<(&ObjectivePlan, &Interpretation)>,
    ) -> Result<bool, Interruption> {
        let score = self.evaluate(program, &model, options, cancellation, prepared)?;
        let minimum = Minimum::new().step(
            self.best.as_ref().map(|best| ScoreValue(&best.score)),
            ScoreValue(&score),
        );
        if minimum.incoming == Ordering::Less {
            // The score is retained once for this entire tied family. Admission
            // of its full replacement precedes dropping any previous incumbent.
            let score_bytes = score_payload_bytes(Some(minimum.value.0))
                .map_err(OptimizationStop::from)
                .map_err(Interruption::Incumbent)?;
            self.retain(model, score_bytes, options, true)
                .map_err(Interruption::Incumbent)?;
            self.best = Some(Optimization {
                score,
                tied_models: 1,
                ties_pruned: false,
                scored_models: self.scored,
                work: self.work,
            });
            return Ok(true);
        }
        if minimum.incoming == Ordering::Equal {
            let best = self
                .best
                .as_mut()
                .expect("first score establishes an incumbent");
            best.tied_models = CheckedSum
                .combine(Some(best.tied_models), Some(1))
                .ok_or(Interruption::Incumbent(OptimizationStop::Overflow))?;
            if options.models == 0 || self.models.len() < options.models {
                self.retain(model, 0, options, false)
                    .map_err(Interruption::Incumbent)?;
            }
        }
        Ok(false)
    }

    pub(crate) fn score(&self) -> Option<&Score> {
        self.best.as_ref().map(|best| &best.score)
    }

    fn retain(
        &mut self,
        model: Model,
        associated_bytes: usize,
        options: &SolveConfig,
        replacement: bool,
    ) -> Result<(), OptimizationStop> {
        let (models, atoms) = if replacement {
            (0, 0)
        } else {
            (self.models.len(), self.atoms)
        };
        if models >= options.max_optimal_models {
            return Err(OptimizationStop::Models);
        }
        let atoms = atoms
            .checked_add(model.atoms().len())
            .ok_or(OptimizationStop::Overflow)?;
        if atoms > options.max_optimal_atoms {
            return Err(OptimizationStop::Atoms);
        }
        let admission = if replacement {
            self.retention
                .replace(&model, associated_bytes, options.max_optimal_bytes)?
        } else {
            self.retention
                .admit(&model, associated_bytes, options.max_optimal_bytes)?
        };
        if !replacement || self.models.capacity() == 0 {
            self.models
                .try_reserve(1)
                .map_err(|_| OptimizationStop::Allocation)?;
        }
        // No fallible step follows either publication. Replacement needs one
        // available slot, not additional capacity beside every previous tie.
        admission.commit();
        if replacement {
            self.models.clear();
        }
        self.models.push(model);
        self.atoms = atoms;
        Ok(())
    }

    /// Begin a tie pass at the proved optimum, and return how many retained
    /// models it will meet again, which it must not retain twice. The
    /// optimum's count restarts at them: the pass meets every optimal model,
    /// those counted earlier without being retained included, and counts each
    /// of the others once.
    pub(crate) fn begin_tie_pass(&mut self) -> usize {
        let retained = self.models.len();
        if let Some(best) = self.best.as_mut() {
            best.tied_models = u64::try_from(retained).unwrap_or(u64::MAX);
        }
        retained
    }

    /// Whether `model` is among the first `count` retained models.
    pub(crate) fn holds_among(&self, count: usize, model: &Model) -> bool {
        self.models[..count.min(self.models.len())].contains(model)
    }

    pub(crate) fn metadata(&self) -> Option<&Optimization> {
        self.best.as_ref()
    }

    pub(crate) const fn work(&self) -> u64 {
        self.work
    }

    pub(crate) const fn scored(&self) -> u64 {
        self.scored
    }

    pub(crate) fn retained(&self) -> usize {
        self.models.len()
    }

    pub(crate) fn take_models(&mut self) -> std::vec::IntoIter<Model> {
        // Final outcome metadata is captured before this transfer. The consumer
        // owns the yielded queue thereafter; no new retention is attempted.
        self.retention.clear();
        self.atoms = 0;
        std::mem::take(&mut self.models).into_iter()
    }
}

pub(crate) fn score_payload_bytes(score: Option<&Score>) -> Result<usize, RetentionError> {
    // One option tag; a present score adds u64 length and i32/i64 pairs.
    // Incumbents stores this once; WorldView stores one record per answer.
    const SCORE_LEVEL_BYTES: usize = size_of::<i32>() + size_of::<i64>();
    const SCORE_HEADER_BYTES: usize = 1 + size_of::<u64>();
    match score {
        None => Ok(1),
        Some(score) => score
            .costs()
            .len()
            .checked_mul(SCORE_LEVEL_BYTES)
            .and_then(|bytes| bytes.checked_add(SCORE_HEADER_BYTES))
            .ok_or(RetentionError::Overflow),
    }
}

#[cfg(test)]
mod tests;
