//! Search completion and logical interruption.

use std::fmt;
use zetesis_cpu::Stop;

/// Why a successful driver invocation stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    /// Every relevant candidate was checked. With an objective, candidates
    /// worse than a verified incumbent may be omitted; all optimal ties remain
    /// covered. This is search coverage, independently of output delivery.
    /// UNSAT additionally requires no verified stable model.
    Exhausted,
    /// The requested number of models was returned; coverage remains partial.
    RequestedModels,
    /// Search or one oracle stopped with explicit incomplete coverage.
    Interrupted,
}

/// Established search status, independently of external publication.
/// An absent status means that no stopping classification is established.
/// Each interruption carries its cause; exhausted and requested-count states
/// cannot carry a contradictory interruption.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchState {
    /// Every relevant candidate was checked; see [`Completion::Exhausted`].
    Exhausted,
    /// The requested answer count was reached with partial coverage.
    RequestedModels,
    /// Enumeration reached a known stop and finalized its checked prefix.
    Interrupted(Interruption),
    /// A batch knows its eventual stop but still has checked answers to yield.
    /// Draining that prefix establishes [`Self::Interrupted`]; observing this
    /// state alone establishes neither completed search nor completed delivery.
    PendingInterruption(Interruption),
}

impl SearchState {
    /// Compatibility coverage classification. A pending stop is not finalized.
    #[must_use]
    pub const fn completion(self) -> Option<Completion> {
        match self {
            Self::Exhausted => Some(Completion::Exhausted),
            Self::RequestedModels => Some(Completion::RequestedModels),
            Self::Interrupted(_) => Some(Completion::Interrupted),
            Self::PendingInterruption(_) => None,
        }
    }

    /// Known stop, including one pending behind already checked answers.
    #[must_use]
    pub const fn interruption(self) -> Option<Interruption> {
        match self {
            Self::Interrupted(reason) | Self::PendingInterruption(reason) => Some(reason),
            Self::Exhausted | Self::RequestedModels => None,
        }
    }
}

/// A typed reason why model enumeration could not establish complete coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interruption {
    /// Incremental candidate enumeration or a closure oracle stopped.
    Oracle(Stop),
    /// Reduct countermodel encoding, search or independent witness checking stopped.
    Countermodel(zetesis_sat::Incomplete),
    /// An objective could not be completely evaluated for a verified model.
    Objective(zetesis_objective::Error),
    /// Retaining complete incumbent models exceeded an explicit bound.
    Incumbent(crate::OptimizationStop),
}
impl fmt::Display for Interruption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oracle(error) => error.fmt(f),
            Self::Countermodel(error) => error.fmt(f),
            Self::Objective(error) => error.fmt(f),
            Self::Incumbent(error) => error.fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Completion, Interruption, SearchState};
    use zetesis_cpu::Stop;

    #[test]
    fn compatibility_views_derive_from_one_search_state() {
        let reason = Interruption::Oracle(Stop::CandidateLimit);
        for (state, completion, interruption) in [
            (SearchState::Exhausted, Some(Completion::Exhausted), None),
            (
                SearchState::RequestedModels,
                Some(Completion::RequestedModels),
                None,
            ),
            (
                SearchState::Interrupted(reason),
                Some(Completion::Interrupted),
                Some(reason),
            ),
            (SearchState::PendingInterruption(reason), None, Some(reason)),
        ] {
            assert_eq!(state.completion(), completion);
            assert_eq!(state.interruption(), interruption);
        }
    }
}
