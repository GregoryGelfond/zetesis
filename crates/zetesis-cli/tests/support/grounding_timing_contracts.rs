//! Attribution remains bounded and preserves unavailable measurements.

use super::*;
use crate::test_writer::BoundedWriter;

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
        measurement.record(Duration::from_nanos(rows), outcome, work);
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
    measurement.record(Duration::from_nanos(1), GroundingOutcome::Completed, work);
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
        recorder.enter(phase);
        recorder.exit(phase, GroundingOutcome::Completed, GroundingWork::default());
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
fn every_attribution_prefix_preserves_writer_failure() {
    let mut timings = GroundingTimings::default();
    let mut measurement = GroundingMeasurement::default();
    measurement.work.join_rows = None;
    measurement.elapsed = None;
    timings.measurements[phase_index(GroundingPhase::RuleInstantiation)] = Some(measurement);
    let mut expected = Vec::new();
    super::write(&mut expected, &timings).unwrap();
    let text = std::str::from_utf8(&expected).unwrap();
    assert!(text.contains("elapsed_ns=unavailable;"));
    assert!(text.contains("join_rows=unavailable;"));
    for capacity in 0..expected.len() {
        let mut writer = BoundedWriter::new(capacity);
        assert_eq!(
            super::write(&mut writer, &timings).unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        assert_eq!(writer.bytes(), &expected[..capacity]);
    }
}
