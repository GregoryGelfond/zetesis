//! An objective instance needs its condition and every scalar field defined:
//! one that divides by zero is omitted, and its statement keeps a warning.

use crate::support::warnings::zero_divisor_statement;
use zetesis_core::Value;
use zetesis_reference_support::admit;
use zetesis_themelios::{
    AdmittedFormula, ExpansionFailure, FormulaFailure, FormulaLimits, observation::EvaluationError,
};

fn refused(source: &str, expected: &EvaluationError) {
    let failure = admit(source, &FormulaLimits::default()).unwrap_err();
    assert!(
        matches!(&failure,
        FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, .. }) if error == expected),
        "{source}: {failure}"
    );
}

/// `source` admitted with exactly one warning: a zero divisor omitted from
/// an instance of the objective statement `objective`.
fn omitting(source: &str, objective: &str) -> AdmittedFormula {
    let admitted = admit(source, &FormulaLimits::default())
        .unwrap_or_else(|failure| panic!("{source}: {failure}"));
    assert_eq!(zero_divisor_statement(&admitted), objective, "{source}");
    admitted
}

#[test]
fn mixed_objective_fields_retain_only_defined_instances() {
    for (fields, priority) in [("1/X@0,X", 0), ("1@1/X,X", 1), ("1@0,1/X", 0)] {
        let source = format!("d(0..1). #minimize{{{fields}:d(X)}}.");
        let admitted = admit(&source, &FormulaLimits::default()).unwrap();
        assert_eq!(admitted.warnings().len(), 1, "{source}");
        assert_eq!(
            admitted.objectives().templates().len(),
            1,
            "one defined objective instance: {source}"
        );
        let template = admitted.objectives().templates().at(0).unwrap();
        assert_eq!(
            template.weight(),
            zetesis_core::TemplateTerm::Constant((&Value::Number(1)).into()),
            "{source}"
        );
        assert_eq!(template.priority(), priority, "{source}");
        assert_eq!(
            template.tuple().iter().collect::<Vec<_>>(),
            [zetesis_core::TemplateTerm::Constant(
                (&Value::Number(1)).into()
            )],
            "{source}"
        );
    }
}

#[test]
fn wholly_undefined_objective_fields_leave_no_instance() {
    // Each field in turn divides by zero at the one row, which is omitted:
    // the objective has no instance and keeps the warning.
    for fields in ["1/X@0,X", "1@1/X,X", "1@0,1/X"] {
        let objective = format!("#minimize{{{fields}:d(X)}}.");
        let admitted = omitting(&format!("d(0). {objective}"), &objective);
        assert!(
            admitted.objectives().templates().is_empty(),
            "no objective instance: {objective}"
        );
    }
}

#[test]
fn an_objective_instance_needs_every_field_defined() {
    // The weight divides by zero at X = 0 and the priority at X = 1, so each
    // instance has an undefined field: both are omitted, under one warning.
    let objective = "#minimize{1/X@1/(1-X),X:d(X)}.";
    let admitted = omitting(&format!("d(0..1). {objective}"), objective);
    assert!(admitted.objectives().templates().is_empty());
}

#[test]
fn pooled_objective_fragments_warn_once_for_their_statement() {
    // One alternative of the pooled weight divides by zero at the one row
    // and is omitted; the other keeps its instance. The one warning is
    // located at the objective as written, pool included.
    for objective in [
        "#minimize{1/(X;X+1)@0,X:d(X)}.",
        "#minimize{1/(X+1;X)@0,X:d(X)}.",
        ":~d(X).[1/(X;X+1)@0,X]",
    ] {
        let admitted = omitting(&format!("d(0). {objective}"), objective);
        assert_eq!(
            admitted.objectives().templates().len(),
            1,
            "one defined pooled alternative: {objective}"
        );
        let template = admitted.objectives().templates().at(0).unwrap();
        assert_eq!(
            template.weight(),
            zetesis_core::TemplateTerm::Constant((&Value::Number(1)).into()),
            "{objective}"
        );
    }
}

#[test]
fn a_defined_objective_element_does_not_hide_an_omitted_one() {
    // Element a divides by zero at the one row and is omitted. Element b is
    // defined there: it keeps its own instance, and neither removes the
    // warning nor restores an instance of a.
    for elements in [
        "1/X@0,a:d(X);1/(X+1)@0,b:d(X)",
        "1/(X+1)@0,b:d(X);1/X@0,a:d(X)",
    ] {
        let objective = format!("#minimize{{{elements}}}.");
        let admitted = omitting(&format!("d(0). {objective}"), &objective);
        assert_eq!(
            admitted.objectives().templates().len(),
            1,
            "the defined element's instance: {objective}"
        );
        let template = admitted.objectives().templates().at(0).unwrap();
        assert_eq!(
            template.weight(),
            zetesis_core::TemplateTerm::Constant((&Value::Number(1)).into()),
            "{objective}"
        );
        assert_eq!(
            template.tuple().iter().collect::<Vec<_>>(),
            [zetesis_core::TemplateTerm::Constant(
                (&Value::Symbol("b".into())).into()
            )],
            "{objective}"
        );
    }
}

#[test]
fn a_false_objective_row_does_not_hide_an_omitted_one() {
    // The condition divides by zero at X = 0, so that row is omitted with a
    // warning. At X = 1 the condition is defined and false: the row is
    // excluded before its weight 1/(1-X) is read, and neither removes that
    // warning nor leaves an instance.
    let objective = "#minimize{1/(1-X)@0,X:d(X),1/X=0}.";
    let admitted = omitting(&format!("d(0..1). {objective}"), objective);
    assert!(admitted.objectives().templates().is_empty());
}

#[test]
fn zero_priority_cannot_hide_an_independent_field_overflow() {
    for fields in ["2147483647+(1-X)@1/X,X", "1@1/X,2147483647+(1-X)"] {
        // X=0 reaches zero in priority and overflow in a later field; X=1 is
        // defined. Merely dropping the first field's failure would admit it.
        refused(
            &format!("d(0..1). #minimize{{{fields}:d(X)}}."),
            &EvaluationError::Overflow,
        );
    }
}

#[test]
fn zero_weight_branch_cannot_hide_independent_overflow() {
    refused(
        "d(0..1). #minimize{1/X+((2147483647+(1-X))\\2)@0,X:d(X)}.",
        &EvaluationError::Overflow,
    );
}

#[test]
fn zero_weight_branch_cannot_hide_an_undefined_power() {
    refused(
        "d(0..1). #minimize{1/X+2**(X-1)@0,X:d(X)}.",
        &EvaluationError::Undefined,
    );
}

#[test]
fn guarded_objective_fields_emit_no_warning() {
    let admitted = admit(
        "d(0..1). #minimize{1/X@0,X:d(X),X!=0}.",
        &FormulaLimits::default(),
    )
    .unwrap();
    assert!(admitted.warnings().is_empty());
    assert_eq!(admitted.objectives().templates().len(), 1);
}

#[test]
fn outer_objective_guards_exclude_undefined_local_families() {
    let admitted = admit(
        "d(0..1). e(0,0). e(1,1). :~d(K),K!=0,#count{1/X:e(K,X)}>0.[1@0,K]",
        &FormulaLimits::default(),
    )
    .unwrap();
    assert!(admitted.warnings().is_empty());
    assert_eq!(admitted.objectives().templates().len(), 1);
}

#[test]
fn objective_condition_warnings_survive_lifted_fields() {
    let admitted = admit(
        "d(0..2). #minimize{1@0,X:d(X),1/X=1}.",
        &FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(admitted.warnings().len(), 1);
    assert_eq!(admitted.objectives().templates().len(), 1);
}
