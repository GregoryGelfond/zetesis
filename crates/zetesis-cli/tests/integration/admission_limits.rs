//! Typed source limits preserve the library's distinct resource populations.

use zetesis_cli::{Completion, RunError};
use zetesis_themelios::{FormulaFailure, FormulaResource};

fn formula(
    source: &str,
    configure: impl FnOnce(&mut zetesis_themelios::FormulaLimits),
) -> Result<Completion, RunError> {
    let mut limits = zetesis_themelios::FormulaLimits::default();
    configure(&mut limits);
    let (result, output) = crate::support::prepared::bounded_admission(
        source,
        zetesis_themelios::ExpansionLimits::default(),
        &limits,
    );
    if result.is_err() {
        assert!(output.is_empty(), "admission precedes publication");
    }
    result.map(|report| report.completion)
}

#[test]
fn domain_limit_controls_formula_admission() {
    let source = "p(1). p(2).";
    assert!(matches!(
        formula(source, |limits| limits.max_domain_values = 1),
        Err(RunError::FormulaAdmission(FormulaFailure::Limit {
            resource: FormulaResource::DomainValues,
            observed: 2,
            limit: 1,
            ..
        }))
    ));
    assert_eq!(
        formula(source, |limits| limits.max_domain_values = 2).unwrap(),
        Completion::Exhausted
    );
}

#[test]
fn assignment_limit_bounds_one_range() {
    let source = "p(X):-X=1..2.";
    assert!(matches!(
        formula(source, |limits| limits.max_assignment_values = 1),
        Err(RunError::FormulaAdmission(FormulaFailure::Limit {
            resource: FormulaResource::AssignmentValues,
            observed: 2,
            limit: 1,
            ..
        }))
    ));
    assert_eq!(
        formula(source, |limits| limits.max_assignment_values = 2).unwrap(),
        Completion::Exhausted
    );
}

#[test]
fn generated_values_have_a_cumulative_limit() {
    let source = "d(1..2).p(f(X)):-d(X).";
    assert!(matches!(
        formula(source, |limits| limits.max_generated_values = 1),
        Err(RunError::FormulaAdmission(FormulaFailure::Limit {
            resource: FormulaResource::GeneratedValues,
            observed: 2,
            limit: 1,
            ..
        }))
    ));
    assert_eq!(
        formula(source, |limits| limits.max_generated_values = 2).unwrap(),
        Completion::Exhausted
    );
}

#[test]
fn generated_values_remain_charged_after_support() {
    // The fact generator runs during support construction; the constraint's
    // generator runs during final instantiation and produces no head value.
    let source = "p(X):-X=1..1. :-Y=2..2.";
    assert!(matches!(
        formula(source, |limits| {
            limits.max_assignment_values = 1;
            limits.max_generated_values = 1;
        }),
        Err(RunError::FormulaAdmission(FormulaFailure::Limit {
            resource: FormulaResource::GeneratedValues,
            observed: 2,
            limit: 1,
            ..
        }))
    ));
    assert_eq!(
        formula(source, |limits| {
            limits.max_assignment_values = 1;
            limits.max_generated_values = 2;
        })
        .unwrap(),
        Completion::Exhausted
    );
}

#[test]
fn support_round_limit_preserves_incomplete_admission() {
    assert!(matches!(
        formula("a.", |limits| limits.max_support_rounds = 0),
        Err(RunError::FormulaAdmission(FormulaFailure::Limit {
            resource: FormulaResource::SupportRounds,
            observed: 1,
            limit: 0,
            ..
        }))
    ));
    assert_eq!(formula("a.", |_| {}).unwrap(), Completion::Exhausted);
}
