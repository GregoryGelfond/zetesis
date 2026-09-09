//! Bounded incumbent retention; optimality requires complete relevant search.

use std::cmp::Ordering;
use std::fmt;

use zetesis_core::{Model, Value};
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
    /// Retained full-model payload bytes would exceed the configured ceiling.
    Bytes,
    /// Count or size arithmetic overflowed.
    Overflow,
    /// Fallible reservation of a retained model slot failed.
    Allocation,
}
impl fmt::Display for OptimizationStop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "incumbent retention stopped: {self:?}")
    }
}
impl std::error::Error for OptimizationStop {}

#[derive(Default)]
pub(crate) struct Incumbents {
    best: Option<Optimization>,
    models: Vec<Model>,
    atoms: usize,
    bytes: usize,
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
            // Admission of the replacement precedes dropping any previous incumbent.
            self.admit(&model, options, true)
                .map_err(Interruption::Incumbent)?;
            self.models.clear();
            self.atoms = 0;
            self.bytes = 0;
            self.best = Some(Optimization {
                score,
                tied_models: 0,
                scored_models: self.scored,
                work: self.work,
            });
        }
        if order != Ordering::Greater {
            let best = self
                .best
                .as_mut()
                .expect("first score establishes an incumbent");
            best.tied_models = best
                .tied_models
                .checked_add(1)
                .ok_or(Interruption::Incumbent(OptimizationStop::Overflow))?;
            if options.models == 0 || self.models.len() < options.models {
                self.admit(&model, options, false)
                    .map_err(Interruption::Incumbent)?;
                self.atoms += model.atoms().len();
                self.bytes += payload_bytes(&model).map_err(Interruption::Incumbent)?;
                self.models.push(model);
            }
        }
        Ok(order == Ordering::Less)
    }

    pub(crate) fn score(&self) -> Option<&Score> {
        self.best.as_ref().map(|best| &best.score)
    }

    fn admit(
        &mut self,
        model: &Model,
        options: &SolveConfig,
        replacement: bool,
    ) -> Result<(), OptimizationStop> {
        let (models, atoms, bytes) = if replacement {
            (0, 0, 0)
        } else {
            (self.models.len(), self.atoms, self.bytes)
        };
        if models >= options.max_optimal_models {
            return Err(OptimizationStop::Models);
        }
        if atoms
            .checked_add(model.atoms().len())
            .ok_or(OptimizationStop::Overflow)?
            > options.max_optimal_atoms
        {
            return Err(OptimizationStop::Atoms);
        }
        if bytes
            .checked_add(payload_bytes(model)?)
            .ok_or(OptimizationStop::Overflow)?
            > options.max_optimal_bytes
        {
            return Err(OptimizationStop::Bytes);
        }
        self.models
            .try_reserve(1)
            .map_err(|_| OptimizationStop::Allocation)
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
        std::mem::take(&mut self.models).into_iter()
    }
}

pub(crate) fn payload_bytes(model: &Model) -> Result<usize, OptimizationStop> {
    // Canonical payload accounting: u64 model/atom lengths, one predicate-sign
    // tag per atom, predicate UTF-8,
    // and tagged i32 or length-prefixed scalar values; not allocator overhead.
    let mut bytes = 8usize;
    for atom in model.atoms() {
        bytes = bytes
            .checked_add(17)
            .and_then(|sum| sum.checked_add(atom.predicate().name().len()))
            .ok_or(OptimizationStop::Overflow)?;
        for value in atom.values() {
            let payload = match value {
                Value::Infimum | Value::Supremum => 0,
                Value::Number(_) => 4,
                Value::Structured(value) => value.canonical_bytes() - 1,
                Value::Symbol(text) | Value::String(text) => 8usize
                    .checked_add(text.len())
                    .ok_or(OptimizationStop::Overflow)?,
            };
            bytes = bytes
                .checked_add(1)
                .and_then(|sum| sum.checked_add(payload))
                .ok_or(OptimizationStop::Overflow)?;
        }
    }
    Ok(bytes)
}
