//! Typed failures preserve exact reduct enumeration and distinguish incomplete work.

use crate::support::choice_theories::choices;
use std::collections::BTreeSet;
use std::error::Error as _;
use std::time::Instant;

use zetesis_cpu::Stop;
use zetesis_ferraris::{AdmissionLimits, Interpretation, Theory};
use zetesis_stable::{Cancellation, Check, Incomplete, Limits, StableModels, check};

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
    let original = choices(2);
    let mut search =
        StableModels::new(&original, Limits::default(), Cancellation::default()).unwrap();
    let wrong = Theory::new(
        1,
        zetesis_ferraris::FormulaParts::new(vec![], vec![]).unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
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
fn independent_verification_and_foreign_candidate_failures_are_not_rejections() {
    let original = choices(2);
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
    let other = choices(2);
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
