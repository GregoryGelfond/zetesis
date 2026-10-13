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
    // #true, a comparison chain and a negated comparison lower to guards,
    // which the element's local join decides before it yields a row.
    for (source, expected) in [
        ("q(M) :- M = #count{1 : #true}.", "q(1)"),
        ("q(M) :- M = #count{X : X = 1..3, 1 < X < 3}.", "q(1)"),
        ("q(M) :- M = #count{X : X = 1..3, not X > 2}.", "q(2)"),
    ] {
        assert_eq!(possible(&formula(source)), atoms(&[expected]), "{source}");
    }
}

#[test]
fn a_key_with_one_required_occurrence_is_required() {
    // Tuple 1 occurs under p and unconditionally: one required occurrence
    // makes the key required at both proposal sites, and final grounding finds
    // a root for every proposal.
    let admitted = formula("{p}. q(M) :- M = #count{1 : p; 1}.");
    assert_eq!(possible(&admitted), atoms(&["p", "q(1)"]));
    assert_eq!(native(&admitted), exhaustive(&admitted));
}

#[test]
fn a_sum_omits_a_value_only_an_interpretation_outside_the_support_gives() {
    // p has no rule, so the -1 element has no row: the carrier omits V = 0,
    // which only the interpretation {p}, outside the support, would give.
    let admitted = formula("x(V) :- V = #sum{1; -1 : p}.");
    assert_eq!(possible(&admitted), atoms(&["x(1)"]));
    let expected = Models::from([BTreeSet::from(["x(1)".into()])]);
    assert_eq!(native(&admitted), expected);
    assert_eq!(exhaustive(&admitted), expected);
}

#[test]
fn a_required_extremum_keeps_the_optional_values_before_it() {
    // The unconditional element is required. The value under a precedes the
    // required extremum in the function's order, so the carrier holds both and
    // each interpretation of a finds a root for its own value.
    for (source, with_a, without_a) in [
        ("{a}. n(N) :- N = #max{2 : a; 1}.", "n(2)", "n(1)"),
        ("{a}. n(N) :- N = #min{1 : a; 2}.", "n(1)", "n(2)"),
    ] {
        let admitted = formula(source);
        assert_eq!(
            possible(&admitted),
            atoms(&["a", "n(1)", "n(2)"]),
            "{source}"
        );
        let expected = Models::from([atoms(&["a", with_a]), atoms(&[without_a])]);
        assert_eq!(native(&admitted), expected, "{source}");
        assert_eq!(exhaustive(&admitted), expected, "{source}");
    }
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
fn nested_mastermind_possible_atoms_follow_the_carrier() {
    // shared/3 is exact per binding, because the two elements of its #min are
    // unconditional. score/3 pairs every carrier value of B with every carrier
    // value of H, so it holds values no answer set realizes (for example B > H).
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
