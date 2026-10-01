//! Quiet execution suppresses information, never a diagnostic or its I/O failure.

use crate::presentation::{Diagnostics, Label};
use crate::{ColorMode, ExecutionObservation, ExecutionObserver, Grounder, Oracle, SearchMethod};
use zetesis_test_support::io::BoundedWriter;

#[test]
fn quiet_information_never_touches_the_writer() {
    let mut writer = BoundedWriter::new(0);
    let mut diagnostics = Diagnostics::new(&mut writer, ColorMode::Never).with_quiet(true);
    diagnostics
        .metadata(Label::Source, format_args!("input.lp"))
        .unwrap();
    diagnostics
        .information(format_args!("Projection: completed"))
        .unwrap();
    diagnostics
        .observe(ExecutionObservation::CpuFormula {
            oracle: Oracle::Auto,
            grounder: Grounder::Auto,
            search: SearchMethod::Regions,
        })
        .unwrap();
    diagnostics
        .observe(ExecutionObservation::ObjectiveBound {
            restrictions: 1,
            costs: &[(0, 7)],
            work: 8,
        })
        .unwrap();
    assert!(writer.bytes().is_empty());
}

#[test]
fn quiet_diagnostics_preserve_source_messages() {
    for message in [
        "warning: missing arithmetic guard",
        "error: source was refused",
    ] {
        let mut output = Vec::new();
        Diagnostics::new(&mut output, ColorMode::Never)
            .with_quiet(true)
            .diagnostic(&message)
            .unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            format!("zetesis: {message}\n")
        );
    }
}

#[test]
fn quiet_diagnostics_preserve_objective_refusals() {
    for (event, expected) in [
        (
            ExecutionObservation::ObjectiveTheoryMismatch,
            "Objective pruning stopped: original theory mismatch; exact search continues\n",
        ),
        (
            ExecutionObservation::ObjectiveRestrictionStopped(zetesis_sat::Incomplete::Admission(
                zetesis_sat::AdmissionError::Limit {
                    resource: zetesis_sat::Resource::Variables,
                    observed: 3,
                    limit: 2,
                },
            )),
            "Objective pruning stopped: SAT Variables count 3 exceeds 2; exact search continues\n",
        ),
    ] {
        let mut output = Vec::new();
        Diagnostics::new(&mut output, ColorMode::Never)
            .with_quiet(true)
            .observe(event)
            .unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), expected);
    }
}

#[test]
fn quiet_diagnostic_writer_errors_remain_failures() {
    let mut writer = BoundedWriter::new(0);
    let error = Diagnostics::new(&mut writer, ColorMode::Never)
        .with_quiet(true)
        .diagnostic(&"warning: missing guard")
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
}
