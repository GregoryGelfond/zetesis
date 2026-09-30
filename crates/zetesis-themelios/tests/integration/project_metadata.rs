//! Source declarations stay separate from completed fixed-domain projection.

use zetesis_core::Predicate;
use zetesis_reference_support::formula;
use zetesis_test_support::programs::unary as atom;
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource,
    SourceDirective, admit_formula,
};

fn selected_pair() -> (
    zetesis_themelios::AdmittedFormula,
    zetesis_themelios::AdmittedFormula,
) {
    let source = "p(1).{hidden}.#minimize{1@2:hidden}.";
    (
        formula(source),
        formula(&format!("{source} #project p/1. #project p(X):p(X).")),
    )
}

#[test]
fn project_declarations_preserve_the_original_theory() {
    let (original, selected) = selected_pair();
    assert_eq!(selected.atoms(), original.atoms());
    assert_eq!(selected.theory().nodes(), original.theory().nodes());
    assert_eq!(selected.theory().roots(), original.theory().roots());
    assert_eq!(selected.formula_origins(), original.formula_origins());
}

#[test]
fn project_declarations_preserve_objective_priorities() {
    let (original, selected) = selected_pair();
    assert_eq!(
        selected.objectives().priorities(),
        original.objectives().priorities()
    );
}

#[test]
fn project_declarations_leave_show_selection_unchanged() {
    let (original, selected) = selected_pair();
    assert_eq!(selected.metadata().output(), original.metadata().output());
}

#[test]
fn project_metadata_records_the_explicit_domain() {
    let (original, selected) = selected_pair();
    assert!(!original.projection().is_explicit());
    assert!(selected.projection().is_explicit());
    assert_eq!(
        selected.projection().atoms().iter().collect::<Vec<_>>(),
        &[atom("p", 1)]
    );
    let policy = selected.metadata().project_selection();
    assert!(policy.is_explicit());
    assert!(policy.has_conditional_atoms());
    let expected = Predicate::new("p", 1).unwrap();
    assert!(
        policy
            .signatures()
            .iter()
            .eq([zetesis_core::catalog::PredicateRef::from(&expected)])
    );
    assert!(
        selected
            .metadata()
            .directives()
            .iter()
            .any(|entry| matches!(entry.directive(), SourceDirective::ProjectAtom))
    );
}

#[test]
fn absent_project_heads_leave_an_explicit_empty_domain() {
    let result = admit_formula(
        "p(1).#project q(X):p(X).".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_project_atoms: 0,
            max_project_bytes: 0,
            ..FormulaLimits::default()
        },
    )
    .unwrap();
    assert!(result.projection().is_explicit());
    assert!(result.projection().atoms().is_empty());
    assert_eq!(result.atoms().iter().collect::<Vec<_>>(), &[atom("p", 1)]);
}

#[test]
fn source_activity_is_fixed_before_any_answer_is_selected() {
    for (source, count) in [
        ("{p;q}.#project p:q.", 1),
        ("{p;q}.#project p:not q.", 1),
        ("{p;q}.#project p:q,not q.", 1),
        ("q.{p}.#project p:not q.", 0),
        ("{p}.#project p:2=#count{1}.", 0),
    ] {
        assert_eq!(
            formula(source).projection().atoms().len(),
            count,
            "{source}"
        );
    }
}

#[test]
fn pooled_project_declarations_union_into_the_same_fixed_domain() {
    let input = formula("p(1;2).#project p((1;2;3)).#project p/1.");
    assert_eq!(
        input.projection().atoms().iter().collect::<Vec<_>>(),
        &[atom("p", 1), atom("p", 2)]
    );
    assert!(input.projection().contains(&atom("p", 1)));
    assert!(!input.projection().contains(&atom("p", 3)));
}

#[test]
fn projection_admission_refuses_the_complete_owner_at_its_own_limit() {
    let source = "p(1;2).#project p/1.";
    for (limits, resource) in [
        (
            FormulaLimits {
                max_project_atoms: 1,
                ..FormulaLimits::default()
            },
            FormulaResource::ProjectAtoms,
        ),
        (
            FormulaLimits {
                max_project_bytes: 0,
                ..FormulaLimits::default()
            },
            FormulaResource::ProjectBytes,
        ),
    ] {
        let error = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits,
        )
        .unwrap_err();
        assert!(
            matches!(error, FormulaFailure::Limit { resource: actual, .. } if actual == resource),
            "{error}"
        );
    }
    assert_eq!(formula(source).projection().atoms().len(), 2);
}

#[test]
fn declaration_bodies_count_authored_occurrences_before_deduplication() {
    use zetesis_themelios::{AdmissionFailure, ExpansionFailure, InputLimit};
    let source = "p(1).#project p(X):p(X),p(X).";
    let error = admit_formula(
        source.into(),
        AdmissionOptions {
            max_body_elements: 1,
            ..AdmissionOptions::default()
        },
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Limit {
            resource: InputLimit::BodyElements,
            limit: 1,
            observed: 2,
            ..
        }))
    ));
    assert_eq!(
        formula(source)
            .projection()
            .atoms()
            .iter()
            .collect::<Vec<_>>(),
        &[atom("p", 1)]
    );
}

#[test]
fn project_constants_use_a_separate_domain_allowance() {
    let limits = FormulaLimits {
        max_domain_values: 2,
        ..FormulaLimits::default()
    };
    let source = "p(1).";
    let original = formula(source);
    let selected = admit_formula(
        format!("{source}#project q((2;3))."),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        limits,
    )
    .unwrap();
    assert_eq!(selected.atoms(), original.atoms());
    assert_eq!(selected.theory().nodes(), original.theory().nodes());
    assert_eq!(selected.theory().roots(), original.theory().roots());
    assert!(selected.projection().is_explicit());
    assert!(selected.projection().atoms().is_empty());
}

#[test]
fn project_pools_preserve_the_logical_analysis() {
    let original = formula("p(1).");
    let selected = formula("p(1).#project q((2;3)).");
    assert_eq!(selected.analysis_basis(), original.analysis_basis());
    assert_eq!(selected.analyzed_program(), original.analyzed_program());
}
