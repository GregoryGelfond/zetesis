//! Both public candidate doors poll after their selected materialization work.

use super::*;

#[test]
fn cancelled_materialization_does_not_count_an_emitted_candidate() {
    let program = Program::new(vec![], zetesis_core::AdmissionLimits::default()).unwrap();
    let control = Control::default();
    let mut candidates = Candidates::new(&program, CandidateLimits::default(), control.clone());
    let result = candidates.pull(|selection| {
        control.cancel();
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
