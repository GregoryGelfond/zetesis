//! Undefined generated values cannot conceal independent arithmetic faults.

use zetesis_core::Value;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, FormulaFailure,
    FormulaLimits, admit_formula, observation::EvaluationError,
};

fn admit(source: &str) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
}

fn overflow(source: &str) {
    let Err(failure) = admit(source) else {
        panic!("independent overflow must refuse {source}");
    };
    assert!(
        matches!(
            failure,
            FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                error: EvaluationError::Overflow,
                ..
            })
        ),
        "{source}: {failure}"
    );
}

#[test]
fn independent_body_generators_preserve_fatal_errors() {
    for body in ["Y=1/X,Z=2147483647+(1-X)", "Z=2147483647+(1-X),Y=1/X"] {
        overflow(&format!("d(0;1).p(X,Y,Z):-d(X),{body}."));
    }
}

#[test]
fn independent_head_arguments_preserve_fatal_errors() {
    for arguments in ["1/X,2147483647+(1-X)", "2147483647+(1-X),1/X"] {
        overflow(&format!("d(0;1).p({arguments}):-d(X)."));
    }
}

#[test]
fn independent_range_alternatives_preserve_fatal_errors() {
    overflow("d(0;1).p(Y,Z,R):-d(X),Y=1/X,R=0..1,Z=2147483647+(R-X).");
}

#[test]
fn dependent_expressions_still_check_independent_branches() {
    overflow("d(0;1).p(Z):-d(X),Y=-1/X,Z=Y+(2147483647+(1-X)).");
}

#[test]
fn dependent_range_bounds_still_check_independent_branches() {
    overflow("d(0;1).p(Z):-d(X),Y=1/X,Z=Y..(2147483647+(1-X)).");
}

#[test]
fn missing_generator_values_do_not_become_inputs() {
    let admitted = admit("d(0;1).p(Y,Z):-d(X),Y=1/X,Z=Y+2147483646.").unwrap();
    assert_eq!(admitted.warnings().len(), 1);
    let produced: Vec<_> = admitted
        .atoms()
        .iter()
        .filter(|atom| atom.predicate().name() == "p")
        .map(|atom| atom.values().iter().collect::<Vec<_>>())
        .collect();
    assert_eq!(
        produced,
        vec![&[Value::Number(1), Value::Number(i32::MAX)][..]]
    );
}

#[test]
fn empty_generators_leave_no_arithmetic_family() {
    for body in ["Y=1/X,R=(X+1)..X", "R=(X+1)..X,Y=1/X"] {
        let admitted = admit(&format!("d(0).p(Y,R):-d(X),{body}.")).unwrap();
        assert!(admitted.warnings().is_empty());
        assert!(
            admitted
                .atoms()
                .iter()
                .all(|atom| atom.predicate().name() != "p")
        );
    }
}

#[test]
fn backtracking_discards_unavailable_outputs() {
    let admitted = admit("p(X,Y,Z):-X=0..2,Y=1/X,Z=1/(X-1).").unwrap();
    assert_eq!(admitted.warnings().len(), 1);
    assert_eq!(admitted.atoms(), admit("p(2,0,1).").unwrap().atoms());
}

#[test]
fn constructors_do_not_publish_unavailable_arguments() {
    let admitted = admit("p(X,Z):-X=0..1,Y=1/X,Z=g(c,Y).").unwrap();
    assert_eq!(admitted.warnings().len(), 1);
    assert_eq!(admitted.atoms(), admit("p(1,g(c,1)).").unwrap().atoms());
}
