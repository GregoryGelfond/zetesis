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

/// Settle a wrapper without replacing an already established interruption.
/// A cleanup failure still prevents exhaustion or a requested-count outcome.
pub(crate) fn after_cleanup(
    state: Option<SearchState>,
    cleanup: Result<(), zetesis_sat::Incomplete>,
) -> Option<SearchState> {
    if matches!(state, Some(SearchState::Interrupted(_))) {
        state
    } else {
        cleanup.map_or_else(
            |error| Some(SearchState::Interrupted(Interruption::Countermodel(error))),
            |()| state,
        )
    }
}

/// A typed reason why model enumeration could not establish complete coverage.
/// Implements [`std::error::Error`], preserving any underlying typed cause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interruption {
    /// Cancellation stopped preparation before an executor began checking candidates.
    Preparation(Stop),
    /// Incremental candidate enumeration or a closure oracle stopped.
    Oracle(Stop),
    /// Reduct countermodel encoding, search or independent witness checking stopped.
    Countermodel(zetesis_sat::Incomplete),
    /// A streamed source constraint stopped before original-program membership.
    Constraint(Stop),
    /// Full-answer reconstruction stopped after verified base membership.
    Reconstruction(Stop),
    /// Preparing semantic atom order or constructing a selected model stopped.
    ModelConstruction(crate::ModelConstructionStop),
    /// An objective could not be completely evaluated for a verified model.
    Objective(zetesis_objective::Error),
    /// A prepared objective eligibility read or numeric reduction stopped.
    PreparedObjective(zetesis_themelios::objective_bound::ObjectiveScoreError),
    /// Retaining complete incumbent models exceeded an explicit bound.
    Incumbent(crate::OptimizationStop),
}
impl fmt::Display for Interruption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preparation(error)
            | Self::Oracle(error)
            | Self::Constraint(error)
            | Self::Reconstruction(error) => error.fmt(f),
            Self::Countermodel(error) => error.fmt(f),
            Self::Objective(error) => error.fmt(f),
            Self::PreparedObjective(error) => error.fmt(f),
            Self::Incumbent(error) => error.fmt(f),
            Self::ModelConstruction(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Interruption {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Preparation(error)
            | Self::Oracle(error)
            | Self::Constraint(error)
            | Self::Reconstruction(error)
            | Self::ModelConstruction(crate::ModelConstructionStop::Control(error)) => Some(error),
            Self::Countermodel(error) => Some(error),
            Self::Objective(error) => Some(error),
            Self::PreparedObjective(error) => Some(error),
            Self::Incumbent(error) => Some(error),
            // These resource records are the cause itself, without a nested Error.
            Self::ModelConstruction(
                crate::ModelConstructionStop::Work { .. }
                | crate::ModelConstructionStop::Bytes { .. },
            ) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Completion, Interruption, SearchState};
    use std::error::Error;
    use zetesis_cpu::Stop;

    #[test]
    fn interruption_retains_its_public_type_when_boxed() {
        let reason = Interruption::Oracle(Stop::WorkLimit);
        let error: Box<dyn Error + Send + Sync> = Box::new(reason);
        assert_eq!(error.downcast_ref::<Interruption>(), Some(&reason));
    }

    #[test]
    fn direct_control_stages_expose_the_original_stop() {
        for reason in [
            Interruption::Preparation(Stop::Cancelled),
            Interruption::Oracle(Stop::Cancelled),
            Interruption::Constraint(Stop::Cancelled),
            Interruption::Reconstruction(Stop::Cancelled),
            Interruption::ModelConstruction(crate::ModelConstructionStop::Control(Stop::Cancelled)),
        ] {
            assert_eq!(
                reason
                    .source()
                    .and_then(|source| source.downcast_ref::<Stop>()),
                Some(&Stop::Cancelled),
            );
        }
    }

    #[test]
    fn countermodel_interruption_preserves_the_nested_error_chain() {
        let native = zetesis_sat::Incomplete::Verification(Stop::Deadline);
        let reason = Interruption::Countermodel(native);
        let source = reason.source().expect("the native countermodel cause");
        assert_eq!(
            source.downcast_ref::<zetesis_sat::Incomplete>(),
            Some(&native)
        );
        assert_eq!(
            source
                .source()
                .and_then(|source| source.downcast_ref::<Stop>()),
            Some(&Stop::Deadline),
        );
    }

    #[test]
    fn incumbent_interruption_exposes_the_retention_refusal() {
        let reason = Interruption::Incumbent(crate::OptimizationStop::Bytes);
        assert_eq!(
            reason
                .source()
                .and_then(|source| source.downcast_ref::<crate::OptimizationStop>()),
            Some(&crate::OptimizationStop::Bytes),
        );
    }

    #[test]
    fn model_construction_limits_have_no_fabricated_source() {
        for reason in [
            Interruption::ModelConstruction(crate::ModelConstructionStop::Work {
                observed: 8,
                limit: 7,
            }),
            Interruption::ModelConstruction(crate::ModelConstructionStop::Bytes {
                required: 8,
                limit: 7,
            }),
        ] {
            assert!(reason.source().is_none());
        }
    }

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

    #[test]
    fn cleanup_preserves_an_established_stop_but_invalidates_completion() {
        let primary = SearchState::Interrupted(Interruption::ModelConstruction(
            crate::ModelConstructionStop::Work {
                observed: 8,
                limit: 7,
            },
        ));
        let cleanup = Err(zetesis_sat::Incomplete::RegionFilter);
        assert_eq!(super::after_cleanup(Some(primary), cleanup), Some(primary));
        let secondary = Some(SearchState::Interrupted(Interruption::Countermodel(
            zetesis_sat::Incomplete::RegionFilter,
        )));
        assert_eq!(
            super::after_cleanup(Some(SearchState::RequestedModels), cleanup),
            secondary
        );
        assert_eq!(
            super::after_cleanup(Some(SearchState::Exhausted), cleanup),
            secondary
        );
        assert_eq!(super::after_cleanup(None, cleanup), secondary);
        assert_eq!(
            super::after_cleanup(Some(SearchState::Exhausted), Ok(())),
            Some(SearchState::Exhausted)
        );
    }
}
