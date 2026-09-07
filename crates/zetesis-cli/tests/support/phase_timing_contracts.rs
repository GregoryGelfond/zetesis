//! Recorder failure/overflow and exact output-prefix contracts without a device.

use std::io;
use std::time::Duration;

use super::{PhaseMeasurement, PhaseTimings, Recorder, SearchPhaseTimings, SolvePhase};
use crate::test_writer::BoundedWriter;

#[test]
fn disabled_recorder_and_returned_errors_do_not_invent_semantic_completion() {
    let disabled = Recorder::new(false);
    assert_eq!(
        disabled.measure(SolvePhase::AdmissionMaterialization, || Err::<(), _>(7)),
        Err(7)
    );
    disabled.search(Some(SearchPhaseTimings::default()));
    assert!(disabled.snapshot().is_none());
    let active = Recorder::new(true);
    assert_eq!(
        active.measure(SolvePhase::AdmissionMaterialization, || Err::<(), _>(7)),
        Err(7)
    );
    assert_eq!(
        active
            .snapshot()
            .unwrap()
            .get(SolvePhase::AdmissionMaterialization)
            .unwrap()
            .calls,
        1
    );
    assert!(
        active
            .snapshot()
            .unwrap()
            .get(SolvePhase::CandidateGeneration)
            .is_none()
    );
}

#[test]
fn cumulative_search_snapshots_replace_instead_of_double_counting_attempts() {
    let active = Recorder::new(true);
    let measurement = PhaseMeasurement {
        calls: 2,
        elapsed: Duration::from_nanos(7),
        overflowed: false,
    };
    let timing = SearchPhaseTimings {
        candidates: measurement,
        ..SearchPhaseTimings::default()
    };
    active.search(Some(timing));
    active.search(Some(timing));
    assert_eq!(
        active
            .snapshot()
            .unwrap()
            .get(SolvePhase::CandidateGeneration),
        Some(measurement)
    );
    assert!(
        active
            .snapshot()
            .unwrap()
            .get(SolvePhase::ExactReductMembership)
            .is_none()
    );
    active.search(Some(SearchPhaseTimings {
        reduct: PhaseMeasurement {
            overflowed: true,
            ..PhaseMeasurement::default()
        },
        ..timing
    }));
    assert!(
        active
            .snapshot()
            .unwrap()
            .get(SolvePhase::ExactReductMembership)
            .unwrap()
            .overflowed
    );
}

#[test]
fn every_phase_output_prefix_propagates_write_failure_and_incomplete_measurements() {
    let mut timings = PhaseTimings {
        driver_elapsed: Duration::from_nanos(23),
        measurements: [None; 11],
    };
    timings.measurements[SolvePhase::AdmissionMaterialization as usize] = Some(PhaseMeasurement {
        calls: 2,
        elapsed: Duration::from_nanos(7),
        overflowed: false,
    });
    timings.measurements[SolvePhase::ObjectiveFeedback as usize] = Some(PhaseMeasurement {
        calls: 1,
        elapsed: Duration::from_nanos(3),
        overflowed: true,
    });
    let mut reference = Vec::new();
    super::write(&mut reference, &timings).unwrap();
    let text = std::str::from_utf8(&reference).unwrap();
    assert!(text.contains("phase driver: elapsed_ns=23\n"));
    assert!(
        text.contains("phase admission_materialization: calls=2; elapsed_ns=7; complete=true\n")
    );
    assert!(text.contains("phase objective_feedback: calls=1; elapsed_ns=3; complete=false\n"));
    assert_eq!(text.matches(": unmeasured\n").count(), 9);
    assert!(text.ends_with("kernel_time=unmeasured\n"));
    for capacity in 0..reference.len() {
        let mut output = BoundedWriter::new(capacity);
        let error = super::write(&mut output, &timings).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(output.bytes(), &reference[..capacity]);
    }
    let mut output = BoundedWriter::new(reference.len());
    super::write(&mut output, &timings).unwrap();
    assert_eq!(output.bytes(), reference);
}
