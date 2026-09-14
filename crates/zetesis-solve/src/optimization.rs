//! Bounded incumbent retention; optimality requires complete relevant search.

use std::cmp::Ordering;
use std::fmt;

use zetesis_core::Model;
use zetesis_core::retention::{ModelRetention, RetentionError};
use zetesis_cpu::Control;
use zetesis_objective::{ObjectiveProgram, Score};

use crate::{Interruption, SolveConfig};

/// A completely evaluated incumbent, not by itself an optimality certificate.
#[derive(Clone, Debug)]
pub struct Optimization {
    /// Best retained cost, at descending priorities. A storage interruption may
    /// preserve an earlier model even after a better score was evaluated.
    pub score: Score,
    /// Number of full stable models found with this cost, including hidden ties.
    pub tied_models: u64,
    /// Number of stable models whose objective was completely evaluated.
    pub scored_models: u64,
    /// Cumulative charged evaluation work.
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
    /// Evaluate without retaining or selecting the model. Both unrestricted
    /// enumeration and optimization spend this same cumulative score budget.
    pub(crate) fn evaluate(
        &mut self,
        program: &ObjectiveProgram,
        model: &Model,
        options: &SolveConfig,
        control: &Control,
    ) -> Result<Score, Interruption> {
        let evaluation = zetesis_objective::evaluate(
            program,
            model,
            zetesis_objective::Limits {
                max_work: options.max_objective_work.saturating_sub(self.work),
                max_bindings: options.max_objective_bindings,
                max_keys: options.max_objective_keys,
                max_key_bytes: options.max_objective_key_bytes,
            },
            control,
        )
        .map_err(|error| {
            // Each call receives only the remaining cumulative work allowance.
            self.work = self
                .work
                .checked_add(error.statistics().work)
                .expect("evaluation work fits the remaining allowance");
            if let Some(best) = &mut self.best {
                best.work = self.work;
            }
            Interruption::Objective(error)
        })?;
        self.work = self
            .work
            .checked_add(evaluation.statistics().work)
            .ok_or(Interruption::Incumbent(OptimizationStop::Overflow))?;
        self.scored = self
            .scored
            .checked_add(1)
            .ok_or(Interruption::Incumbent(OptimizationStop::Overflow))?;
        if let Some(best) = &mut self.best {
            best.scored_models = self.scored;
            best.work = self.work;
        }
        Ok(evaluation.into_score())
    }

    pub(crate) fn consider(
        &mut self,
        program: &ObjectiveProgram,
        model: Model,
        options: &SolveConfig,
        control: &Control,
    ) -> Result<bool, Interruption> {
        let score = self.evaluate(program, &model, options, control)?;
        let order = self
            .best
            .as_ref()
            .map_or(Ordering::Less, |best| score.compare_costs(&best.score));
        if order == Ordering::Less {
            // The score is retained once for this entire tied family. Admission
            // of its full replacement precedes dropping any previous incumbent.
            let score_bytes = score_payload_bytes(Some(&score))
                .map_err(OptimizationStop::from)
                .map_err(Interruption::Incumbent)?;
            self.retain(model, score_bytes, options, true)
                .map_err(Interruption::Incumbent)?;
            self.best = Some(Optimization {
                score,
                tied_models: 1,
                scored_models: self.scored,
                work: self.work,
            });
            return Ok(true);
        }
        if order == Ordering::Equal {
            let best = self
                .best
                .as_mut()
                .expect("first score establishes an incumbent");
            best.tied_models = best
                .tied_models
                .checked_add(1)
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

    pub(crate) fn metadata(&self) -> Option<&Optimization> {
        self.best.as_ref()
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
