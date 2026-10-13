//! Both public candidate doors poll after their selected materialization work.

use super::*;

#[test]
fn cancelled_materialization_does_not_count_an_emitted_candidate() {
    let program = Program::new(vec![], zetesis_core::AdmissionLimits::default()).unwrap();
    let cancellation = Cancellation::default();
    let mut candidates =
        Candidates::new(&program, CandidateLimits::default(), cancellation.clone());
    let result = candidates.pull(|selection| {
        cancellation.cancel();
        selection.to_seed()
    });
    assert!(matches!(result, Some(Err(Stop::Cancelled))));
    assert_eq!(candidates.emitted, 0);
    assert_eq!(
        candidates.termination(),
        Some(CandidateTermination::Stopped(Stop::Cancelled))
    );
    assert!(candidates.next_selection().is_none());
    assert!(candidates.next().is_none());
}

#[test]
fn an_exhausted_enumeration_stays_exhausted() {
    // The empty program has one candidate, the empty interpretation.
    let program = Program::new(vec![], zetesis_core::AdmissionLimits::default()).unwrap();
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    assert!(matches!(candidates.next(), Some(Ok(_))));
    assert!(candidates.next().is_none());
    assert_eq!(
        candidates.termination(),
        Some(CandidateTermination::Exhausted)
    );
    assert!(candidates.next().is_none());
    assert!(candidates.next_selection().is_none());
}
