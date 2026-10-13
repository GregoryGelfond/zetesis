//! An assignment proposes the measures of the selections between its required
//! tuples, those from elements whose conditions hold for every binding, and
//! its possible ones; the original aggregate equality still decides truth.

use crate::support::finite_bindings as reference;
use zetesis_reference_support::{canonical, formula};

use reference::{Models, exhaustive, native};
use std::collections::BTreeSet;
use zetesis_themelios::{AdmittedFormula, FormulaWarning};

/// The spellings of `admitted`'s possible atoms.
fn possible(admitted: &AdmittedFormula) -> BTreeSet<String> {
    admitted.atoms().iter().map(canonical).collect()
}

fn atoms(spellings: &[&str]) -> BTreeSet<String> {
    spellings
        .iter()
        .map(|&spelling| spelling.to_owned())
        .collect()
}

#[test]
fn a_comparison_only_condition_is_required() {
    let admitted = formula("q(M) :- M = #count{X : X = 1..3, X > 1}.");
    assert_eq!(possible(&admitted), atoms(&["q(2)"]));
}

#[test]
fn a_guard_only_condition_is_required() {
    // A comparison guard and #true are decided by the element's local join.
    let admitted = formula("q(M) :- M = #count{1 : #true; 2 : 1 < 2}.");
    assert_eq!(possible(&admitted), atoms(&["q(2)"]));
}

#[test]
fn an_atom_of_either_sign_makes_a_condition_optional() {
    for source in [
        "{p}. q(M) :- M = #count{1 : p}.",
        "{p}. q(M) :- M = #count{1 : not p}.",
    ] {
        let admitted = formula(source);
        assert_eq!(
            possible(&admitted),
            atoms(&["p", "q(0)", "q(1)"]),
            "{source}"
        );
    }
}

#[test]
fn a_projected_negation_over_an_empty_relation_keeps_both_sites_agreed() {
    // The condition lowers to true at final grounding, but its IR literal is
    // an atom, so both proposal sites treat it as optional and every proposal
    // finds its root.
    let admitted = formula("q(M) :- M = #count{1 : not r(_)}.");
    assert_eq!(possible(&admitted), atoms(&["q(0)", "q(1)"]));
    let expected = Models::from([BTreeSet::from(["q(1)".into()])]);
    assert_eq!(native(&admitted), expected);
    assert_eq!(exhaustive(&admitted), expected);
}

#[test]
fn an_optional_only_overflow_admits_a_carrier_that_fits() {
    // The optional weights alone exceed i32; every carrier value fits.
    let admitted = formula("{p; q}. x(V) :- V = #sum{-10,a; 2147483647,b : p; 1,c : q}.");
    let expected = Models::from([
        BTreeSet::from(["x(-10)".into()]),
        BTreeSet::from(["q".into(), "x(-9)".into()]),
        BTreeSet::from(["p".into(), "x(2147483637)".into()]),
        BTreeSet::from(["p".into(), "q".into(), "x(2147483638)".into()]),
    ]);
    assert_eq!(native(&admitted), expected);
    assert_eq!(exhaustive(&admitted), expected);
}

#[test]
fn a_tuple_in_every_interpretation_without_an_atom_free_condition_stays_optional() {
    // Tuple 1 is present through p or through not p, but neither condition is
    // atom-free, so the family keeps N = 0, which divides by zero.
    let admitted = formula("{p}. q(Y) :- N = #count{1 : p; 1 : not p}, Y = 1/(N-1).");
    assert!(
        matches!(admitted.warnings(), [FormulaWarning::ZeroDivisor { .. }]),
        "{:?}",
        admitted.warnings()
    );
}

#[test]
fn nested_mastermind_proposes_only_realizable_values() {
    // Each minimum of two unconditional counts is exact, so shared/3 holds one
    // atom per realizable minimum and score/3 one per realizable total.
    let admitted = formula(include_str!(
        "../../../../examples/scalability/mastermind-nested.lp"
    ));
    let count = |name: &str| {
        possible(&admitted)
            .iter()
            .filter(|atom| atom.starts_with(name))
            .count()
    };
    assert_eq!((count("shared("), count("score(")), (74, 427));
}
