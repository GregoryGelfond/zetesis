//! Certainty changes what is emitted and what an assignment proposes, never
//! which interpretations are answer sets.

use std::collections::BTreeSet;

use super::testing::{with_every_predicate_assumed, without_certainty};
use crate::formula::Compiled;
use crate::formula_ground::ground;
use crate::formula_support::testing;
use crate::test_support::compiled_atoms;
use zetesis_algebra::Cancellation;
use zetesis_core::{Sign, Value};
use zetesis_ferraris::{Interpretation, Limits, models, models_reduct};

fn compile(source: &str) -> Compiled {
    ground(testing::prepare(source), None, None).unwrap()
}

/// Whether grounding `source` is refused because a family has a zero-divisor
/// instance and no defined one: the undefined-operation failure, and no
/// other.
fn refused_as_undefined(source: &str) -> bool {
    matches!(
        ground(testing::prepare(source), None, None),
        Err(crate::FormulaFailure::Expansion(
            crate::ExpansionFailure::Evaluation {
                error: themelios_program::term::EvalError::Undefined,
                ..
            }
        ))
    )
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

/// Sources that read a certain atom under default negation only, where it is
/// left as written.
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
    BODIES.iter().chain(NEGATED).chain(CONDITIONS)
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
fn a_negated_certain_atom_is_emitted_as_written() {
    for source in NEGATED {
        let actual = compile(source);
        let reference = without_certainty(|| compile(source));
        assert_eq!(names(&actual), names(&reference), "{source}");
        assert_eq!(actual.theory.nodes(), reference.theory.nodes(), "{source}");
        assert_eq!(actual.theory.roots(), reference.theory.roots(), "{source}");
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
fn an_exact_assignment_refuses_arithmetic_undefined_at_its_one_value() {
    // The count is 3 in every interpretation, so `6/(N-3)` has no defined
    // instance and its family is entirely undefined. The counts 0 to 2 that
    // an uncertain condition also proposes are defined, and admit the rule
    // with a warning.
    let source = "c(1..3). p(Y) :- N = #count { X : c(X) }, Y = 6/(N-3). {a}.";
    assert!(refused_as_undefined(source));
    assert_eq!(
        written_answer_sets(source),
        sets(&["c(1) c(2) c(3)", "c(1) c(2) c(3) a"])
    );
}

// The seven tests below pin how far the narrowing reaches: a family follows
// the values an assignment can still take wherever it reads them. Each
// program is admitted with a warning when no predicate is treated as
// certain, and answers as written there.

#[test]
fn a_rule_reading_an_exact_head_is_refused_where_that_value_is_undefined() {
    // Only `n(3)` is supported, so the reader's one instance divides by zero.
    let source = "c(1..3). n(N) :- N = #count { D : c(D) }. z(X) :- n(N), X = 6/(N-3).";
    assert!(refused_as_undefined(source));
    assert_eq!(written_answer_sets(source), sets(&["c(1) c(2) c(3) n(3)"]));
}

#[test]
fn a_reader_of_a_reader_follows_the_exact_head() {
    let source = "c(1..2). n(N) :- N = #count { D : c(D) }. m(N) :- n(N). \
        z(X) :- m(N), X = 6/(N-2).";
    assert!(refused_as_undefined(source));
    assert_eq!(written_answer_sets(source), sets(&["c(1) c(2) n(2) m(2)"]));
}

#[test]
fn a_constraint_reading_an_exact_head_follows_it() {
    let source = "c(1..3). n(N) :- N = #count { D : c(D) }. {a}. :- n(N), 6/(N-3) > 1, a.";
    assert!(refused_as_undefined(source));
    assert_eq!(
        written_answer_sets(source),
        sets(&["c(1) c(2) c(3) n(3)", "c(1) c(2) c(3) n(3) a"])
    );
}

#[test]
fn a_choice_body_reading_an_exact_head_follows_it() {
    let source = "c(1..2). n(N) :- N = #count { D : c(D) }. {y} :- n(N), 6/(N-2) > 1.";
    assert!(refused_as_undefined(source));
    assert_eq!(written_answer_sets(source), sets(&["c(1) c(2) n(2)"]));
}

#[test]
fn an_aggregate_element_reading_an_exact_head_follows_it() {
    // The element's one tuple is undefined, so its local family is.
    let source = "c(1..2). n(N) :- N = #count { D : c(D) }. \
        q(S) :- S = #sum { 6/(N-2) : n(N) }.";
    assert!(refused_as_undefined(source));
    assert_eq!(written_answer_sets(source), sets(&["c(1) c(2) n(2) q(0)"]));
}

#[test]
fn a_weak_constraint_reading_an_exact_head_follows_it() {
    let source = "c(1..2). n(N) :- N = #count { D : c(D) }. {a}. :~ n(N), a. [6/(N-2)@1]";
    assert!(refused_as_undefined(source));
    assert_eq!(
        written_answer_sets(source),
        sets(&["c(1) c(2) n(2)", "c(1) c(2) n(2) a"])
    );
}

#[test]
fn an_assignment_left_with_several_values_is_refused_where_each_is_undefined() {
    // The three tuples over `c` are required and the two over `d` are not,
    // so the count is 3, 4 or 5, and the divisor is zero at each.
    let source = "c(1..3). {d(1..2)}. \
        z(X) :- N = #count { D : c(D); D,d : d(D) }, X = 60/((N-3)*(N-4)*(N-5)).";
    assert!(refused_as_undefined(source));
    assert_eq!(
        written_answer_sets(source),
        sets(&[
            "c(1) c(2) c(3)",
            "c(1) c(2) c(3) d(1)",
            "c(1) c(2) c(3) d(2)",
            "c(1) c(2) c(3) d(1) d(2)"
        ])
    );
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
// family and the families that read its head change with it, in both
// directions.

#[test]
fn a_held_back_assignment_is_refused_where_its_remaining_value_is_undefined() {
    // The fact's tuple is required, so the count below 2 is 1, where
    // `6/(N-1)` divides by zero. With the tuple optional the count 0 is
    // defined, and admits the rule with a warning.
    let source = "f(3). {p(Y)} :- N = #count { V : p(V); V : f(V) }, N < 2, Y = 6/(N-1).";
    assert!(refused_as_undefined(source));
    assert_eq!(written_answer_sets(source), sets(&["f(3)"]));
}

#[test]
fn a_rule_reading_a_held_back_head_is_refused_where_its_value_is_undefined() {
    // Only `p(1)` is supported, so the reader's one instance divides by zero.
    let source = "f(1). {p(N)} :- N = #count { V : p(V); V : f(V) }, N < 2. \
        z(Y) :- p(N), Y = 6/(N-1).";
    assert!(refused_as_undefined(source));
    assert_eq!(written_answer_sets(source), sets(&["f(1)", "f(1) p(1)"]));
}

#[test]
fn a_held_back_assignment_admits_what_only_an_unrequired_value_refused() {
    // The fact's tuple is required, so the maximum is at least 3. With the
    // tuple optional the empty maximum is proposed, and `#inf + 1` is
    // arithmetic on a value that is no number.
    let source = "f(3). {p(N+1)} :- N = #max { V : p(V); V : f(V) }, N < 6.";
    assert_eq!(answer_sets(&compile(source)), sets(&["f(3)"]));
    assert!(without_certainty(|| refused_as_undefined(source)));
}
