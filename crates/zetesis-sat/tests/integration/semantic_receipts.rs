//! Native receipt construction keeps membership separate from coverage.

use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory};
use zetesis_sat::{
    BatchError, BatchLimits, Cancellation, Check, Incomplete, Limits, StableModels,
    check_interpretation,
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
    );
    assert!(decision.accepted());
    assert!(decision.candidate().theory().same_instance(&theory));
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
    .into_stable_interpretation()
    .unwrap_err();
    assert!(matches!(decision.verdict(), Check::NotModel));
    assert!(decision.candidate().theory().same_instance(&theory));
}

#[test]
fn nonminimality_cannot_yield_a_stable_receipt() {
    let theory = Theory::new(1, vec![], vec![], AdmissionLimits::default()).unwrap();
    let decision = check_interpretation(
        Interpretation::new(&theory, [0]).unwrap(),
        Limits::default(),
        &Cancellation::default(),
    )
    .into_stable_interpretation()
    .unwrap_err();
    let Check::NonMinimal(witness) = decision.verdict() else {
        panic!("expected a proper-subset countermodel");
    };
    assert!(witness.atoms().next().is_none());
    assert!(witness.theory().same_instance(&theory));
    assert_eq!(decision.candidate().atoms().collect::<Vec<_>>(), [0]);
}

#[test]
fn inconclusive_attempt_cannot_yield_a_stable_receipt() {
    let theory = fact();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let decision = check_interpretation(
        Interpretation::new(&theory, [0]).unwrap(),
        Limits::default(),
        &cancellation,
    )
    .into_stable_interpretation()
    .unwrap_err();
    assert!(matches!(
        decision.verdict(),
        Check::Inconclusive(Incomplete::Cancelled)
    ));
    assert_eq!(decision.candidate().atoms().collect::<Vec<_>>(), [0]);
}

#[test]
fn verified_scalar_step_preserves_native_work() {
    let theory = fact();
    let mut raw = StableModels::new(&theory, Limits::default(), Cancellation::default()).unwrap();
    let mut verified =
        StableModels::new(&theory, Limits::default(), Cancellation::default()).unwrap();
    let interpretation = raw.next().unwrap().unwrap();
    let receipt = verified.next_verified().unwrap().unwrap();
    assert!(receipt.theory().same_instance(&theory));
    assert_eq!(
        interpretation.atoms().collect::<Vec<_>>(),
        receipt.interpretation().atoms().collect::<Vec<_>>()
    );
    assert_eq!(raw.statistics(), verified.statistics());
    assert!(!verified.exhausted());
    assert!(raw.next().is_none());
    assert!(verified.next_verified().is_none());
    assert!(verified.exhausted());
    assert_eq!(raw.statistics(), verified.statistics());
}

#[test]
fn verified_scalar_refusal_does_not_claim_exhaustion() {
    let theory = fact();
    let mut models = StableModels::new(
        &theory,
        Limits {
            max_candidates: 0,
            ..Limits::default()
        },
        Cancellation::default(),
    )
    .unwrap();
    assert!(matches!(
        models.next_verified(),
        Some(Err(Incomplete::CandidateLimit))
    ));
    assert!(models.next_verified().is_none());
    assert!(!models.exhausted());
    assert_eq!(models.statistics().stable_models, 0);
}

#[test]
fn verified_scalar_step_refuses_unresolved_batch_proposals() {
    let theory = fact();
    let mut models =
        StableModels::new(&theory, Limits::default(), Cancellation::default()).unwrap();
    let result = models.next_batch(
        BatchLimits {
            max_candidates: std::num::NonZeroUsize::new(1).unwrap(),
            max_pending_bytes: 1024,
        },
        |_, _| Err("checker unavailable"),
    );
    assert!(matches!(
        result,
        Err(BatchError::Checker("checker unavailable"))
    ));
    assert_eq!(models.batch_statistics().pending, 1);
    assert!(matches!(
        models.next_verified(),
        Some(Err(Incomplete::PendingBatch))
    ));
    assert_eq!(models.statistics().stable_models, 0);
    assert!(!models.exhausted());
}
