//! The analysis keeps exactly the predicates every rule of which is definite
//! over kept predicates, and lists the assignment elements they make required.

use crate::formula_support::certain::Certainty;
use crate::formula_support::producers::Producers;
use crate::formula_support::{Counters, testing::prepare};
use crate::{FormulaFailure, FormulaLimits};

/// The certainty of `source`, through the producers its completion prepares.
fn certainty(source: &str) -> Certainty {
    crate::formula_support::testing::certainty(source)
        .expect("the source's predicates resolve through a component view")
}

/// The certain predicates of `source`, as `name/arity`, in their order. A
/// strongly negated predicate, which follows the positive ones, is `-name/arity`.
fn certain(source: &str) -> Vec<String> {
    let certainty = certainty(source);
    assert!(!certainty.is_all(), "the source is not positive and flat");
    certainty
        .signed_predicates()
        .map(|(sign, name, arity)| match sign {
            zetesis_core::Sign::Positive => format!("{name}/{arity}"),
            zetesis_core::Sign::Negative => format!("-{name}/{arity}"),
        })
        .collect()
}

/// A choice keeps a source out of the positive flat plan without reading or
/// deriving any predicate the tests name.
const CHOICE: &str = "{free}.";

#[test]
fn facts_and_definite_rules_over_them_are_certain() {
    assert_eq!(
        certain(&format!(
            "c(1..3). d(X) :- c(X). e(X, Y) :- d(X), c(Y), X < Y. {CHOICE} b :- free."
        )),
        ["c/1", "d/1", "e/2"]
    );
}

#[test]
fn a_recursive_definite_predicate_is_certain() {
    assert_eq!(
        certain(&format!(
            "e(1, 2). e(2, 3). p(X, Y) :- e(X, Y). p(X, Z) :- p(X, Y), e(Y, Z). {CHOICE}"
        )),
        ["e/2", "p/2"]
    );
}

#[test]
fn a_default_negation_removes_the_head_it_guards() {
    assert_eq!(
        certain(&format!(
            "c(1). e(1) :- c(1). d(1) :- c(1), not e(1). {CHOICE}"
        )),
        ["c/1", "e/1"]
    );
}

#[test]
fn a_double_negation_removes_the_head_it_guards() {
    assert_eq!(
        certain(&format!("c(1). d(1) :- c(1), not not c(1). {CHOICE}")),
        ["c/1"]
    );
}

#[test]
fn a_choice_removes_the_predicate_it_derives() {
    assert_eq!(certain("c(1..2). {p(X) : c(X)}. q(X) :- p(X)."), ["c/1"]);
}

#[test]
fn a_fact_does_not_keep_a_predicate_a_choice_also_derives() {
    assert_eq!(certain("c(1..2). p(1). {p(X)} :- c(X)."), ["c/1"]);
}

#[test]
fn a_disjunction_removes_every_predicate_it_derives() {
    assert_eq!(
        certain("c(1). p(X) | q(X) :- c(X). r(X) :- c(X)."),
        ["c/1", "r/1"]
    );
}

#[test]
fn a_conditional_head_removes_the_predicate_it_derives() {
    assert_eq!(
        certain(&format!("c(1..2). d(X) : c(X). e(X) :- c(X). {CHOICE}")),
        ["c/1", "e/1"]
    );
}

#[test]
fn a_bounded_choice_removes_the_predicate_it_derives() {
    assert_eq!(
        certain("c(1..2). 1 { d(X) : c(X) } 1. e(X) :- c(X)."),
        ["c/1", "e/1"]
    );
}

#[test]
fn a_negated_projection_removes_the_head_it_guards() {
    // `not c(_)` reads the projection of `c` under default negation.
    assert_eq!(
        certain(&format!("c(1..2). e :- not c(_). d(X) :- c(X). {CHOICE}")),
        ["c/1", "d/1"]
    );
}

#[test]
fn an_anonymous_positive_argument_is_an_ordinary_atom() {
    assert_eq!(
        certain(&format!("c(1..2). e :- c(_). {CHOICE}")),
        ["c/1", "e/0"]
    );
}

#[test]
fn a_predicate_and_its_strong_negation_are_classified_apart() {
    // `-d` is derived by a definite rule; `d` is chosen.
    assert_eq!(certain("c(1..2). -d(X) :- c(X). {d(3)}."), ["c/1", "-d/1"]);
}

#[test]
fn predicates_of_one_name_and_different_arity_are_classified_apart() {
    assert_eq!(certain("c(1..2). p(X) :- c(X). {p(1, 2)}."), ["c/1", "p/1"]);
}

#[test]
fn a_predicate_reading_a_removed_one_is_removed_in_turn() {
    assert_eq!(
        certain("c(1). {p(1)}. q(X) :- p(X). r(X) :- q(X), c(X). s(X) :- r(X). t(X) :- c(X)."),
        ["c/1", "t/1"]
    );
}

#[test]
fn one_indefinite_rule_removes_a_predicate_with_definite_ones() {
    assert_eq!(
        certain(&format!(
            "c(1..2). d(X) :- c(X). d(X) :- c(X), not free. {CHOICE}"
        )),
        ["c/1"]
    );
}

#[test]
fn a_cycle_through_a_removed_predicate_is_removed_whole() {
    assert_eq!(
        certain("c(1). {x(1)}. p(X) :- q(X). q(X) :- p(X). q(X) :- x(X). r(X) :- c(X)."),
        ["c/1", "r/1"]
    );
}

#[test]
fn a_positive_flat_source_makes_every_predicate_certain() {
    let certainty = certainty("e(1, 2). e(2, 3). p(X, Y) :- e(X, Y). p(X, Z) :- p(X, Y), e(Y, Z).");
    assert!(certainty.is_all());
    assert!(certainty.elements().is_empty());
}

/// A count over the certain `c`, read by a rule.
fn counted() -> String {
    format!("c(1..3). n(N) :- N = #count {{ X : c(X) }}. m(N) :- n(N). {CHOICE}")
}

#[test]
fn a_count_over_certain_atoms_keeps_its_head_certain() {
    assert_eq!(certain(&counted()), ["c/1", "m/1", "n/1"]);
}

#[test]
fn a_count_over_certain_atoms_lists_its_element() {
    assert_eq!(certainty(&counted()).elements(), [(0, 0)]);
}

/// A count over the chosen `c`.
const COUNTED_OVER_A_CHOICE: &str = "d(1..3). {c(X) : d(X)}. n(N) :- N = #count { X : c(X) }.";

#[test]
fn a_count_over_an_uncertain_atom_removes_its_head() {
    assert_eq!(certain(COUNTED_OVER_A_CHOICE), ["d/1"]);
}

#[test]
fn a_count_over_an_uncertain_atom_lists_nothing() {
    assert!(certainty(COUNTED_OVER_A_CHOICE).elements().is_empty());
}

/// A plain count with one element over the certain `d` and one over the
/// chosen `c`.
const COUNTED_OVER_BOTH: &str =
    "d(1..3). {c(X) : d(X)}. n(N) :- N = #count { X : d(X); X, 1 : c(X) }.";

#[test]
fn an_assignment_with_an_uncertain_element_is_not_exact() {
    // It reads `c`, so its head is removed.
    assert_eq!(certain(COUNTED_OVER_BOTH), ["d/1"]);
}

#[test]
fn a_certain_element_is_listed_beside_an_uncertain_one() {
    // Its tuples are required whatever the other element yields.
    assert_eq!(certainty(COUNTED_OVER_BOTH).elements(), [(0, 0)]);
}

/// A count read through a guard, which binds nothing.
fn guarded() -> String {
    format!("c(1..3). big :- #count {{ X : c(X) }} >= 2. {CHOICE}")
}

#[test]
fn a_guard_aggregate_removes_its_head() {
    assert_eq!(certain(&guarded()), ["c/1"]);
}

#[test]
fn a_guard_aggregate_lists_nothing() {
    assert!(certainty(&guarded()).elements().is_empty());
}

#[test]
fn a_constraint_lists_the_certain_elements_of_its_assignment() {
    // A constraint proposes no head and is grounded after completion, so its
    // count reads the complete `c`.
    let certainty = certainty(&format!(
        "c(1..3). :- N = #count {{ X : c(X) }}, N > 5. {CHOICE}"
    ));
    assert_eq!(certainty.elements(), [(0, 0)]);
}

#[test]
fn an_aggregate_reading_its_own_component_is_not_exact() {
    // The producer reads the predicate it derives, so the schedule cannot
    // traverse it once over complete relations.
    let certainty = certainty(&format!(
        "c(1..3). n(0). n(N) :- n(M), M < 2, N = #count {{ X : c(X), n(X) }}. {CHOICE}"
    ));
    let predicates: Vec<_> = certainty.predicates().collect();
    assert_eq!(predicates, [("c", 1)]);
    assert!(certainty.elements().is_empty());
}

#[test]
fn a_producer_traversed_every_round_lists_no_element() {
    // The count's condition holds certain atoms only, but its rule reads the
    // predicate it derives, so the schedule traverses it in every round and
    // it proposes from relations that may still grow.
    let certainty = certainty(&format!(
        "c(1..3). n(0). n(N) :- n(M), M < 2, N = #count {{ X : c(X) }}. {CHOICE}"
    ));
    let predicates: Vec<_> = certainty.predicates().collect();
    assert_eq!(predicates, [("c", 1)]);
    assert!(certainty.elements().is_empty());
}

#[test]
fn the_analysis_refuses_every_insufficient_work_prefix() {
    let source = format!("c(1..3). d(X) :- c(X). n(N) :- N = #count {{ X : d(X) }}. {CHOICE}");
    let preparation = prepare(&source);
    let defaults = FormulaLimits::default();
    // The producers are prepared under the default allowance; only the
    // analysis runs under the swept one.
    let producers = Producers::prepare(
        &preparation.program,
        &preparation.catalog,
        &defaults,
        &mut Counters::default(),
        preparation.location,
    )
    .unwrap();
    let run = |maximum| {
        let limits = FormulaLimits {
            max_work: maximum,
            ..defaults
        };
        let mut counters = Counters::default();
        let result = producers.certainty(
            &preparation.program,
            &preparation.catalog,
            &limits,
            &mut counters,
            preparation.location,
        );
        (result, counters.accounting.work)
    };
    let (complete, required) = run(defaults.max_work);
    assert!(complete.unwrap().is_some());
    assert!(required > 0);
    for maximum in 0..required {
        let (result, work) = run(maximum);
        assert!(matches!(result, Err(FormulaFailure::Limit {
            resource: crate::FormulaResource::Work, observed, limit, ..
        }) if observed > limit && limit == u128::from(maximum)));
        assert!(work <= maximum);
    }
    assert!(run(required).0.is_ok());
}
