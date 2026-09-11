//! Grounding snapshots preserve unavailable values and every failed output prefix.

use crate::test_writer::BoundedWriter;
use crate::{
    GroundingMeasurement, GroundingOutcome, GroundingPhase, GroundingWork, SolveMeasurements,
};
use zetesis_themelios::GroundingObserver as _;

#[test]
fn every_attribution_prefix_preserves_writer_failure() {
    let measurements = SolveMeasurements::new(true);
    let observer = measurements.grounding_observer().unwrap();
    observer.enter();
    observer.phase_enter(GroundingPhase::RuleInstantiation, None);
    observer.phase_exit(
        GroundingPhase::RuleInstantiation,
        None,
        GroundingOutcome::Completed,
        GroundingWork::default(),
    );
    observer.exit();
    let timings = measurements.snapshot().unwrap().grounding;
    let mut expected = Vec::new();
    super::write(&mut expected, &timings).unwrap();
    let text = std::str::from_utf8(&expected).unwrap();
    assert!(text.contains("completed=1;"));
    assert_eq!(
        text.matches(": unmeasured\n").count(),
        GroundingPhase::ALL.len() - 1
    );
    for capacity in 0..expected.len() {
        let mut writer = BoundedWriter::new(capacity);
        assert_eq!(
            super::write(&mut writer, &timings).unwrap_err().kind(),
            std::io::ErrorKind::BrokenPipe
        );
        assert_eq!(writer.bytes(), &expected[..capacity]);
    }
    let mut complete = BoundedWriter::new(expected.len());
    super::write(&mut complete, &timings).unwrap();
    assert_eq!(complete.bytes(), expected);
}

#[test]
fn attribution_output_preserves_unavailable_values() {
    let mut measurement = GroundingMeasurement::default();
    measurement.work.join_rows = None;
    measurement.elapsed = None;
    let mut expected = Vec::new();
    super::write_measurement(&mut expected, &measurement).unwrap();
    let text = std::str::from_utf8(&expected).unwrap();
    assert!(text.contains("elapsed_ns=unavailable;"));
    assert!(text.contains("join_rows=unavailable;"));
    for capacity in 0..expected.len() {
        let mut writer = BoundedWriter::new(capacity);
        assert_eq!(
            super::write_measurement(&mut writer, &measurement)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::BrokenPipe
        );
        assert_eq!(writer.bytes(), &expected[..capacity]);
    }
}
