//! A consequent that divides by zero is false under its condition.
//!
//! A conditional literal keeps one implication for each binding of its
//! condition. Its consequent there is the disjunction of the values its
//! alternatives take, and an alternative that divides by zero takes none. A
//! consequent with no defined alternative is therefore false, whatever its
//! sign, and its implication requires the condition to be false. A condition
//! that divides by zero has no binding at all.

use crate::support::finite_bindings as reference;
use crate::support::stable_models;

use reference::{expected, external, native};
use std::collections::BTreeSet;

use zetesis_clingo_support as oracle;
use zetesis_reference_support::{admit, formula};
use zetesis_themelios::{
    ExpansionFailure, FormulaFailure, FormulaLimits, observation::EvaluationError,
};

fn check(source: &str, warnings: usize, expected: &[&str]) {
    let admitted = admit(source, &FormulaLimits::default())
        .unwrap_or_else(|error| panic!("{source}: {error}"));
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
    let expected = admit(&facts, &FormulaLimits::default()).unwrap();
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

/// Sources whose conditional literal meets a zero divisor, with their answer
/// sets. Each warns once.
const UNDEFINED: &[(&str, &[&[&str]])] = &[
    // No alternative is defined at `X = 0`, so `p` needs `d(0)` false.
    ("d(0;1).q(1).p:-q(1/X):d(X).", &[&["d(0)", "d(1)", "q(1)"]]),
    ("d(0;1).p:-not q(1/X):d(X).", &[&["d(0)", "d(1)"]]),
    (
        "d(0;1).q(1).p:-not not q(1/X):d(X).",
        &[&["d(0)", "d(1)", "q(1)"]],
    ),
    (
        "{d(0)}.d(1).q(1).p:-q(1/X):d(X).",
        &[&["d(0)", "d(1)", "q(1)"], &["d(1)", "p", "q(1)"]],
    ),
    (
        "{d(0)}.d(1).{q(1)}.p:-not q(1/X):d(X).",
        &[
            &["d(0)", "d(1)"],
            &["d(0)", "d(1)", "q(1)"],
            &["d(1)", "p"],
            &["d(1)", "q(1)"],
        ],
    ),
    // A comparison, alone and with pooled alternatives.
    ("d(0;1).p:-1/X=0:d(X).", &[&["d(0)", "d(1)"]]),
    ("d(0;1).p:-1/(X;2*X)=2:d(X).", &[&["d(0)", "d(1)"]]),
    ("d(0;1).p:-1/(X;2*X)>=0:d(X).", &[&["d(0)", "d(1)"]]),
    (
        "{d(0)}.d(1).p:-1=1/X:d(X).",
        &[&["d(0)", "d(1)"], &["d(1)", "p"]],
    ),
    (
        "{d(0)}.d(1).p:-1/(X;2*X)>=0:d(X).",
        &[&["d(0)", "d(1)"], &["d(1)", "p"]],
    ),
    // A defined alternative beside an undefined one decides the consequent.
    ("d(0).q(1).p:-q(1/X;1):d(X).", &[&["d(0)", "p", "q(1)"]]),
    ("d(0).p:-not q(1/X;1):d(X).", &[&["d(0)", "p"]]),
    (
        "d(0).q(1).p:-not not q(1/X;1):d(X).",
        &[&["d(0)", "p", "q(1)"]],
    ),
    ("d(0).p:-q(1/X;1):d(X).", &[&["d(0)"]]),
    // A projected witness whose argument is undefined.
    ("d(0;1).p:-not q(_,1/X):d(X).", &[&["d(0)", "d(1)"]]),
    (
        "{d(0)}.d(1).p:-not q(_,1/X):d(X).",
        &[&["d(0)", "d(1)"], &["d(1)", "p"]],
    ),
    (
        "d(0;1).q(a,1).p:-not not q(_,1/X):d(X).",
        &[&["d(0)", "d(1)", "q(a,1)"]],
    ),
    (
        "{d(0)}.d(1).q(a,1).p:-not not q(_,1/X):d(X).",
        &[&["d(0)", "d(1)", "q(a,1)"], &["d(1)", "p", "q(a,1)"]],
    ),
    (
        "{d(0)}.d(1).q(f(a,1)).p:-not not q(f(_,1/X)):d(X).",
        &[&["d(0)", "d(1)", "q(f(a,1))"], &["d(1)", "p", "q(f(a,1))"]],
    ),
    // Every instance of the family is undefined: the source is admitted.
    ("d(0).p:-q(1/X):d(X).", &[&["d(0)"]]),
    ("d(0).p:-not q(1/X):d(X).", &[&["d(0)"]]),
    ("d(0).p:-not not q(1/X):d(X).", &[&["d(0)"]]),
    ("d(0).p:-1/X=0:d(X).", &[&["d(0)"]]),
    // A constraint's body is false under `d(0)` and excludes nothing.
    ("{d(0)}.q(1).:-q(1/X):d(X).", &[&["d(0)", "q(1)"]]),
    // A condition that divides by zero has no binding.
    (
        "d(0;1).q(1).p:-q(X):d(X),1/X=1.",
        &[&["d(0)", "d(1)", "p", "q(1)"]],
    ),
    // An outer binding in the consequent.
    (
        "{d(0;1)}.e(2).{q(2)}.p(Z):-e(Z),q(Z/X):d(X).",
        &[
            &["d(0)", "d(1)", "e(2)"],
            &["d(0)", "d(1)", "e(2)", "q(2)"],
            &["d(0)", "e(2)"],
            &["d(0)", "e(2)", "q(2)"],
            &["d(1)", "e(2)"],
            &["d(1)", "e(2)", "p(2)", "q(2)"],
            &["e(2)", "p(2)"],
            &["e(2)", "p(2)", "q(2)"],
        ],
    ),
];

#[test]
fn an_undefined_consequent_is_false_under_its_condition() {
    for &(source, records) in UNDEFINED {
        assert_eq!(native(&formula(source)), expected(records), "{source}");
    }
}

#[test]
fn an_undefined_consequent_warns_once() {
    for &(source, _) in UNDEFINED {
        assert_eq!(formula(source).warnings().len(), 1, "{source}");
    }
}

#[test]
fn undefined_consequents_keep_membership_by_subset_enumeration() {
    reference::stability_matches_exhaustion(UNDEFINED.iter().map(|&(source, _)| source), formula);
}

#[test]
#[ignore = "requires clingo: undefined consequents match clingo full models"]
fn undefined_consequents_match_clingo_full_models() {
    let mut models = 0;
    for &(source, records) in UNDEFINED {
        let actual = oracle::objective_free_models(&external(source, true));
        assert_eq!(actual, expected(records), "{source}");
        assert_eq!(native(&formula(source)), actual, "{source}");
        models += actual.len();
    }
    println!("complete_sources={} full_models={models}", UNDEFINED.len());
}

#[test]
fn a_fatal_failure_in_a_consequent_still_refuses() {
    // An overflow is no zero divisor. It refuses beside one in the same
    // comparison, beside one at another binding, and in an atom's argument.
    for source in [
        "d(0).p:-1/X=X+2147483647+1:d(X).",
        "d(0;1).p:-1/X=2147483647+X:d(X).",
        "d(0;1).q(1).p:-q(2147483647+X):d(X).",
    ] {
        let failure = admit(source, &FormulaLimits::default()).unwrap_err();
        assert!(
            matches!(
                failure,
                FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                    error: EvaluationError::Overflow,
                    ..
                })
            ),
            "{source}: {failure}",
        );
    }
}

#[test]
fn an_excluded_binding_reaches_no_consequent() {
    // `X != 0` excludes the binding whose consequent would divide by zero.
    check("d(0;1).p:-1/X=1:d(X),X!=0.", 0, &["d(0)", "d(1)", "p"]);
    check(
        "d(0;1).q(1).p:-q(1/X):d(X),X!=0.",
        0,
        &["d(0)", "d(1)", "q(1)", "p"],
    );
}

#[test]
fn a_binding_that_fails_its_condition_reaches_no_consequent() {
    // A tuple comparison excludes nothing, but it is part of the condition.
    // The binding that fails it has no implication, and its consequent is
    // not evaluated: a zero divisor there leaves no warning, and an overflow
    // there does not refuse.
    check(
        "d(0;1).p:-1/X=1:d(X),(X,1)!=(0,1).",
        0,
        &["d(0)", "d(1)", "p"],
    );
    check(
        "d(0;1).p:-2147483647+X=1:d(X),(X,1)!=(1,1).",
        0,
        &["d(0)", "d(1)"],
    );
}

#[test]
fn a_condition_that_divides_by_zero_reaches_no_consequent() {
    // The condition has no binding, so the literal holds, and the overflow
    // in its consequent is not reached. The statement warns for the
    // condition's omitted instance.
    check("d(0).p:-X+2147483647+1=1:d(X),1/X>0.", 1, &["d(0)", "p"]);
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
fn a_defined_comparison_consequent_keeps_its_value() {
    check("d(0;1).p:-X=0:d(X).", 0, &["d(0)", "d(1)"]);
    check("d(1;2).p:-2/X>=1:d(X).", 0, &["d(1)", "d(2)", "p"]);
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
