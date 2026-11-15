//! Projection refusals preserve typed byte evidence and whole diagnostic units.

use crate::presentation::Diagnostics;
use crate::{ColorMode, ExecutionObservation, ExecutionObserver};
use zetesis_algebra::Cancellation;
use zetesis_reference_support::formula;
use zetesis_test_support::io::{BoundedWriter, Chunks, FULL};
use zetesis_themelios::objective_bound::{
    ObjectivePlan, ObjectivePlanLimits, ObjectiveProjectionError, ObjectiveProjectionErrorKind,
};

fn refusal() -> ObjectiveProjectionError {
    let owner = formula("{a}. #minimize {1:a}.");
    let error = ObjectivePlan::new_projected(
        owner.objective_source(),
        ObjectivePlanLimits::default(),
        0,
        &Cancellation::default(),
    )
    .error()
    .unwrap();
    assert!(
        matches!(error.kind(), ObjectiveProjectionErrorKind::Bytes { required, limit: 0 } if *required > 0)
    );
    error
}

fn expected(error: ObjectiveProjectionError) -> String {
    format!(
        "Objective projection unavailable: {error}; detailed scoring is subject to the remaining objective allowance\n"
    )
}

#[test]
fn projected_byte_refusal_is_one_exact_unit_even_when_quiet() {
    let error = refusal();
    let reference = expected(error);
    for quiet in [false, true] {
        let mut output = Chunks::new();
        Diagnostics::new(&mut output, ColorMode::Never)
            .with_quiet(quiet)
            .observe(ExecutionObservation::ObjectiveProjectionUnavailable(error))
            .unwrap();
        assert_eq!(output.chunks().len(), 1);
        assert_eq!(output.chunks()[0].as_slice(), reference.as_bytes());
    }
}

#[test]
fn projected_diagnostic_failure_keeps_each_accepted_prefix() {
    let error = refusal();
    let reference = expected(error);
    for capacity in 0..reference.len() {
        let mut output = BoundedWriter::new(capacity);
        let failure = Diagnostics::new(&mut output, ColorMode::Never)
            .with_quiet(true)
            .observe(ExecutionObservation::ObjectiveProjectionUnavailable(error))
            .unwrap_err();
        assert_eq!(failure.kind(), std::io::ErrorKind::BrokenPipe);
        assert_eq!(failure.to_string(), FULL);
        assert_eq!(output.bytes(), &reference.as_bytes()[..capacity]);
    }
}
