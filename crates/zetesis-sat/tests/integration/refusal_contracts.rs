//! Typed failures preserve exact reduct enumeration and distinguish incomplete work.

use std::collections::BTreeSet;
use std::error::Error as _;
use std::time::Instant;

use crate::support::clause_search::by_clauses;
use zetesis_cpu::Stop;
use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory};
use zetesis_sat::{Cancellation, Check, Incomplete, Limits, StableModels, check};

fn choices() -> Theory {
    Theory::new(
        2,
        vec![
            Node::False,
            Node::Atom(0),
            Node::Implies(1, 0),
            Node::Or(1, 2),
            Node::Atom(1),
            Node::Implies(4, 0),
            Node::Or(4, 5),
        ],
        vec![3, 6],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn remaining(search: &mut StableModels) -> BTreeSet<Vec<usize>> {
    let result = search
        .by_ref()
        .map(|entry| entry.unwrap().atoms().collect())
        .collect();
    assert!(search.exhausted());
    result
}

#[test]
fn mismatched_restriction_error_does_not_consume_the_live_enumerator() {
    let original = choices();
    let mut search =
        StableModels::new(&original, Limits::default(), Cancellation::default()).unwrap();
    let wrong = Theory::new(1, vec![], vec![], AdmissionLimits::default()).unwrap();
    let before = search.statistics();
    let error = search.restrict_candidates(&wrong).unwrap_err();
    assert_eq!(
        error,
        Incomplete::RestrictionUniverse {
            expected: 2,
            actual: 1
        }
    );
    assert!(error.to_string().contains("declares 1 atoms; expected 2"));
    assert!(error.source().is_none());
    assert_eq!(search.statistics(), before);
    assert_eq!(
        remaining(&mut search),
        BTreeSet::from([vec![], vec![0], vec![1], vec![0, 1]])
    );
    let error = search.restrict_candidates(&original).unwrap_err();
    assert_eq!(error, Incomplete::ClosedEnumerator);
    assert!(error.to_string().contains("already closed"));
    assert!(search.next().is_none());
    assert!(search.exhausted());
}

#[test]
fn late_restriction_capacity_error_keeps_its_cause_and_previous_model_block() {
    let original = choices();
    let mut limits = Limits::default();
    // Refuse the four-clause restriction itself, independently of history.
    limits.admission.max_clauses = 3;
    let mut search = by_clauses(&original, limits, Cancellation::default()).unwrap();
    assert_eq!(search.next().unwrap().unwrap().atoms().count(), 0);
    let guard = Theory::new(
        2,
        vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)],
        vec![2],
        AdmissionLimits::default(),
    )
    .unwrap();
    let error = search.restrict_candidates(&guard).unwrap_err();
    let Incomplete::Admission(cause) = error else {
        panic!("expected bounded clause admission: {error}")
    };
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<zetesis_sat::AdmissionError>(),
        Some(&cause)
    );
    assert_eq!(error.to_string(), cause.to_string());
    assert_eq!(search.statistics().candidate_restrictions, 0);
    assert_eq!(
        remaining(&mut search),
        BTreeSet::from([vec![0], vec![1], vec![0, 1]])
    );
}

#[test]
fn independent_verification_and_foreign_candidate_failures_are_not_rejections() {
    let original = choices();
    let candidate = Interpretation::new(&original, [0]).unwrap();
    let Check::Inconclusive(error) = check(
        &original,
        &candidate,
        Limits {
            max_verification_work: 0,
            ..Default::default()
        },
        &Cancellation::default(),
    ) else {
        panic!("verification must be incomplete")
    };
    assert_eq!(error, Incomplete::Verification(Stop::WorkLimit));
    assert_eq!(
        error.source().unwrap().downcast_ref::<Stop>(),
        Some(&Stop::WorkLimit)
    );
    assert!(error.to_string().starts_with("independent verification:"));
    let other = choices();
    let foreign = Interpretation::new(&other, [0]).unwrap();
    let Check::Inconclusive(error) = check(
        &original,
        &foreign,
        Limits::default(),
        &Cancellation::default(),
    ) else {
        panic!("foreign candidate must be incomplete")
    };
    assert_eq!(error, Incomplete::WrongTheory);
    assert!(error.to_string().contains("different theory"));
    assert!(error.source().is_none());
    let Check::Inconclusive(error) = check(
        &original,
        &candidate,
        Limits::default(),
        &Cancellation::with_deadline(Instant::now()).unwrap(),
    ) else {
        panic!("deadline must be incomplete")
    };
    assert_eq!(error, Incomplete::Deadline);
    assert!(error.to_string().contains("deadline expired"));
    assert!(
        check(
            &original,
            &candidate,
            Limits::default(),
            &Cancellation::default()
        )
        .accepted()
    );
}
