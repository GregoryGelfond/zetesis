//! Established semantic evidence, independent of external publication.

use crate::{Completion, Interruption, Optimization, SearchState};

/// The answer family requested by a semantic session, independently of whether
/// it was completely searched or delivered. Optional projected enumeration
/// selects representatives only after establishing this original family choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnswerSelection {
    /// All full answer sets of the original program, regardless of objectives.
    All,
    /// Objective-selected answers. Interrupted incumbents need not be optimal;
    /// even a proved optimum need not have every tie retained or yielded.
    Optimal,
}

/// Immutable semantic accounting when a session stops or finishes its search.
/// The driver retains this evidence as its semantic authority; compatibility
/// reports derive from it together with separate publication and timing state.
/// A verified model need not have been scored, retained, yielded or published.
/// Neither missing coverage nor a zero publication count establishes UNSAT.
#[derive(Clone, Debug)]
pub struct SemanticOutcome {
    pub(crate) projection: Option<crate::ProjectionStatistics>,
    pub(crate) subject: Option<crate::Subject>,
    pub(crate) selection: Option<AnswerSelection>,
    pub(crate) verified: u64,
    pub(crate) scored: u64,
    pub(crate) retained: usize,
    pub(crate) search_state: Option<SearchState>,
    pub(crate) optimization: Option<Optimization>,
    pub(crate) checked: u64,
    pub(crate) gate_atoms: usize,
    pub(crate) candidate_statistics: Option<zetesis_cpu::CandidateStatistics>,
    pub(crate) countermodel_statistics: Option<zetesis_sat::Statistics>,
    pub(crate) formula_execution: Option<crate::FormulaExecutionStatistics>,
    pub(crate) lazy_execution: Option<crate::LazyExecutionStatistics>,
    pub(crate) shared_execution: Option<crate::SharedExecutionStatistics>,
    pub(crate) closure_execution: Option<crate::ClosureExecutionStatistics>,
    pub(crate) query_execution: Option<crate::QueryExecutionObservation>,
    pub(crate) hybrid_execution: Option<crate::HybridExecutionStatistics>,
    pub(crate) terminal_execution: Option<crate::TerminalExecutionStatistics>,
    pub(crate) model_construction: Option<crate::ModelConstructionStatistics>,
}

impl SemanticOutcome {
    /// Record an interruption before admission established an input or selection.
    ///
    /// No answer has been checked and no search coverage is established. This
    /// constructor cannot create an exhausted outcome or prove inconsistency.
    #[must_use]
    pub const fn interrupted_before_start(interruption: Interruption) -> Self {
        Self {
            projection: None,
            subject: None,
            selection: None,
            verified: 0,
            scored: 0,
            retained: 0,
            search_state: Some(SearchState::Interrupted(interruption)),
            optimization: None,
            checked: 0,
            gate_atoms: 0,
            candidate_statistics: None,
            countermodel_statistics: None,
            formula_execution: None,
            lazy_execution: None,
            shared_execution: None,
            closure_execution: None,
            query_execution: None,
            hybrid_execution: None,
            terminal_execution: None,
            model_construction: None,
        }
    }

    /// Requested answer family. Absent only when a driver stopped before a
    /// session established its selection. This is not a completeness claim.
    #[must_use]
    pub const fn selection(&self) -> Option<AnswerSelection> {
        self.selection
    }

    /// Distinct projected representatives when projection was requested.
    /// Absence denotes full answer identity. Full-model search coverage and
    /// optimum evidence remain independent of projected-stream completion.
    #[must_use]
    pub const fn projection(&self) -> Option<&crate::ProjectionStatistics> {
        self.projection.as_ref()
    }

    /// Original immutable semantic subject, before any candidate restrictions.
    /// Absent only for a control stop before source admission established an input.
    #[must_use]
    pub const fn subject(&self) -> Option<&crate::Subject> {
        self.subject.as_ref()
    }

    /// Closure result/control records consumed, or formula candidates proposed.
    /// One shared-batch stop record can represent several interrupted occurrences.
    #[must_use]
    pub const fn candidate_progress(&self) -> u64 {
        self.checked
    }

    /// Gate tuples retained by closure enumeration; formula routes return zero.
    #[must_use]
    pub const fn discovered_gate_atoms(&self) -> usize {
        self.gate_atoms
    }

    /// Source candidate restriction accounting, including incomplete preparation.
    /// Absent for formula input or an interruption before closure construction.
    #[must_use]
    pub const fn candidate_statistics(&self) -> Option<zetesis_cpu::CandidateStatistics> {
        self.candidate_statistics
    }

    /// Formula candidate/reduct accounting, when that stream was initialized.
    /// For hybrid or terminal-definition input this describes the retained base.
    /// Its stable-model count does not include subsequent source-constraint
    /// checking or full-answer reconstruction; use
    /// [`Self::verified_models`] for original-program membership.
    #[must_use]
    pub const fn countermodel_statistics(&self) -> Option<&zetesis_sat::Statistics> {
        self.countermodel_statistics.as_ref()
    }

    /// Core answers filtered by complete source-constraint checks. Absent for
    /// eager and relational execution. Pending checks establish no membership.
    #[must_use]
    pub const fn hybrid_execution(&self) -> Option<&crate::HybridExecutionStatistics> {
        self.hybrid_execution.as_ref()
    }

    /// Base answers consumed by terminal-definition reconstruction, including
    /// interrupted attempts. Only completed reconstruction establishes a full
    /// answer of the original subject; buffered base answers do not.
    #[must_use]
    pub const fn terminal_execution(&self) -> Option<&crate::TerminalExecutionStatistics> {
        self.terminal_execution.as_ref()
    }

    /// Prepared semantic order and model construction over the formula catalog.
    /// Hybrid and terminal sessions retain the base receipt here. Reconstruction
    /// and source qualification are separate; constructed models are not a count
    /// of published original answers. Absent before a construction attempt or
    /// for closure execution.
    #[must_use]
    pub const fn model_construction(&self) -> Option<&crate::ModelConstructionStatistics> {
        self.model_construction.as_ref()
    }

    /// Bounded formula batch accounting, including effective device limits,
    /// actual submissions, decoded results and queued verified models.
    #[must_use]
    pub const fn formula_execution(&self) -> Option<&crate::FormulaExecutionStatistics> {
        self.formula_execution.as_ref()
    }
    /// Lazy device accounting, including completed but unconsumed results.
    #[must_use]
    pub const fn lazy_execution(&self) -> Option<&crate::LazyExecutionStatistics> {
        self.lazy_execution.as_ref()
    }
    /// Shared CPU accounting, including complete but unconsumed checks.
    #[must_use]
    pub const fn shared_execution(&self) -> Option<&crate::SharedExecutionStatistics> {
        self.shared_execution.as_ref()
    }
    /// Independent CPU closure counters summed over completed checks, lazy or
    /// eager. Absent for the shared, device and formula routes.
    #[must_use]
    pub const fn closure_execution(&self) -> Option<&crate::ClosureExecutionStatistics> {
        self.closure_execution.as_ref()
    }
    /// Prepared independent CPU ownership receipts, including a snapshot fault
    /// retained independently of already checked models. Absent for other routes.
    #[must_use]
    pub const fn query_execution(&self) -> Option<&crate::QueryExecutionObservation> {
        self.query_execution.as_ref()
    }
    /// Completed exact stable-model memberships of the original subject,
    /// including still-queued results. A terminal-definition input counts only
    /// completed full reconstruction, never queued or consumed base answers.
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
    /// Unrestricted enumeration does not retain incumbents, even with objectives.
    #[must_use]
    pub const fn retained_models(&self) -> usize {
        self.retained
    }

    /// Established search status, including a stop pending behind checked answers.
    /// Absent when no stopping classification has been established.
    #[must_use]
    pub const fn search_state(&self) -> Option<SearchState> {
        self.search_state
    }

    /// Established search coverage; absent after an unrelated failure or when the
    /// consumer stops pulling before the engine establishes a stopping classification.
    #[must_use]
    pub const fn completion(&self) -> Option<Completion> {
        match self.search_state {
            Some(state) => state.completion(),
            None => None,
        }
    }

    /// Established logical stop, independent of a later delivery failure.
    #[must_use]
    pub const fn interruption(&self) -> Option<Interruption> {
        match self.search_state {
            Some(state) => state.interruption(),
            None => None,
        }
    }

    /// Retained objective evidence; a score alone does not establish an optimum.
    #[must_use]
    pub const fn incumbent(&self) -> Option<&Optimization> {
        self.optimization.as_ref()
    }

    /// Complete relevant search established the retained incumbent's optimum.
    #[must_use]
    pub const fn optimum_proved(&self) -> bool {
        matches!(self.selection, Some(AnswerSelection::Optimal))
            && matches!(self.search_state, Some(SearchState::Exhausted))
            && self.optimization.is_some()
    }

    /// Complete search found no stable model, independently of output delivery.
    #[must_use]
    pub const fn unsatisfiable(&self) -> bool {
        matches!(self.search_state, Some(SearchState::Exhausted)) && self.verified == 0
    }
}
