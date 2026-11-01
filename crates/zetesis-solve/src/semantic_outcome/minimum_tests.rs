//! Cost completion is distinct from retained incumbents and complete tie families.

use super::{AnswerSelection, Interruption, Optimization, SearchState, SemanticOutcome, Verdict};

fn outcome() -> SemanticOutcome {
    let mut outcome = SemanticOutcome::interrupted_before_start(Interruption::Preparation(
        zetesis_algebra::Stop::Cancelled,
    ));
    outcome.selection = Some(AnswerSelection::Optimal);
    outcome.verified = 1;
    outcome.optimization = Some(Optimization {
        score: zetesis_objective::Score::from_costs(vec![(0, 7)]).unwrap(),
        tied_models: 1,
        ties_pruned: false,
        scored_models: 1,
        work: 0,
    });
    outcome
}

const fn optimum_from_const_context(outcome: &SemanticOutcome) -> bool {
    outcome.optimum_proved()
}

#[test]
fn completed_minimum_borrows_the_retained_score() {
    let mut outcome = outcome();
    outcome.search_state = Some(SearchState::Exhausted);
    let Verdict::Complete(score) = outcome.minimum_verdict() else {
        panic!("exhausted optimization has its retained minimum");
    };
    assert!(std::ptr::eq(
        score,
        &raw const outcome.incumbent().unwrap().score
    ));
    assert!(optimum_from_const_context(&outcome));
    assert_eq!(outcome.complete_answer_count(), Some(1));
}

#[test]
fn empty_complete_search_has_no_minimum_value() {
    let mut outcome = outcome();
    outcome.search_state = Some(SearchState::Exhausted);
    outcome.optimization = None;
    outcome.verified = 0;
    assert!(matches!(outcome.minimum_verdict(), Verdict::Empty));
    assert!(!optimum_from_const_context(&outcome));
    assert_eq!(outcome.complete_answer_count(), Some(0));
    outcome.verified = 1;
    assert!(matches!(outcome.minimum_verdict(), Verdict::Undecided(())));
    assert!(!outcome.optimum_proved());
}

#[test]
fn stopped_tie_pass_keeps_the_cost_complete_without_the_family() {
    let reason = Interruption::Preparation(zetesis_algebra::Stop::Cancelled);
    for state in [
        SearchState::Interrupted(reason),
        SearchState::PendingInterruption(reason),
    ] {
        let mut outcome = outcome();
        outcome.search_state = Some(state);
        assert!(matches!(outcome.minimum_verdict(), Verdict::Undecided(())));
        assert!(!outcome.optimum_proved());
        outcome.optimum_established = true;
        assert!(matches!(outcome.minimum_verdict(), Verdict::Complete(_)));
        assert!(outcome.optimum_proved());
        assert_eq!(outcome.complete_answer_count(), None);
        assert_eq!(outcome.interruption(), Some(reason));
    }
}

#[test]
fn missing_selection_or_completion_never_promotes_an_incumbent() {
    let mut outcome = outcome();
    outcome.optimum_established = true;
    for state in [None, Some(SearchState::RequestedModels)] {
        outcome.search_state = state;
        assert!(matches!(outcome.minimum_verdict(), Verdict::Undecided(())));
        assert!(!outcome.optimum_proved());
    }
    outcome.search_state = Some(SearchState::Exhausted);
    for selection in [None, Some(AnswerSelection::All)] {
        outcome.selection = selection;
        assert!(matches!(outcome.minimum_verdict(), Verdict::Undecided(())));
        assert!(!outcome.optimum_proved());
    }
}

#[test]
fn established_flag_without_an_incumbent_cannot_supply_a_minimum() {
    let mut outcome = outcome();
    outcome.optimum_established = true;
    outcome.optimization = None;
    assert!(matches!(outcome.minimum_verdict(), Verdict::Undecided(())));
    assert!(!outcome.optimum_proved());
    assert_eq!(outcome.complete_answer_count(), None);
}
