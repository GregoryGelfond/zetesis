//! A failed collection retains a typed cause chain and its checked owners.

use super::*;
use std::error::Error as _;

fn collection_refusal(limits: WorldViewLimits) -> WorldViewFailure {
    let owner = normal("a. b.");
    WorldView::collect(
        PreparedInput::admitted(&owner),
        config(),
        limits,
        Control::default(),
    )
    .unwrap_err()
}

fn collection_chain(failure: &WorldViewFailure, message: &str) {
    assert_eq!(failure.to_string(), message);
    let linked = failure
        .source()
        .unwrap()
        .downcast_ref::<WorldViewError>()
        .unwrap();
    assert!(std::ptr::eq(linked, failure.cause()));
    assert_eq!(linked.to_string(), message);
    assert!(linked.source().is_none());
    assert!(failure.answer_sets().is_empty());
    let outcome = failure.outcome().unwrap();
    assert_eq!(outcome.verified_models(), 1);
    assert!(!outcome.unsatisfiable());
    assert!(outcome.subject().unwrap().same_instance(failure.subject()));
}

#[test]
fn count_refusal_remains_a_collection_cause() {
    let failure = collection_refusal(WorldViewLimits {
        max_answer_sets: 0,
        ..Default::default()
    });
    assert!(matches!(failure.cause(), WorldViewError::AnswerSets));
    collection_chain(&failure, "world-view answer-set limit reached");
}

#[test]
fn atom_refusal_remains_a_collection_cause() {
    let failure = collection_refusal(WorldViewLimits {
        max_atoms: 1,
        ..Default::default()
    });
    assert!(matches!(failure.cause(), WorldViewError::Atoms));
    collection_chain(&failure, "world-view atom limit reached");
}

#[test]
fn byte_refusal_remains_a_collection_cause() {
    let failure = collection_refusal(WorldViewLimits {
        max_bytes: 0,
        ..Default::default()
    });
    assert!(matches!(failure.cause(), WorldViewError::Bytes));
    collection_chain(&failure, "world-view payload-byte limit reached");
}

#[test]
fn search_refusal_transfers_the_original_checked_prefix() {
    let owner = formula("a. {b}.");
    let failure = WorldView::collect(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_candidates: 1,
            ..config()
        },
        WorldViewLimits::default(),
        Control::default(),
    )
    .unwrap_err();
    assert!(matches!(failure.cause(), WorldViewError::NotExhausted));
    assert_eq!(
        failure.to_string(),
        "original answer-set search was not exhausted"
    );
    assert!(failure.source().unwrap().source().is_none());
    let outcome = failure.outcome().unwrap();
    assert_eq!(outcome.verified_models(), 1);
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(failure.answer_sets().len(), 1);
    let subject = failure.subject().clone();
    let original_answer = std::ptr::from_ref(&failure.answer_sets()[0]);
    let original_atom = std::ptr::from_ref(
        failure.answer_sets()[0]
            .interpretation()
            .atoms()
            .first()
            .unwrap(),
    );
    let expected = names(&failure.answer_sets()[0]);
    assert!(expected.contains(&"a".into()));
    let answers = failure.into_answer_sets();
    drop(owner);
    assert_eq!(answers.len(), 1);
    assert!(std::ptr::eq(&answers[0], original_answer));
    assert!(std::ptr::eq(
        answers[0].interpretation().atoms().first().unwrap(),
        original_atom
    ));
    assert_eq!(names(&answers[0]), expected);
    assert!(answers[0].subject().same_instance(&subject));
}

#[test]
fn observer_refusal_chain_retains_the_external_error() {
    for at in [Refusal::Execution, Refusal::Formula] {
        let (failure, _) = observer_failure(at);
        assert_eq!(
            failure.to_string(),
            "execution observer: collection observer refused"
        );
        let cause = failure
            .source()
            .unwrap()
            .downcast_ref::<WorldViewError>()
            .unwrap();
        assert!(std::ptr::eq(cause, failure.cause()));
        let solve = cause
            .source()
            .unwrap()
            .downcast_ref::<zetesis_solve::SolveFailure>()
            .unwrap();
        let inner = solve
            .source()
            .unwrap()
            .downcast_ref::<SolveError>()
            .unwrap();
        assert!(std::ptr::eq(inner, solve.cause.as_ref()));
        let external = inner.source().unwrap().downcast_ref::<io::Error>().unwrap();
        assert_eq!(external.kind(), io::ErrorKind::ConnectionAborted);
        assert_eq!(external.to_string(), "collection observer refused");
        assert!(external.source().is_none());
    }
}
