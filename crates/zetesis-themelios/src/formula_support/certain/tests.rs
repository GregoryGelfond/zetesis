//! Certainty changes what is emitted and what an assignment proposes, never
//! which interpretations are answer sets.

use std::collections::BTreeSet;

use super::testing::{with_every_predicate_assumed, without_certainty, without_decided_negation};
use crate::formula::Compiled;
use crate::formula_ground::ground;
use crate::formula_support::testing;
use crate::test_support::{compiled_atoms, zero_divisor_statements};
use zetesis_algebra::Cancellation;
use zetesis_core::{Sign, Value};
use zetesis_ferraris::{Interpretation, Limits, models, models_reduct};

fn compile(source: &str) -> Compiled {
    ground(testing::prepare(source), None, None).unwrap()
}

/// `source` grounded with certainty and without it, in that order. Either way
/// it must be admitted with one zero-divisor warning, for `statement`, and
/// have exactly the answer sets `expected`. Certainty changes neither; it
/// changes which instances a family has, which the callers compare.
fn omitting(source: &str, statement: &str, expected: &[&str]) -> (Compiled, Compiled) {
    let compiled = compile(source);
    let written = without_certainty(|| compile(source));
    for grounded in [&compiled, &written] {
        assert_eq!(
            zero_divisor_statements(source, &grounded.warnings),
            [statement],
            "{source}"
        );
        assert_eq!(answer_sets(grounded), sets(expected), "{source}");
    }
    (compiled, written)
}

/// The answer sets of `source` grounded without certainty, as it was before
/// certain predicates narrowed any family.
fn written_answer_sets(source: &str) -> BTreeSet<BTreeSet<String>> {
    answer_sets(&without_certainty(|| ground(testing::prepare(source), None, None)).unwrap())
}

/// Sets of atoms, each written as its space-separated atoms.
fn sets(rows: &[&str]) -> BTreeSet<BTreeSet<String>> {
    rows.iter()
        .map(|row| row.split_whitespace().map(str::to_owned).collect())
        .collect()
}

/// `source` grounded on the hybrid schedule with a share that lets its
/// constraints be grounded into the core once every producer is, and the
/// constraints that stayed streamed.
fn retained(
    source: &str,
) -> Result<(Compiled, Option<crate::formula_hybrid::Constraints>), crate::FormulaFailure> {
    let mut preparation = testing::prepare(source);
    preparation.limits.max_retained_constraint_nodes = 1 << 20;
    crate::formula_ground::ground_hybrid(preparation, None)
}

/// The atoms of `compiled`, in catalog order: a sign, the predicate's name
/// and its arguments, numbers and constants as written.
fn names(compiled: &Compiled) -> Vec<String> {
    compiled_atoms(compiled)
        .iter()
        .map(|atom| {
            let arguments: Vec<String> = atom
                .values()
                .iter()
                .map(|value| match value {
                    Value::Number(number) => number.to_string(),
                    Value::Symbol(name) => name.clone(),
                    other => format!("{other:?}"),
                })
                .collect();
            let sign = match atom.predicate().sign() {
                Sign::Positive => "",
                Sign::Negative => "-",
            };
            let name = atom.predicate().name();
            if arguments.is_empty() {
                format!("{sign}{name}")
            } else {
                format!("{sign}{name}({})", arguments.join(","))
            }
        })
        .collect()
}

/// Every interpretation of `compiled`'s atoms, by the bits of its index.
fn interpretations(compiled: &Compiled, most: usize) -> Vec<Interpretation> {
    let atoms = compiled_atoms(compiled).len();
    assert!(
        atoms <= most,
        "bounded exhaustive population: {atoms} atoms"
    );
    (0..1_usize << atoms)
        .map(|bits| {
            Interpretation::new(
                &compiled.theory,
                (0..atoms).filter(|&atom| bits & (1 << atom) != 0),
            )
            .unwrap()
        })
        .collect()
}

/// The answer sets of `compiled`, by the definition: a model of the theory no
/// proper subset of which models its reduct. Each is the set of its atoms.
fn answer_sets(compiled: &Compiled) -> BTreeSet<BTreeSet<String>> {
    let cancellation = Cancellation::default();
    let names = names(compiled);
    let all = interpretations(compiled, 11);
    let mut found = BTreeSet::new();
    for (bits, candidate) in all.iter().enumerate() {
        if !models(
            &compiled.theory,
            candidate,
            Limits::default(),
            &cancellation,
        )
        .unwrap()
        {
            continue;
        }
        // Every proper subset of the candidate's atoms.
        let mut smaller = bits;
        let mut minimal = true;
        while smaller != 0 {
            smaller = (smaller - 1) & bits;
            if models_reduct(
                &compiled.theory,
                candidate,
                &all[smaller],
                Limits::default(),
                &cancellation,
            )
            .unwrap()
            {
                minimal = false;
                break;
            }
        }
        if minimal {
            found.insert(
                names
                    .iter()
                    .enumerate()
                    .filter(|&(atom, _)| bits & (1 << atom) != 0)
                    .map(|(_, name)| name.clone())
                    .collect(),
            );
        }
    }
    found
}

/// Sources whose certain atoms are read by bodies of every kind. Certainty
/// keeps their atoms, so truth is compared interpretation by interpretation.
const BODIES: &[&str] = &[
    "c. d :- c. {p}. q :- d, p.",
    "c(1;2). d(X) :- c(X). {p}. q :- d(1), p, not r. r :- not p.",
    "e(1,2). t(X,Y) :- e(X,Y). t(X,Y) :- t(X,Z), e(Z,Y). {p}. q :- t(1,2), p.",
    "c. d :- c. {p}. q | r :- d, p.",
    "c. d :- c. {p}. :- d, p.",
    "-c. -d :- -c. {p}. q :- -d, p.",
    "c. d :- c. {p}. q :- 2 <= #count { a : d; b : p }.",
    "c(f(1)). d(X) :- c(f(X)). {p}. q :- d(1), p.",
    "c(1;2). d(X) :- c(X), X > 1. {p}. q :- d(2), p. r :- d(1), p.",
    // A total body of two positive atoms is emitted through shared witnesses.
    "e(1,2). e(2,3). t(X,Z) :- e(X,Y), e(Y,Z). {p}. q :- t(1,3), p.",
    // A variable outside the head makes the body a factored product.
    "c(1;2). d(1;2). f(X) :- c(X), d(Y). {p}. q :- f(1), p.",
];

/// Sources that read a certain atom under default negation only. The
/// negation is decided, so certainty names fewer atoms and the answer sets
/// are compared.
const NEGATED: &[&str] = &[
    "c. d :- c. {p}. q :- not d, p.",
    "c. d :- c. {p}. q :- not not d, p.",
];

/// Sources that read a certain atom in a choice element's condition or a
/// conditional disjunct's, which are bodies and are read as true as well.
const CONDITIONS: &[&str] = &[
    "c(1;2). d(X) :- c(X). {p(X) : d(X)}.",
    "c(1;2). d(X) :- c(X). {q}. {p(X) : d(X)} :- q.",
    "c(1;2). d(X) :- c(X). {q}. p(X) : d(X) :- q.",
];

/// Every source whose atoms certainty keeps.
fn kept() -> impl Iterator<Item = &'static &'static str> {
    BODIES.iter().chain(CONDITIONS)
}

/// Sources whose assignment aggregates read certain atoms. Certainty supports
/// fewer of their heads, so their answer sets are compared.
const ASSIGNMENTS: &[&str] = &[
    "c(1..2). n(N) :- N = #count { X : c(X) }. {p}. q :- n(2), p.",
    "c(1..2). r(C,R) :- c(C), R = #count { D : c(D), D <= C }. {p}. q :- r(2,2), p.",
    "c(1..2). s(S) :- S = #sum { X : c(X) }. {p}. q :- s(3), p.",
    "c(1..2). m(M) :- M = #max { X : c(X) }. {p}. q :- m(2), p.",
    "c(1..2). m(M) :- M = #min { X : c(X) }. {p}. q :- m(1), p.",
    "c(1..2). n(N) :- N = #count { X : c(X) }. m(M) :- n(N), M = N + 1. {p}. q :- m(3), p.",
    "c(1). {p}. n(N) :- N = #count { a : c(1); b : p }. q :- n(2).",
    "c(1..2). d(X) :- c(X). n(N) :- N = #count { X : d(X) }. {p}. q :- n(2), p.",
    "c(1..2). n(N) :- N = #count { X : c(X) }. {p}. q :- not n(1), p.",
    "c(1..2). n(N) :- N = #count { X : c(X) }. {p(N)} :- n(N).",
    "c(1..2). {p}. :- p, N = #count { X : c(X) }, N > 2.",
    "c(1..2). {p}. :- p, N = #count { X : c(X) }, N < 3.",
];

#[test]
fn certain_body_atoms_preserve_original_truth() {
    let cancellation = Cancellation::default();
    for source in kept() {
        let actual = compile(source);
        let reference = without_certainty(|| compile(source));
        assert_eq!(names(&actual), names(&reference), "{source}");
        for (actual_i, reference_i) in interpretations(&actual, 8)
            .iter()
            .zip(interpretations(&reference, 8))
        {
            assert_eq!(
                models(&actual.theory, actual_i, Limits::default(), &cancellation).unwrap(),
                models(
                    &reference.theory,
                    &reference_i,
                    Limits::default(),
                    &cancellation
                )
                .unwrap(),
                "{source}",
            );
        }
    }
}

#[test]
fn certain_body_atoms_preserve_arbitrary_frozen_truth() {
    let cancellation = Cancellation::default();
    for source in kept() {
        let actual = compile(source);
        let reference = without_certainty(|| compile(source));
        let actual_i = interpretations(&actual, 8);
        let reference_i = interpretations(&reference, 8);
        for (candidate, actual_m) in actual_i.iter().enumerate() {
            for (tested, actual_j) in actual_i.iter().enumerate() {
                assert_eq!(
                    models_reduct(
                        &actual.theory,
                        actual_m,
                        actual_j,
                        Limits::default(),
                        &cancellation
                    )
                    .unwrap(),
                    models_reduct(
                        &reference.theory,
                        &reference_i[candidate],
                        &reference_i[tested],
                        Limits::default(),
                        &cancellation
                    )
                    .unwrap(),
                    "I={candidate}, J={tested}: {source}",
                );
            }
        }
    }
}

#[test]
fn certain_body_atoms_shrink_the_theory_they_are_read_in() {
    for source in BODIES {
        let actual = compile(source);
        let reference = without_certainty(|| compile(source));
        assert!(
            actual.theory.nodes().len() < reference.theory.nodes().len(),
            "{source}"
        );
    }
}

#[test]
fn a_certain_atom_in_a_condition_is_read_as_true() {
    // The condition's atom leaves the formula. Where it stood alone the
    // implication it guarded keeps its node, so the nodes differ rather than
    // shrink.
    for source in CONDITIONS {
        let actual = compile(source);
        let reference = without_certainty(|| compile(source));
        assert_eq!(names(&actual), names(&reference), "{source}");
        assert_ne!(actual.theory.nodes(), reference.theory.nodes(), "{source}");
    }
}

#[test]
fn a_negated_certain_atom_is_decided() {
    // `d` holds in every answer set. Under `not` the rule has no instance,
    // so `q` is never named; under `not not` the literal leaves the body.
    let [dropped, kept] = NEGATED else {
        panic!("two negated sources");
    };
    let reference = without_certainty(|| compile(dropped));
    assert!(names(&reference).contains(&"q".to_owned()));
    assert!(!names(&compile(dropped)).contains(&"q".to_owned()));
    let actual = compile(kept);
    let reference = without_certainty(|| compile(kept));
    assert_eq!(names(&actual), names(&reference));
    assert!(actual.theory.nodes().len() < reference.theory.nodes().len());
}

#[test]
fn negated_certain_atoms_preserve_the_answer_sets() {
    for source in NEGATED {
        let expected = answer_sets(&without_certainty(|| compile(source)));
        assert!(!expected.is_empty(), "{source}");
        assert_eq!(answer_sets(&compile(source)), expected, "{source}");
    }
}

#[test]
fn negated_certain_atoms_agree_with_every_round_traversal() {
    // The reference holds no producer back, so it decides no negated atom
    // while completing the support; the answer sets are the same.
    use crate::formula_support::producers::testing::with_every_round_traversed;
    for source in NEGATED {
        let actual = compile(source);
        let reference = with_every_round_traversed(|| compile(source));
        assert_eq!(answer_sets(&actual), answer_sets(&reference), "{source}");
    }
}

#[test]
fn exact_assignments_preserve_the_answer_sets() {
    for source in ASSIGNMENTS {
        let actual = compile(source);
        let reference = without_certainty(|| compile(source));
        let expected = answer_sets(&reference);
        assert!(!expected.is_empty(), "{source}");
        assert_eq!(answer_sets(&actual), expected, "{source}");
    }
}

#[test]
fn exact_assignments_agree_with_every_round_traversal() {
    // The reference defers no producer, so it lists no element and proposes
    // every value; the answer sets are the same.
    use crate::formula_support::producers::testing::with_every_round_traversed;
    for source in ASSIGNMENTS {
        let actual = compile(source);
        let reference = with_every_round_traversed(|| compile(source));
        assert_eq!(answer_sets(&actual), answer_sets(&reference), "{source}");
    }
}

/// `source` grounded within `rounds` support rounds.
fn compile_within(source: &str, rounds: u64) -> Result<Compiled, crate::FormulaFailure> {
    let mut preparation = testing::prepare(source);
    preparation.limits.max_support_rounds = rounds;
    ground(preparation, None, None)
}

/// Whether support completion refused for want of rounds.
fn out_of_rounds(result: &Result<Compiled, crate::FormulaFailure>) -> bool {
    matches!(
        result,
        Err(crate::FormulaFailure::Limit {
            resource: crate::FormulaResource::SupportRounds,
            ..
        })
    )
}

/// The atoms of `compiled` whose name starts with `prefix`.
fn heads(compiled: &Compiled, prefix: &str) -> Vec<String> {
    names(compiled)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .collect()
}

/// Rules that read the predicate they derive through an assignment, with an
/// element over an earlier certain predicate. Each reaches a fixed point
/// without certainty as well, so the answer sets can be compared.
const HELD_BACK: &[&str] = &[
    "f(1). {p(N)} :- N = #count { V : p(V); V : f(V) }, N < 3.",
    "f(1). p(N) :- N = #count { V : p(V); V : f(V) }, N < 3.",
    "g(1). f(X) :- g(X). {p(N)} :- N = #count { V : p(V); V : f(V) }, N < 3.",
    "f(2). {p(N)} :- N = #sum { V : p(V); V : f(V) }.",
    "f(2). {p(N)} :- N = #max { V : p(V); V : f(V) }.",
    "f(2). {p(N)} :- N = #min { V : p(V); V : f(V) }.",
    // A certain element beside an uncertain one from an earlier component.
    "f(1). {q}. g(2) :- q. {p(N)} :- N = #count { V : p(V); V : f(V); V : g(V) }, N < 4.",
    // The same rule read by another.
    "f(1). {p(N)} :- N = #count { V : p(V); V : f(V) }, N < 3. r :- p(1).",
];

#[test]
fn a_held_back_assignment_preserves_the_answer_sets() {
    for source in HELD_BACK {
        let expected = written_answer_sets(source);
        assert!(!expected.is_empty(), "{source}");
        assert_eq!(answer_sets(&compile(source)), expected, "{source}");
    }
}

#[test]
fn a_held_back_assignment_agrees_with_every_round_traversal() {
    // The reference holds no producer back, lists nothing and proposes every
    // value its tuples allow; the answer sets are the same.
    use crate::formula_support::producers::testing::with_every_round_traversed;
    for source in HELD_BACK {
        let reference = with_every_round_traversed(|| compile(source));
        assert_eq!(
            answer_sets(&compile(source)),
            answer_sets(&reference),
            "{source}"
        );
    }
}

#[test]
fn a_held_back_assignment_proposes_no_value_below_its_required_tuples() {
    let source = "f(1). {p(N)} :- N = #count { V : p(V); V : f(V) }, N < 5.";
    assert_eq!(heads(&compile(source), "p("), ["p(1)"]);
    // Without certainty the fact's tuple is optional: none is proposed, its
    // head adds a tuple, and every value up to the bound follows.
    assert_eq!(heads(&without_certainty(|| compile(source)), "p(").len(), 5);
}

/// A count over the predicate its rule derives and over a fact. Its values
/// are bounded only because the fact's tuple is required: 3 is the last
/// value whose head adds no tuple.
const BOUNDED_BY_A_FACT: &str = "f(3). {p(N)} :- N = #count { V : p(V); V : f(V) }.";

#[test]
fn a_count_bounded_by_a_required_tuple_reaches_its_fixed_point() {
    let compiled = compile_within(BOUNDED_BY_A_FACT, 16).unwrap();
    assert_eq!(heads(&compiled, "p("), ["p(1)", "p(2)", "p(3)"]);
    // Choosing any `p(N)` raises the count past `N`.
    assert_eq!(answer_sets(&compiled), sets(&["f(3)"]));
}

#[test]
fn a_count_bounded_by_a_derived_certain_tuple_reaches_its_fixed_point() {
    let source = "g(3). f(X) :- g(X). {p(N)} :- N = #count { V : p(V); V : f(V) }.";
    let compiled = compile_within(source, 16).unwrap();
    assert_eq!(heads(&compiled, "p("), ["p(1)", "p(2)", "p(3)"]);
    assert_eq!(answer_sets(&compiled), sets(&["f(3) g(3)"]));
}

#[test]
fn a_count_whose_required_tuple_is_its_own_value_reaches_its_fixed_point() {
    let source = "f(1). {p(N)} :- N = #count { V : p(V); V : f(V) }.";
    let compiled = compile_within(source, 16).unwrap();
    assert_eq!(heads(&compiled, "p("), ["p(1)"]);
    assert_eq!(answer_sets(&compiled), sets(&["f(1)", "f(1) p(1)"]));
}

#[test]
fn a_count_bounded_by_a_tuple_decided_through_negation_reaches_its_fixed_point() {
    // No rule derives `h`, so `not h(X)` holds and `f` is certain: its tuple
    // is required, as a fact's is.
    let source = "g(3). f(X) :- g(X), not h(X). {p(N)} :- N = #count { V : p(V); V : f(V) }.";
    let compiled = compile_within(source, 16).unwrap();
    assert_eq!(heads(&compiled, "p("), ["p(1)", "p(2)", "p(3)"]);
    assert_eq!(answer_sets(&compiled), sets(&["f(3) g(3)"]));
    // Without the decision `f` is not certain and the count proposes 0.
    assert!(out_of_rounds(&without_decided_negation(|| {
        compile_within(source, 16)
    })));
}

#[test]
fn a_negated_fact_stops_a_recursive_count() {
    // The count's rule waits for `stop`, which then holds in every answer
    // set: no row of the rule derives a head, so the count proposes nothing.
    let source = "stop. {p(N)} :- N = #count { V : p(V) }, not stop.";
    let compiled = compile_within(source, 16).unwrap();
    assert!(heads(&compiled, "p(").is_empty());
    assert_eq!(answer_sets(&compiled), sets(&["stop"]));
    assert!(out_of_rounds(&without_decided_negation(|| {
        compile_within(source, 16)
    })));
}

#[test]
fn a_deferred_assignment_is_omitted_where_its_decided_negation_fails() {
    // The count is 2. `not e(2)` fails where `e(2)` is a fact, so the rule
    // supports no head; with `e(1)` it supports `n(2)`, which is certain.
    assert!(
        heads(
            &compile("c(1..2). e(2). n(N) :- N = #count { X : c(X) }, not e(N)."),
            "n("
        )
        .is_empty()
    );
    assert_eq!(
        heads(
            &compile("c(1..2). e(1). n(N) :- N = #count { X : c(X) }, not e(N)."),
            "n("
        ),
        ["n(2)"]
    );
}

#[test]
fn a_possible_blocker_does_not_stop_a_recursive_count() {
    // A chosen atom decides nothing: the count still proposes without end.
    assert!(out_of_rounds(&compile_within(
        "{stop}. {p(N)} :- N = #count { V : p(V) }, not stop.",
        16
    )));
}

#[test]
fn a_held_back_assignment_counts_every_tuple_of_a_relation_that_grew() {
    // `f` gains a row in each of four rounds. The count's rule waits for all
    // four, so its least value is 4; a traversal over fewer would propose
    // each of 1 to 3 on the way.
    let source = "f(0). f(Y) :- f(X), X < 3, Y = X + 1. \
                  {p(N)} :- N = #count { V : p(V); V : f(V) }, N < 6.";
    assert_eq!(heads(&compile(source), "p("), ["p(4)", "p(5)"]);
}

#[test]
fn the_same_count_without_its_required_tuple_never_reaches_one() {
    use crate::formula_support::producers::testing::with_every_round_traversed;
    // What the required tuple prevents: with the fact's tuple optional the
    // count proposes 0, and each head it supports adds a tuple.
    assert!(out_of_rounds(&without_certainty(|| compile_within(
        BOUNDED_BY_A_FACT,
        16
    ))));
    assert!(out_of_rounds(&with_every_round_traversed(|| {
        compile_within(BOUNDED_BY_A_FACT, 16)
    })));
}

#[test]
fn an_exact_assignment_supports_only_its_value() {
    let source = "c(1..3). r(C,R) :- c(C), R = #count { D : c(D), D <= C }. {p}.";
    assert_eq!(
        heads(&compile(source), "r("),
        ["r(1,1)", "r(2,2)", "r(3,3)"]
    );
    // Without certainty every count from none to all is proposed.
    assert_eq!(
        heads(&without_certainty(|| compile(source)), "r(").len(),
        2 + 3 + 4
    );
}

#[test]
fn an_assignment_with_an_uncertain_element_keeps_its_optional_values() {
    let source = "c(1). {p}. n(N) :- N = #count { a : c(1); b : p }.";
    let heads = |compiled: &Compiled| -> Vec<String> {
        names(compiled)
            .into_iter()
            .filter(|name| name.starts_with("n("))
            .collect()
    };
    // The certain tuple is required; the other may or may not be present.
    assert_eq!(heads(&compile(source)), ["n(1)", "n(2)"]);
    assert_eq!(
        heads(&without_certainty(|| compile(source))),
        ["n(0)", "n(1)", "n(2)"]
    );
}

#[test]
fn certainty_reaches_through_an_exact_assignment() {
    // `n` is certain through its count, so `m` is, and `q` reads neither.
    let source = "c(1..2). n(N) :- N = #count { X : c(X) }. m(M) :- n(N), M = N + 1. \
                  {p}. q :- m(3), p.";
    let actual = compile(source);
    let reference = without_certainty(|| compile(source));
    assert_eq!(
        names(&actual)
            .iter()
            .filter(|name| name.starts_with("m("))
            .collect::<Vec<_>>(),
        ["m(3)"]
    );
    assert!(actual.theory.nodes().len() < reference.theory.nodes().len());
}

#[test]
fn a_fully_required_assignment_is_emitted_as_its_value_alone() {
    // The count's head is rooted as a fact is: no family tests its tuples.
    let counted = compile("c(1..3). n(N) :- N = #count { X : c(X) }. {p}.");
    let stated = compile("c(1..3). n(3). {p}.");
    assert_eq!(names(&counted), names(&stated));
    assert_eq!(counted.theory.nodes(), stated.theory.nodes());
    assert_eq!(counted.theory.roots(), stated.theory.roots());
}

#[test]
fn a_chosen_atom_assumed_certain_is_refused() {
    // A choice produces `d` under its own permission, which is no ordinary
    // root. Calling `d` certain would read it as true in `q`'s body; emission
    // must not publish that.
    let result =
        with_every_predicate_assumed(|| ground(testing::prepare("{d}. q :- d."), None, None));
    assert!(matches!(
        result,
        Err(crate::FormulaFailure::SourceActivity { .. })
    ));
}

#[test]
fn an_atom_assumed_certain_under_a_condition_is_refused() {
    // `d` follows only when `e` is absent. Calling every predicate certain
    // leaves that producer conditional, so `d` is not derived by
    // unconditional roots alone.
    let result = with_every_predicate_assumed(|| {
        ground(testing::prepare("{e}. d :- not e. q :- d."), None, None)
    });
    assert!(matches!(
        result,
        Err(crate::FormulaFailure::SourceActivity { .. })
    ));
}

#[test]
fn a_circular_assumption_is_refused() {
    // A choice seeds `a`, and `a` and `b` derive each other. Were both called
    // certain, each would gain an unconditional root through the other. The
    // choice is a producer of `a` that is not one, so nothing is published.
    let result = with_every_predicate_assumed(|| {
        ground(testing::prepare("{a}. b :- a. a :- b."), None, None)
    });
    assert!(matches!(
        result,
        Err(crate::FormulaFailure::SourceActivity { .. })
    ));
}

#[test]
fn an_atom_read_as_true_by_a_retained_constraint_is_checked() {
    // On the hybrid schedule the constraint is grounded into the core after
    // every producer, and is then the only body that reads `d`.
    let result = with_every_predicate_assumed(|| retained("{d}. :- d."));
    assert!(matches!(
        result,
        Err(crate::FormulaFailure::SourceActivity { .. })
    ));
}

#[test]
fn a_retained_constraint_reads_certain_atoms_as_eager_emission_does() {
    let source = "c(1..3). d(X) :- c(X), X > 1. {p(X)} :- c(X). :- d(X), not p(X).";
    let (hybrid, streamed) = retained(source).unwrap();
    assert!(streamed.is_none(), "the constraint fits in the core");
    let eager = compile(source);
    assert_eq!(names(&hybrid), names(&eager));
    assert_eq!(answer_sets(&hybrid), answer_sets(&eager));
    let (written, _) = without_certainty(|| retained(source)).unwrap();
    assert!(hybrid.theory.nodes().len() < written.theory.nodes().len());
}

#[test]
fn an_explicit_fact_with_another_producer_is_still_read_as_true() {
    // `d` is a fact and also follows from a choice, so its predicate is not
    // certain; its own root forces it all the same.
    let compiled = compile("d. {c}. d :- c. q :- d.");
    let stated = compile("d. {c}. d :- c. q.");
    assert_eq!(names(&compiled), names(&stated));
    assert_eq!(answer_sets(&compiled), answer_sets(&stated));
    assert_eq!(compiled.theory.nodes(), stated.theory.nodes());
}

#[test]
fn an_exact_assignment_omits_the_instance_undefined_at_its_one_value() {
    // The count is 3 in every interpretation, so the rule's one instance
    // divides by zero and is omitted with a warning: `p` has no atom. The
    // counts 0 to 2 that an uncertain condition also proposes are defined, so
    // `p` has an atom at each, which no answer set holds.
    let source = "c(1..3). p(Y) :- N = #count { X : c(X) }, Y = 6/(N-3). {a}.";
    let (compiled, written) = omitting(
        source,
        "p(Y) :- N = #count { X : c(X) }, Y = 6/(N-3).",
        &["c(1) c(2) c(3)", "c(1) c(2) c(3) a"],
    );
    assert!(heads(&compiled, "p(").is_empty());
    assert_eq!(heads(&written, "p("), ["p(-2)", "p(-3)", "p(-6)"]);
}

// The seven tests below pin how far the narrowing reaches: a family follows
// the values an assignment can still take wherever it reads them. At those
// values every instance of the family divides by zero, so each is omitted
// with a warning and the family yields nothing. With no predicate treated as
// certain the family also has instances at values no answer set takes, and
// the answer sets are the same. A constraint and a choice body show no more
// than that: their instances at those other values are defined and false, so
// neither grounding holds one.

#[test]
fn a_rule_reading_an_exact_head_is_omitted_where_that_value_is_undefined() {
    // Only `n(3)` is supported, so the reader's one instance divides by zero.
    let source = "c(1..3). n(N) :- N = #count { D : c(D) }. z(X) :- n(N), X = 6/(N-3).";
    let (compiled, written) = omitting(
        source,
        "z(X) :- n(N), X = 6/(N-3).",
        &["c(1) c(2) c(3) n(3)"],
    );
    assert!(heads(&compiled, "z(").is_empty());
    assert_eq!(heads(&written, "z("), ["z(-2)", "z(-3)", "z(-6)"]);
}

#[test]
fn a_reader_of_a_reader_follows_the_exact_head() {
    let source = "c(1..2). n(N) :- N = #count { D : c(D) }. m(N) :- n(N). \
        z(X) :- m(N), X = 6/(N-2).";
    let (compiled, written) = omitting(
        source,
        "z(X) :- m(N), X = 6/(N-2).",
        &["c(1) c(2) n(2) m(2)"],
    );
    assert!(heads(&compiled, "z(").is_empty());
    assert_eq!(heads(&written, "z("), ["z(-3)", "z(-6)"]);
}

#[test]
fn a_constraint_reading_an_exact_head_is_omitted_at_an_undefined_value() {
    // `6/(N-3) > 1` is false at the counts 0 to 2, so the constraint has no
    // instance without certainty either. Were its instance at 3 kept as a
    // body that holds, no answer set would hold `a`.
    let source = "c(1..3). n(N) :- N = #count { D : c(D) }. {a}. :- n(N), 6/(N-3) > 1, a.";
    let (compiled, written) = omitting(
        source,
        ":- n(N), 6/(N-3) > 1, a.",
        &["c(1) c(2) c(3) n(3)", "c(1) c(2) c(3) n(3) a"],
    );
    assert_eq!(heads(&compiled, "n("), ["n(3)"]);
    assert_eq!(heads(&written, "n("), ["n(0)", "n(1)", "n(2)", "n(3)"]);
}

#[test]
fn a_choice_body_reading_an_exact_head_is_omitted_at_an_undefined_value() {
    // `6/(N-2) > 1` is false at the counts 0 and 1, so no grounding supports
    // the choice: `y` has no atom.
    let source = "c(1..2). n(N) :- N = #count { D : c(D) }. {y} :- n(N), 6/(N-2) > 1.";
    let (compiled, written) = omitting(source, "{y} :- n(N), 6/(N-2) > 1.", &["c(1) c(2) n(2)"]);
    assert_eq!(heads(&compiled, "n("), ["n(2)"]);
    assert_eq!(heads(&written, "n("), ["n(0)", "n(1)", "n(2)"]);
    assert!(heads(&compiled, "y").is_empty());
    assert!(heads(&written, "y").is_empty());
}

#[test]
fn an_aggregate_element_reading_an_exact_head_follows_it() {
    // The element's one tuple is undefined and omitted, so the sum is over no
    // tuple. Without certainty the element has a tuple at each of the counts
    // 0 and 1, of weights -3 and -6, and the sum takes each of their totals.
    let source = "c(1..2). n(N) :- N = #count { D : c(D) }. \
        q(S) :- S = #sum { 6/(N-2) : n(N) }.";
    let (compiled, written) = omitting(
        source,
        "q(S) :- S = #sum { 6/(N-2) : n(N) }.",
        &["c(1) c(2) n(2) q(0)"],
    );
    assert_eq!(heads(&compiled, "q("), ["q(0)"]);
    assert_eq!(heads(&written, "q("), ["q(-9)", "q(-6)", "q(-3)", "q(0)"]);
}

#[test]
fn a_weak_constraint_reading_an_exact_head_follows_it() {
    // The objective has no element at the count 2, whose weight is undefined,
    // and one at each of the counts 0 and 1 where they are proposed.
    let source = "c(1..2). n(N) :- N = #count { D : c(D) }. {a}. :~ n(N), a. [6/(N-2)@1]";
    let (compiled, written) = omitting(
        source,
        ":~ n(N), a. [6/(N-2)@1]",
        &["c(1) c(2) n(2)", "c(1) c(2) n(2) a"],
    );
    assert!(compiled.objectives.templates().is_empty());
    assert_eq!(written.objectives.templates().len(), 2);
}

#[test]
fn an_assignment_left_with_several_values_is_omitted_where_each_is_undefined() {
    // The three tuples over `c` are required and the two over `d` are not,
    // so the count is 3, 4 or 5, and the divisor is zero at each. Without
    // certainty the counts 0 to 2 are proposed as well.
    let source = "c(1..3). {d(1..2)}. \
        z(X) :- N = #count { D : c(D); D,d : d(D) }, X = 60/((N-3)*(N-4)*(N-5)).";
    let (compiled, written) = omitting(
        source,
        "z(X) :- N = #count { D : c(D); D,d : d(D) }, X = 60/((N-3)*(N-4)*(N-5)).",
        &[
            "c(1) c(2) c(3)",
            "c(1) c(2) c(3) d(1)",
            "c(1) c(2) c(3) d(2)",
            "c(1) c(2) c(3) d(1) d(2)",
        ],
    );
    assert!(heads(&compiled, "z(").is_empty());
    assert_eq!(heads(&written, "z("), ["z(-1)", "z(-2)", "z(-10)"]);
}

// The next two tests pin the narrowing for the two forms a zero divisor
// cannot show it for. Their arithmetic overflows at every value the exact
// head excludes and at no other, so each program is admitted exactly when
// its family follows that head.

/// Whether `source`, grounded with no predicate treated as certain, is
/// refused for an arithmetic overflow.
fn overflows_as_written(source: &str) -> bool {
    matches!(
        without_certainty(|| ground(testing::prepare(source), None, None)),
        Err(crate::FormulaFailure::Expansion(
            crate::ExpansionFailure::Evaluation {
                error: themelios_program::term::EvalError::Overflow,
                ..
            }
        ))
    )
}

#[test]
fn a_constraint_reading_an_exact_head_follows_it() {
    // `2000000000 * (4 - N)` overflows at every count but 3, the only one
    // `n` takes. There the comparison holds, so the constraint excludes `a`.
    let source = "c(1..3). n(N) :- N = #count { D : c(D) }. {a}. \
        :- n(N), 2000000000 * (4 - N) > 0, a.";
    assert_eq!(
        answer_sets(&compile(source)),
        sets(&["c(1) c(2) c(3) n(3)"])
    );
    assert!(overflows_as_written(source));
}

#[test]
fn a_choice_body_reading_an_exact_head_follows_it() {
    // `2000000000 * (3 - N)` overflows at the counts 0 and 1 and holds at 2,
    // the only one `n` takes, so the choice is supported there.
    let source = "c(1..2). n(N) :- N = #count { D : c(D) }. \
        {y} :- n(N), 2000000000 * (3 - N) > 0.";
    assert_eq!(
        answer_sets(&compile(source)),
        sets(&["c(1) c(2) n(2)", "c(1) c(2) n(2) y"])
    );
    assert!(overflows_as_written(source));
}

#[test]
fn an_exact_assignment_admits_arithmetic_that_fails_only_at_unrealized_values() {
    // `2000000000 * (4 - N)` overflows for every count but 3, the only one
    // the aggregate takes.
    let source = "c(1..3). p(Y) :- N = #count { X : c(X) }, Y = 2000000000 * (4 - N). {a}.";
    let compiled = compile(source);
    assert!(names(&compiled).contains(&"p(2000000000)".to_owned()));
    assert!(without_certainty(|| ground(testing::prepare(source), None, None)).is_err());
}

// A rule held back for its required tuples takes fewer values too, so its
// family and the families that read its head change with it.

#[test]
fn a_held_back_assignment_omits_the_instance_undefined_at_its_remaining_value() {
    // The fact's tuple is required, so the count below 2 is 1, where
    // `6/(N-1)` divides by zero: the rule's one instance is omitted and `p`
    // has no atom. With the tuple optional the count 0 is defined as well,
    // and supports `p(-6)`, which no answer set holds.
    let source = "f(3). {p(Y)} :- N = #count { V : p(V); V : f(V) }, N < 2, Y = 6/(N-1).";
    let (compiled, written) = omitting(
        source,
        "{p(Y)} :- N = #count { V : p(V); V : f(V) }, N < 2, Y = 6/(N-1).",
        &["f(3)"],
    );
    assert!(heads(&compiled, "p(").is_empty());
    assert_eq!(heads(&written, "p("), ["p(-6)"]);
}

#[test]
fn a_rule_reading_a_held_back_head_is_omitted_where_its_value_is_undefined() {
    // Only `p(1)` is supported, so the reader's one instance divides by zero
    // and `z` has no atom. With the fact's tuple optional `p(0)` is supported
    // too, and the reader is defined there.
    let source = "f(1). {p(N)} :- N = #count { V : p(V); V : f(V) }, N < 2. \
        z(Y) :- p(N), Y = 6/(N-1).";
    let (compiled, written) =
        omitting(source, "z(Y) :- p(N), Y = 6/(N-1).", &["f(1)", "f(1) p(1)"]);
    assert!(heads(&compiled, "z(").is_empty());
    assert_eq!(heads(&written, "z("), ["z(-6)"]);
}

#[test]
fn a_held_back_assignment_admits_what_only_an_unrequired_value_refused() {
    // The fact's tuple is required, so the maximum is at least 3. With the
    // tuple optional the empty maximum is proposed, and `#inf + 1` is
    // arithmetic on a value that is no number: a fatal failure, which
    // refuses.
    let source = "f(3). {p(N+1)} :- N = #max { V : p(V); V : f(V) }, N < 6.";
    assert_eq!(answer_sets(&compile(source)), sets(&["f(3)"]));
    assert!(matches!(
        without_certainty(|| ground(testing::prepare(source), None, None)),
        Err(crate::FormulaFailure::Expansion(
            crate::ExpansionFailure::Evaluation {
                error: themelios_program::term::EvalError::Undefined,
                ..
            }
        ))
    ));
}
