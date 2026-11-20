//! Attribution remains bounded and preserves unavailable measurements.

use super::*;

#[test]
fn unentered_phases_have_no_measurement() {
    let timings = GroundingTimings::default();
    for phase in GroundingPhase::ALL {
        assert!(timings.get(phase).is_none());
    }
}

#[test]
fn aggregation_preserves_outcome_populations() {
    let mut measurement = GroundingMeasurement::default();
    for (outcome, rows) in [
        (GroundingOutcome::Completed, 3),
        (GroundingOutcome::Failed, 2),
        (GroundingOutcome::Unwound, 1),
        (GroundingOutcome::Completed, 4),
    ] {
        let mut work = GroundingWork::default();
        work.join_rows = Some(rows);
        measurement.record(Duration::from_nanos(rows), outcome, &work);
    }
    assert_eq!(measurement.elapsed, Some(Duration::from_nanos(10)));
    assert_eq!(measurement.work.join_rows, Some(10));
    assert_eq!(measurement.count(GroundingOutcome::Completed), Some(2));
    assert_eq!(measurement.count(GroundingOutcome::Failed), Some(1));
    assert_eq!(measurement.count(GroundingOutcome::Unwound), Some(1));
}

#[test]
fn overflow_preserves_independent_fields() {
    let mut measurement = GroundingMeasurement {
        elapsed: Some(Duration::MAX),
        ..GroundingMeasurement::default()
    };
    measurement.outcomes[outcome_index(GroundingOutcome::Completed)] = Some(u64::MAX);
    measurement.work.join_rows = Some(u64::MAX);
    let mut work = GroundingWork::default();
    work.join_rows = Some(1);
    work.roots = Some(2);
    measurement.record(Duration::from_nanos(1), GroundingOutcome::Completed, &work);
    assert_eq!(measurement.elapsed, None);
    assert_eq!(measurement.count(GroundingOutcome::Completed), None);
    assert_eq!(measurement.count(GroundingOutcome::Failed), Some(0));
    assert_eq!(measurement.work.join_rows, None);
    assert_eq!(measurement.work.roots, Some(2));
}

#[test]
fn a_recorder_retains_separate_phase_attempts() {
    let recorder = Recorder::default();
    for phase in [
        GroundingPhase::SupportCompletion,
        GroundingPhase::RuleInstantiation,
        GroundingPhase::RuleInstantiation,
    ] {
        let attempt = Attempt::start(phase);
        recorder.exit(
            attempt,
            phase,
            GroundingOutcome::Completed,
            &GroundingWork::default(),
            None,
        );
    }
    let timings = recorder.snapshot();
    assert_eq!(
        timings
            .get(GroundingPhase::SupportCompletion)
            .unwrap()
            .count(GroundingOutcome::Completed),
        Some(1)
    );
    assert_eq!(
        timings
            .get(GroundingPhase::RuleInstantiation)
            .unwrap()
            .count(GroundingOutcome::Completed),
        Some(2)
    );
}

#[test]
fn poisoned_grounding_bookkeeping_stays_unavailable_without_stopping_work() {
    let recorder = Recorder::default();
    let phase = GroundingPhase::SupportCompletion;
    let attempt = Attempt::start(phase);
    recorder.exit(
        attempt,
        phase,
        GroundingOutcome::Completed,
        &GroundingWork::default(),
        None,
    );
    assert!(recorder.snapshot().get(phase).is_some());
    let failure = std::panic::catch_unwind(|| {
        let _record = recorder.timings.lock().unwrap();
        panic!("injected grounding bookkeeping failure");
    });
    assert!(failure.is_err());
    assert!(recorder.snapshot().get(phase).is_none());
    let phase = GroundingPhase::RuleInstantiation;
    let attempt = Attempt::start(phase);
    recorder.exit(
        attempt,
        phase,
        GroundingOutcome::Failed,
        &GroundingWork::default(),
        None,
    );
    let snapshot = recorder.snapshot();
    for phase in GroundingPhase::ALL {
        assert!(snapshot.get(phase).is_none());
    }
}

#[test]
fn domain_aggregation_preserves_unavailable_work() {
    // Aggregation scope: supplied observer fields test arithmetic/absence, not
    // a claim that these counts came from a domain-analysis execution.
    let mut measurement = GroundingMeasurement::default();
    let mut first = GroundingWork::default();
    first.domain_prepare_work = Some(11);
    first.domain_guard_rows = Some(7);
    first.domain_guard_checks = Some(u64::MAX);
    first.domain_rejected_rows = Some(3);
    measurement.record(Duration::ZERO, GroundingOutcome::Completed, &first);
    let mut second = GroundingWork::default();
    second.domain_prepare_work = Some(5);
    second.domain_guard_rows = Some(2);
    second.domain_guard_checks = Some(1);
    second.domain_rejected_rows = None;
    measurement.record(Duration::ZERO, GroundingOutcome::Failed, &second);
    assert_eq!(measurement.work.domain_prepare_work, Some(16));
    assert_eq!(measurement.work.domain_guard_rows, Some(9));
    assert_eq!(measurement.work.domain_guard_checks, None);
    assert_eq!(measurement.work.domain_rejected_rows, None);
    assert_eq!(measurement.count(GroundingOutcome::Completed), Some(1));
    assert_eq!(measurement.count(GroundingOutcome::Failed), Some(1));
}

#[test]
fn operators_fold_into_the_phase_that_contains_them() {
    use zetesis_themelios::{GroundingOperator, OperatorObserver};
    let recorder = Recorder::default();
    let operators = Operators::new();
    let phase = GroundingPhase::SupportCompletion;
    let attempt = Attempt::start(phase);
    for operator in GroundingOperator::ALL {
        operators.operator_enter(operator);
        operators.operator_exit(operator);
    }
    operators.operator_enter(GroundingOperator::SupportJoin);
    std::thread::sleep(std::time::Duration::from_millis(2));
    operators.operator_exit(GroundingOperator::SupportJoin);
    recorder.exit(
        attempt,
        phase,
        GroundingOutcome::Completed,
        &GroundingWork::default(),
        Some(&operators),
    );
    let timings = recorder.snapshot();
    let join = timings.operator(GroundingOperator::SupportJoin).unwrap();
    assert!(join >= std::time::Duration::from_millis(2));
    assert!(join <= timings.get(phase).unwrap().elapsed.unwrap());
    // The attempt's totals were moved into the recorder, not kept for the next phase.
    let attempt = Attempt::start(GroundingPhase::RuleInstantiation);
    recorder.exit(
        attempt,
        GroundingPhase::RuleInstantiation,
        GroundingOutcome::Completed,
        &GroundingWork::default(),
        Some(&operators),
    );
    assert_eq!(
        recorder.snapshot().operator(GroundingOperator::SupportJoin),
        Some(join)
    );
}

#[test]
#[should_panic(expected = "must not nest")]
fn operators_do_not_nest() {
    use zetesis_themelios::{GroundingOperator, OperatorObserver};
    let operators = Operators::new();
    operators.operator_enter(GroundingOperator::SupportJoin);
    operators.operator_enter(GroundingOperator::SupportDedup);
}
