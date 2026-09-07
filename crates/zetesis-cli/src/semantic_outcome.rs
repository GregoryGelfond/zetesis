//! Established semantic evidence, independent of external publication.

use crate::{Completion, Interruption, Optimization};

/// Immutable semantic accounting when a session stops or finishes its search.
/// A verified model need not have been scored, retained, yielded or published.
/// Neither missing coverage nor a zero publication count establishes UNSAT.
#[derive(Clone, Debug)]
pub struct SemanticOutcome {
    pub(crate) subject: Option<crate::Subject>,
    pub(crate) verified: u64,
    pub(crate) scored: u64,
    pub(crate) retained: usize,
    pub(crate) completion: Option<Completion>,
    pub(crate) interruption: Option<Interruption>,
    pub(crate) optimization: Option<Optimization>,
    pub(crate) checked: u64,
    pub(crate) gate_atoms: usize,
    pub(crate) countermodel_statistics: Option<zetesis_sat::Statistics>,
    pub(crate) formula_execution: Option<crate::FormulaExecutionStatistics>,
}

impl SemanticOutcome {
    /// Original immutable semantic subject, before any candidate restrictions.
    /// Absent only for a control stop before source admission established an input.
    #[must_use]
    pub const fn subject(&self) -> Option<&crate::Subject> {
        self.subject.as_ref()
    }

    /// Closure results consumed, or formula candidates proposed.
    #[must_use]
    pub const fn candidate_progress(&self) -> u64 {
        self.checked
    }

    /// Gate tuples retained by closure enumeration; formula routes return zero.
    #[must_use]
    pub const fn discovered_gate_atoms(&self) -> usize {
        self.gate_atoms
    }

    /// Formula candidate/reduct accounting, when that stream was initialized.
    #[must_use]
    pub const fn countermodel_statistics(&self) -> Option<&zetesis_sat::Statistics> {
        self.countermodel_statistics.as_ref()
    }

    /// Bounded formula batch accounting, including queued verified models.
    #[must_use]
    pub const fn formula_execution(&self) -> Option<&crate::FormulaExecutionStatistics> {
        self.formula_execution.as_ref()
    }
    /// Completed exact stable-model memberships, including still-queued results.
    #[must_use]
    pub const fn verified_models(&self) -> u64 {
        self.verified
    }

    /// Verified models whose objective evaluation completed.
    #[must_use]
    pub const fn scored_models(&self) -> u64 {
        self.scored
    }

    /// Incumbent models retained when search stopped, before any delivery.
    /// Ordinary enumeration without objectives does not retain incumbents.
    #[must_use]
    pub const fn retained_models(&self) -> usize {
        self.retained
    }

    /// Established search coverage; absent after an unrelated failure or when the
    /// consumer stops pulling before the engine establishes a stopping classification.
    #[must_use]
    pub const fn completion(&self) -> Option<Completion> {
        self.completion
    }

    /// Established logical stop, independent of a later delivery failure.
    #[must_use]
    pub const fn interruption(&self) -> Option<Interruption> {
        self.interruption
    }

    /// Retained objective evidence; a score alone does not establish an optimum.
    #[must_use]
    pub const fn incumbent(&self) -> Option<&Optimization> {
        self.optimization.as_ref()
    }

    /// Complete relevant search established the retained incumbent's optimum.
    #[must_use]
    pub const fn optimum_proved(&self) -> bool {
        matches!(self.completion, Some(Completion::Exhausted)) && self.optimization.is_some()
    }

    /// Complete search found no stable model, independently of output delivery.
    #[must_use]
    pub const fn unsatisfiable(&self) -> bool {
        matches!(self.completion, Some(Completion::Exhausted)) && self.verified == 0
    }
}
