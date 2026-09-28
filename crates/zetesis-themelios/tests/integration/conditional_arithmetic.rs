//! Omitted arithmetic instances differ from false or empty consequents.

use crate::support::stable_models;

use std::collections::BTreeSet;

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

fn check(source: &str, warnings: usize, expected: &[&str]) {
    let admitted = admit(source).unwrap_or_else(|error| panic!("{source}: {error}"));
    assert_eq!(admitted.warnings().len(), warnings, "{source}");
    assert!(
        admitted.atoms().len() <= 12,
        "bounded complete model family"
    );
    let mut facts = String::new();
    for atom in expected {
        facts.push_str(atom);
        facts.push('.');
    }
    let expected = admit(&facts).unwrap();
    assert_eq!(
        stable_models::stable(&admitted),
        BTreeSet::from([expected
            .atoms()
            .iter()
            .map(|atom| atom.to_atom(zetesis_core::ValueLimits::default()).unwrap())
            .collect()]),
        "{source}",
    );
}

#[test]
fn omitted_atom_instances_do_not_make_conditionals_false() {
    check(
        "d(0;1).q(1).p:-q(1/X):d(X).",
        1,
        &["d(0)", "d(1)", "q(1)", "p"],
    );
    check("d(0;1).p:-not q(1/X):d(X).", 1, &["d(0)", "d(1)", "p"]);
    check(
        "d(0;1).q(1).p:-not not q(1/X):d(X).",
        1,
        &["d(0)", "d(1)", "q(1)", "p"],
    );
}

#[test]
fn defined_false_atom_consequents_remain_false() {
    check("d(0;1).p:-q(1/X):d(X).", 1, &["d(0)", "d(1)"]);
    check(
        "d(0;1).q(1).p:-not q(1/X):d(X).",
        1,
        &["d(0)", "d(1)", "q(1)"],
    );
    check("d(0;1).p:-not not q(1/X):d(X).", 1, &["d(0)", "d(1)"]);
}

#[test]
fn defined_false_guard_consequents_remain_false() {
    for consequent in ["1/X=0", "1/(X;2*X)=2"] {
        check(
            &format!("d(0;1).p:-{consequent}:d(X)."),
            1,
            &["d(0)", "d(1)"],
        );
    }
    check("d(0;1).p:-X=0:d(X).", 0, &["d(0)", "d(1)"]);
    check("d(0;1).p:-1/(X;2*X)>=0:d(X).", 1, &["d(0)", "d(1)", "p"]);
}

#[test]
fn consequent_pools_keep_their_defined_alternatives() {
    check("d(0).q(1).p:-q(1/X;1):d(X).", 1, &["d(0)", "q(1)", "p"]);
    check("d(0).p:-not q(1/X;1):d(X).", 1, &["d(0)", "p"]);
    check(
        "d(0).q(1).p:-not not q(1/X;1):d(X).",
        1,
        &["d(0)", "q(1)", "p"],
    );
    check("d(0).p:-q(1/X;1):d(X).", 1, &["d(0)"]);
}

#[test]
fn empty_conditions_and_empty_positive_witnesses_differ() {
    check("p:-q(Y):d(X).", 0, &["p"]);
    check("d(0;1).p:-q(Y):d(X).", 0, &["d(0)", "d(1)"]);
    check("p:-q(2..1):#true.", 0, &[]);
}

#[test]
fn empty_projection_keeps_its_default_negation() {
    check("d(0;1).p:-not q(_,X):d(X).", 0, &["d(0)", "d(1)", "p"]);
    check("d(0;1).p:-not not q(_,X):d(X).", 0, &["d(0)", "d(1)"]);
}

#[test]
fn projected_consequents_omit_only_undefined_instances() {
    check("d(0;1).p:-not q(_,1/X):d(X).", 1, &["d(0)", "d(1)", "p"]);
    check(
        "d(0;1).q(a,1).p:-not not q(_,1/X):d(X).",
        1,
        &["d(0)", "d(1)", "q(a,1)", "p"],
    );
    check(
        "d(0;1).q(f(a,1)).p:-not not q(f(_,1/X)):d(X).",
        1,
        &["d(0)", "d(1)", "q(f(a,1))", "p"],
    );
}

#[test]
fn wholly_undefined_consequents_still_refuse() {
    for consequent in ["q(1/X)", "not q(1/X)", "not not q(1/X)", "1/X=0"] {
        let source = format!("d(0).p:-{consequent}:d(X).");
        let failure = admit(&source).unwrap_err();
        assert!(
            matches!(
                failure,
                FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                    error: EvaluationError::Undefined,
                    ..
                })
            ),
            "{source}: {failure}",
        );
    }
}
