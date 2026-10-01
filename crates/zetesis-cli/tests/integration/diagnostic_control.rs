//! Admission cancellation and optional-pruning diagnostics remain fallible.

use std::io;

use clap::Parser;
use zetesis_cli::{Completion, Interruption, Options, RunError, run_with_diagnostics};
use zetesis_cpu::Cancellation;
use zetesis_test_support::io::{BoundedWriter, FULL};

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

fn diagnostic_refusal(options: &Options, marker: &str, cause: &str) {
    let mut complete = Vec::new();
    let report = run_with_diagnostics(
        SOURCE.into(),
        options,
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
            options,
            &mut output,
            &mut diagnostics,
            &Cancellation::default(),
        )
        .unwrap_err();
        let RunError::Output(error) = error else {
            panic!("expected output failure: {error}");
        };
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(error.to_string(), FULL);
        assert_eq!(diagnostics.bytes(), &complete[..capacity]);
        let output = String::from_utf8(output).unwrap();
        assert!(!output.contains("OPTIMUM FOUND"));
        assert!(!crate::support::human::exhausted(&output));
        assert!(!output.contains("UNSATISFIABLE"));
    }
}

#[test]
fn preparation_refusal_preserves_diagnostic_failure() {
    let mut options = options();
    options.max_objective_keys = 0;
    diagnostic_refusal(&options, "Objective preparation unavailable:", "Keys");
}

#[test]
fn bound_refusal_preserves_diagnostic_failure() {
    let mut options = options();
    // Preparation has its own objective receipt. One bound unit admits its
    // initialization, then refuses the first copied eligibility node.
    options.max_objective_bound_work = 1;
    diagnostic_refusal(&options, "Objective pruning stopped:", "Work");
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
