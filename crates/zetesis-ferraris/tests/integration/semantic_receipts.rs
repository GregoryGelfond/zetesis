//! Native records bind decisions to their immutable input interpretation.

use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{
    AdmissionLimits, Interpretation, Limits, Node, Theory, Verdict, check_interpretation,
};

fn fact() -> Theory {
    Theory::new(1, vec![Node::Atom(0)], vec![0], AdmissionLimits::default()).unwrap()
}

#[test]
fn accepted_receipt_retains_the_checked_subject() {
    let theory = fact();
    let decision = check_interpretation(
        Interpretation::new(&theory, [0]).unwrap(),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(decision.accepted());
    assert!(decision.candidate().theory().same_instance(&theory));
    assert!(decision.statistics().work > 0);
    let stable = decision.into_stable_interpretation().unwrap();
    assert!(stable.theory().same_instance(&theory.clone()));
    assert!(!stable.theory().same_instance(&fact()));
    assert_eq!(stable.interpretation().atoms().collect::<Vec<_>>(), [0]);
    assert_eq!(
        stable.into_interpretation().atoms().collect::<Vec<_>>(),
        [0]
    );
}

#[test]
fn original_rejection_cannot_yield_a_stable_receipt() {
    let theory = fact();
    let decision = check_interpretation(
        Interpretation::new(&theory, []).unwrap(),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
    .into_stable_interpretation()
    .unwrap_err();
    assert!(matches!(decision.verdict(), Verdict::NotModel { root: 0 }));
    assert!(decision.candidate().theory().same_instance(&theory));
}

#[test]
fn nonminimality_cannot_yield_a_stable_receipt() {
    // An unsupported true atom satisfies the empty theory but is not stable.
    let theory = Theory::new(1, vec![], vec![], AdmissionLimits::default()).unwrap();
    let decision = check_interpretation(
        Interpretation::new(&theory, [0]).unwrap(),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
    .into_stable_interpretation()
    .unwrap_err();
    let Verdict::NonMinimal { witness } = decision.verdict() else {
        panic!("expected a proper-subset countermodel");
    };
    assert!(witness.atoms().next().is_none());
    assert!(witness.theory().same_instance(&theory));
    assert_eq!(decision.candidate().atoms().collect::<Vec<_>>(), [0]);
}

#[test]
fn work_refusal_cannot_yield_a_completed_decision() {
    let theory = fact();
    let result = check_interpretation(
        Interpretation::new(&theory, [0]).unwrap(),
        Limits {
            max_work: 0,
            ..Limits::default()
        },
        &Cancellation::default(),
    );
    assert!(matches!(result, Err(Stop::WorkLimit)));
}
