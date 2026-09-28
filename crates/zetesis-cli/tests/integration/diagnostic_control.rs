//! Admission cancellation and optional-pruning diagnostics remain fallible.

use crate::support::bounded_writer;

use std::io;

use bounded_writer::BoundedWriter;
use clap::Parser;
use zetesis_cli::{Completion, Interruption, Options, RunError, run_with_diagnostics};
use zetesis_cpu::Cancellation;
use zetesis_themelios::objective_bound::{ObjectivePlan, ObjectivePlanLimits};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

const SOURCE: &str = "1 {a;b} 1. #minimize{1,a:a;2,b:b}.";

fn options() -> Options {
    Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--workers",
        "1",
        "--models",
        "0",
    ])
    .unwrap()
}

fn planning_work() -> u64 {
    let admitted = admit_formula(
        SOURCE.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    ObjectivePlan::new(
        admitted.theory(),
        admitted.atoms(),
        admitted.objectives(),
        ObjectivePlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap()
    .statistics()
    .work
}

#[test]
fn real_optional_plan_and_bound_refusals_propagate_diagnostic_writer_errors() {
    let mut no_keys = options();
    no_keys.max_objective_keys = 0;
    let mut no_bound_work = options();
    no_bound_work.max_objective_bound_work = planning_work();
    for (options, marker, cause) in [
        (no_keys, "Objective pruning unavailable:", "Keys"),
        (no_bound_work, "Objective pruning stopped:", "Work"),
    ] {
        let mut complete = Vec::new();
        let report = run_with_diagnostics(
            SOURCE.into(),
            &options,
            &mut io::sink(),
            &mut complete,
            &Cancellation::default(),
        )
        .unwrap();
        if cause == "Work" {
            assert_eq!(report.completion, Completion::Exhausted);
        }
        let text = std::str::from_utf8(&complete).unwrap();
        let start = text
            .find(marker)
            .expect("the real resource refusal is reached");
        let end = start + text[start..].find('\n').unwrap();
        assert!(text[start..end].contains(cause), "{text}");
        for capacity in [start, start + marker.len(), end] {
            let mut diagnostics = BoundedWriter::new(capacity);
            let mut output = Vec::new();
            let error = run_with_diagnostics(
                SOURCE.into(),
                &options,
                &mut output,
                &mut diagnostics,
                &Cancellation::default(),
            )
            .unwrap_err();
            let RunError::Output(error) = error else {
                panic!("expected output failure: {error}");
            };
            assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
            assert_eq!(error.to_string(), "diagnostic sink closed");
            assert_eq!(diagnostics.bytes(), &complete[..capacity]);
            let output = String::from_utf8(output).unwrap();
            assert!(!output.contains("OPTIMUM FOUND"));
            assert!(!output.contains("Coverage: exhausted"));
            assert!(!output.contains("UNSATISFIABLE"));
        }
    }
}

#[test]
fn cancellation_before_auto_formula_fallback_has_no_fabricated_admission_or_model() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut options = options();
    options.max_atoms = 0;
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        "a | b.".into(),
        &options,
        &mut output,
        &mut diagnostics,
        &cancellation,
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Interrupted);
    assert_eq!(
        report.interruption,
        Some(Interruption::Preparation(zetesis_cpu::Stop::Cancelled))
    );
    assert_eq!((report.models, report.checked), (0, 0));
    assert!(report.optimization.is_none());
    assert!(report.countermodel_statistics.is_none());
    assert!(report.formula_execution.is_none());
    assert!(report.lazy_execution.is_none());
    assert!(report.shared_execution.is_none());
    assert!(
        diagnostics.is_empty(),
        "no formula execution or backend started"
    );
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("INCOMPLETE"));
    assert!(!output.contains("Answer:"));
    assert!(!output.contains("UNSATISFIABLE"));
    assert!(!output.contains("OPTIMUM FOUND"));
}
