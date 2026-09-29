//! Public timing snapshots retain exact output-prefix failure behavior.

use crate::{SolveMeasurements, SolvePhase};
use std::{io, time::Duration};
use zetesis_sat::PhaseMeasurement;
use zetesis_test_support::io::BoundedWriter;

#[test]
fn every_phase_output_prefix_propagates_write_failure() {
    let measurements = SolveMeasurements::new(true);
    measurements.measure(SolvePhase::AdmissionMaterialization, || ());
    measurements.measure(SolvePhase::AdmissionMaterialization, || ());
    let mut timings = measurements.snapshot().unwrap();
    timings.driver_elapsed = Duration::from_nanos(23);
    let mut reference = Vec::new();
    super::write(&mut reference, &timings).unwrap();
    let text = std::str::from_utf8(&reference).unwrap();
    assert!(text.contains("phase driver: elapsed_ns=23\n"));
    assert!(text.contains(&format!(
            "phase admission_materialization: calls=2; elapsed_ns={}; complete=true\n",
            timings
                .get(SolvePhase::AdmissionMaterialization)
                .unwrap()
                .elapsed
                .as_nanos()
        )));
    assert_eq!(
        text.matches(": unmeasured\n").count(),
        SolvePhase::ALL.len() - 1
    );
    assert!(text.contains("phase reduct_preparation: unmeasured\n"));
    assert!(text.ends_with("kernel_time=unmeasured\n"));
    for capacity in 0..reference.len() {
        let mut output = BoundedWriter::new(capacity);
        assert_eq!(
            super::write(&mut output, &timings).unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        assert_eq!(output.bytes(), &reference[..capacity]);
    }
    let mut output = BoundedWriter::new(reference.len());
    super::write(&mut output, &timings).unwrap();
    assert_eq!(output.bytes(), reference);
}

#[test]
fn phase_output_preserves_incomplete_measurements() {
    let measurement = PhaseMeasurement {
        calls: 1,
        elapsed: Duration::from_nanos(3),
        overflowed: true,
    };
    let expected = b"  phase objective_feedback: calls=1; elapsed_ns=3; complete=false\n";
    let mut complete = Vec::new();
    super::write_phase(
        &mut complete,
        SolvePhase::ObjectiveFeedback,
        Some(measurement),
    )
    .unwrap();
    assert_eq!(complete, expected);
    for capacity in 0..expected.len() {
        let mut output = BoundedWriter::new(capacity);
        assert_eq!(
            super::write_phase(
                &mut output,
                SolvePhase::ObjectiveFeedback,
                Some(measurement)
            )
            .unwrap_err()
            .kind(),
            io::ErrorKind::BrokenPipe
        );
        assert_eq!(output.bytes(), &expected[..capacity]);
    }
}
