//! Objective families require jointly defined condition and scalar fields.

use zetesis_core::Value;
use zetesis_reference_support::admit;
use zetesis_themelios::{
    ExpansionFailure, FormulaFailure, FormulaLimits, observation::EvaluationError,
};

fn refused(source: &str, expected: &EvaluationError) {
    let failure = admit(source, &FormulaLimits::default()).unwrap_err();
    assert!(
        matches!(&failure,
        FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, .. }) if error == expected),
        "{source}: {failure}"
    );
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
fn wholly_undefined_objective_fields_refuse_admission() {
    for fields in ["1/X@0,X", "1@1/X,X", "1@0,1/X"] {
        refused(
            &format!("d(0). #minimize{{{fields}:d(X)}}."),
            &EvaluationError::Undefined,
        );
    }
}

#[test]
fn objective_fields_need_one_jointly_defined_instance() {
    refused(
        "d(0..1). #minimize{1/X@1/(1-X),X:d(X)}.",
        &EvaluationError::Undefined,
    );
}

#[test]
fn pooled_objective_fragments_share_the_original_family() {
    for objective in [
        "#minimize{1/(X;X+1)@0,X:d(X)}.",
        "#minimize{1/(X+1;X)@0,X:d(X)}.",
        ":~d(X).[1/(X;X+1)@0,X]",
    ] {
        let source = format!("d(0). {objective}");
        let admitted = admit(&source, &FormulaLimits::default()).unwrap();
        assert_eq!(admitted.warnings().len(), 1, "{source}");
        assert_eq!(
            admitted.objectives().templates().len(),
            1,
            "one defined pooled alternative: {source}"
        );
        let template = admitted.objectives().templates().at(0).unwrap();
        assert_eq!(
            template.weight(),
            zetesis_core::TemplateTerm::Constant((&Value::Number(1)).into()),
            "{source}"
        );
    }
}

#[test]
fn distinct_objective_elements_cannot_rescue_each_other() {
    for elements in [
        "1/X@0,a:d(X);1/(X+1)@0,b:d(X)",
        "1/(X+1)@0,b:d(X);1/X@0,a:d(X)",
    ] {
        refused(
            &format!("d(0). #minimize{{{elements}}}."),
            &EvaluationError::Undefined,
        );
    }
}

#[test]
fn undefined_objective_fields_cannot_witness_false_rows() {
    // X=0 reaches undefined condition arithmetic. X=1 has a defined false
    // condition but undefined weight, so it is excluded, not a defined witness.
    refused(
        "d(0..1). #minimize{1/(1-X)@0,X:d(X),1/X=0}.",
        &EvaluationError::Undefined,
    );
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
