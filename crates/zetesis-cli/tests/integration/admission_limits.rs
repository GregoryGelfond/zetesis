//! CLI source limits preserve the library's distinct resource populations.

use std::io;

use clap::Parser;
use zetesis_cli::{Completion, Options, RunError, run_with_diagnostics};
use zetesis_cpu::Cancellation;
use zetesis_themelios::{FormulaFailure, FormulaResource};

fn formula(source: &str, arguments: &[&str]) -> Result<Completion, RunError> {
    let options = Options::try_parse_from(
        [
            "zetesis",
            "--backend",
            "cpu",
            "--oracle",
            "countermodel",
            "--models",
            "0",
        ]
        .into_iter()
        .chain(arguments.iter().copied()),
    )
    .unwrap();
    let mut output = Vec::new();
    let result = run_with_diagnostics(
        source.into(),
        &options,
        &mut output,
        &mut io::sink(),
        &Cancellation::default(),
    );
    if result.is_err() {
        assert!(
            crate::support::human::preamble(std::str::from_utf8(&output).unwrap()),
            "admission must precede answer publication"
        );
    }
    result.map(|report| report.completion)
}

#[test]
fn domain_override_controls_formula_admission() {
    let source = "p(1). p(2).";
    assert!(matches!(
        formula(source, &["--max-domain-values", "1"]),
        Err(RunError::FormulaAdmission(FormulaFailure::Limit {
            resource: FormulaResource::DomainValues,
            observed: 2,
            limit: 1,
            ..
        }))
    ));
    assert_eq!(
        formula(source, &["--max-domain-values", "2"]).unwrap(),
        Completion::Exhausted
    );
}

#[test]
fn assignment_override_bounds_one_range() {
    let source = "p(X):-X=1..2.";
    assert!(matches!(
        formula(source, &["--max-assignment-values", "1"]),
        Err(RunError::FormulaAdmission(FormulaFailure::Limit {
            resource: FormulaResource::AssignmentValues,
            observed: 2,
            limit: 1,
            ..
        }))
    ));
    assert_eq!(
        formula(source, &["--max-assignment-values", "2"]).unwrap(),
        Completion::Exhausted
    );
}

#[test]
fn generated_values_have_a_cumulative_limit() {
    let source = "d(1..2).p(f(X)):-d(X).";
    assert!(matches!(
        formula(source, &["--max-generated-values", "1"]),
        Err(RunError::FormulaAdmission(FormulaFailure::Limit {
            resource: FormulaResource::GeneratedValues,
            observed: 2,
            limit: 1,
            ..
        }))
    ));
    assert_eq!(
        formula(source, &["--max-generated-values", "2"]).unwrap(),
        Completion::Exhausted
    );
}

#[test]
fn generated_values_remain_charged_after_support() {
    // The fact generator runs during support construction; the constraint's
    // generator runs during final instantiation and produces no head value.
    let source = "p(X):-X=1..1. :-Y=2..2.";
    assert!(matches!(
        formula(
            source,
            &[
                "--max-assignment-values",
                "1",
                "--max-generated-values",
                "1"
            ]
        ),
        Err(RunError::FormulaAdmission(FormulaFailure::Limit {
            resource: FormulaResource::GeneratedValues,
            observed: 2,
            limit: 1,
            ..
        }))
    ));
    assert_eq!(
        formula(
            source,
            &[
                "--max-assignment-values",
                "1",
                "--max-generated-values",
                "2"
            ]
        )
        .unwrap(),
        Completion::Exhausted
    );
}

#[test]
fn support_round_override_preserves_incomplete_admission() {
    assert!(matches!(
        formula("a.", &["--max-support-rounds", "0"]),
        Err(RunError::FormulaAdmission(FormulaFailure::Limit {
            resource: FormulaResource::SupportRounds,
            observed: 1,
            limit: 0,
            ..
        }))
    ));
    assert_eq!(formula("a.", &[]).unwrap(), Completion::Exhausted);
}
