//! Complete truth tables and route observations over actual compiled source.

use super::testing::{self as route, Counts, Point};
use crate::formula::Compiled;
use crate::formula_ground::{ground, ground_retained};
use crate::formula_ir::{HeadIr, LiteralIr};
use crate::formula_support::{Counters, testing};
use crate::test_support::compiled_atoms;
use crate::{ExpansionFailure, FormulaFailure, FormulaResource};
use zetesis_core::Atom;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Limits, models, models_reduct};

const INDEPENDENT: &str = "{p(1,a);p(1,b);q(2,a);q(2,b)}. h(X,X+1) :- p(X,_), q(X+1,_).";

fn compile(source: &str, enabled: bool) -> (Compiled, Counts) {
    let (result, counts) = route::scoped(enabled, || ground(testing::prepare(source), None, None));
    (result.unwrap(), counts)
}

fn interpretation(compiled: &Compiled, vocabulary: &[Atom], bits: usize) -> Interpretation {
    Interpretation::new(
        &compiled.theory,
        compiled_atoms(compiled)
            .iter()
            .enumerate()
            .filter_map(|(index, atom)| {
                (bits & (1 << vocabulary.binary_search(atom).unwrap()) != 0).then_some(index)
            }),
    )
    .unwrap()
}

fn same_truth(source: &str) -> Counts {
    let (factored, counts) = compile(source, true);
    let (reference, skipped) = compile(source, false);
    assert_eq!(skipped, Counts::default());
    let mut vocabulary = compiled_atoms(&factored);
    vocabulary.sort();
    let mut original = compiled_atoms(&reference);
    original.sort();
    assert_eq!(vocabulary, original, "{source}");
    assert!(
        vocabulary.len() <= 8,
        "bounded complete truth table: {source}"
    );
    assert_eq!(factored.warnings, reference.warnings, "{source}");
    let origins = |compiled: &Compiled| {
        let mut sites: Vec<_> = compiled.origins.iter().flatten().copied().collect();
        sites.sort();
        sites.dedup();
        sites
    };
    assert_eq!(origins(&factored), origins(&reference), "{source}");
    let interpretations: Vec<_> = (0..1 << vocabulary.len())
        .map(|bits| {
            (
                interpretation(&factored, &vocabulary, bits),
                interpretation(&reference, &vocabulary, bits),
            )
        })
        .collect();
    let limits = Limits::default();
    let cancellation = Cancellation::default();
    for (candidate, (left, right)) in interpretations.iter().enumerate() {
        assert_eq!(
            models(&factored.theory, left, limits, &cancellation).unwrap(),
            models(&reference.theory, right, limits, &cancellation).unwrap(),
            "original {candidate}: {source}",
        );
        for (tested, (left_tested, right_tested)) in interpretations.iter().enumerate() {
            assert_eq!(
                models_reduct(&factored.theory, left, left_tested, limits, &cancellation).unwrap(),
                models_reduct(
                    &reference.theory,
                    right,
                    right_tested,
                    limits,
                    &cancellation
                )
                .unwrap(),
                "frozen {candidate}, tested {tested}: {source}",
            );
        }
    }
    counts
}

#[test]
fn poker_straights_have_flat_checked_components() {
    // Inspect the maintained source's actual preparation, before support or
    // formula construction. This is a shape test, not a model or timing claim.
    let preparation = testing::prepare(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/scalability/poker/draw_poker.lp"
    )));
    let mut counters = Counters::default();
    let components = preparation
        .catalog
        .component_view(&preparation.limits, &mut counters, preparation.location)
        .unwrap()
        .unwrap();
    let rules: Vec<_> = preparation
        .program
        .rules_without_facts()
        .iter()
        .filter(|rule| {
            let HeadIr::Normal(Some(head)) = rule.head else {
                return false;
            };
            head.get(
                components,
                &preparation.limits,
                &mut counters,
                rule.location,
            )
            .unwrap()
            .predicate()
            .name()
                == "straight"
                && rule
                    .body
                    .iter()
                    .any(|literal| matches!(literal, LiteralIr::Bind { .. }))
        })
        .collect();
    assert_eq!(rules.len(), 1);
    let rule = rules[0];
    assert_eq!(rule.variables, rule.body_variables + 1);
    assert!(rule.bindings.is_none());
    assert_eq!(
        rule.body
            .iter()
            .filter(|literal| matches!(literal, LiteralIr::PatternAtom(_)))
            .count(),
        4
    );
    assert!(rule.body.iter().all(|literal| {
        !matches!(literal, LiteralIr::PatternAtom(pattern) if !pattern.arguments.is_empty())
    }));
    assert_eq!(
        rule.body
            .iter()
            .filter(|literal| matches!(literal, LiteralIr::ArgumentCheck(_)))
            .count(),
        4
    );
}

#[test]
fn arithmetic_components_preserve_arbitrary_frozen_truth() {
    for source in [
        INDEPENDENT,
        "{p(1,a);p(1,b);q(2,a);q(2,b);a}. h(X,X+1) :- p(X,_),q(X+1,_),not a.",
        "{p(1,a);p(1,b);q(2,a);q(2,b);a}. h(X,X+1) :- p(X,_),q(X+1,_),not not a.",
        "{p(1,a);q(2,a);a}. h(X,X+1) :- p(X,_),q(X+1,_). p(1,b) :- h(1,2),a.",
        "{p(1,a);p(1,b);q(2,a);q(2,b)}. h(X) :- p(X,_),q(X+1,_),X+1=2.",
    ] {
        let counts = same_truth(source);
        assert!(counts.certified > 0, "certified route: {source}");
        assert!(counts.emissions > 0, "emitted factored producer: {source}");
    }
}

#[test]
fn incompatible_head_rows_keep_their_own_producers() {
    let source = "{p(1,a);p(1,b);q(2,a);q(2,b);a}. h(X,X+1) :- p(X,_),q(X+1,_). h(1,9) :- a.";
    let counts = same_truth(source);
    assert_eq!(counts.heads, 2);
    assert_eq!(counts.emissions, 1);
}

#[test]
fn partial_extra_head_rows_decline_before_emission() {
    for source in [
        "{p(1,a);p(1,b);q(2,a);q(2,b);a}. h(X,X+1) :- p(X,_),q(X+1,_). h(z,2) :- a.",
        "{p(1,a);q(2,a);a}. h(X,2/X) :- p(X,_),q(2/X,_). h(0,2) :- a.",
    ] {
        let counts = same_truth(source);
        assert!(counts.heads > 0);
        assert_eq!(counts.certified, 0);
        assert_eq!(counts.witnesses, 0);
        assert_eq!(counts.emissions, 0);
    }
}

#[test]
fn empty_components_do_not_emit_spurious_support() {
    let source = "{p(1,a);q(3,a);a}. h(X,X+1) :- p(X,_),q(X+1,_). h(1,2) :- a.";
    let counts = same_truth(source);
    assert_eq!(counts.certified, 1);
    assert_eq!(counts.emissions, 0);
}

#[test]
fn connected_or_local_arithmetic_keeps_the_complete_join() {
    for source in [
        "{p(1,a);p(1,b);q(2,a);q(2,b)}. h(X,X+1) :- p(X,S),q(X+1,S).",
        "{p(1,1);q(2,a)}. h(X) :- p(X,Y),q(Y+1,_).",
        "{p(f(1),a);q(2,a)}. h(X,X+1) :- p(f(X),_),q(X+1,_).",
    ] {
        assert_eq!(same_truth(source), Counts::default());
    }
}

#[test]
fn authoritative_arithmetic_failures_precede_factoring() {
    for source in [
        "p(0,a).q(1,a). h(X,1/X) :- p(X,_),q(1,_).",
        "p(a,1).q(2,1). h(X,X+1) :- p(X,_),q(2,_).",
    ] {
        let (factored, counts) =
            route::scoped(true, || ground(testing::prepare(source), None, None));
        let (reference, _) = route::scoped(false, || ground(testing::prepare(source), None, None));
        assert_eq!(counts, Counts::default());
        match (factored.unwrap_err(), reference.unwrap_err()) {
            (
                FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, location }),
                FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                    error: expected,
                    location: site,
                }),
            ) => {
                assert_eq!(error, expected);
                assert_eq!(location, site);
            }
            failures => panic!("same typed arithmetic failure: {failures:?}"),
        }
    }
}

#[test]
fn partial_source_families_keep_their_warning() {
    let source = "p(0,a).p(1,a). {q(1,a)}. h(X,1/X) :- p(X,_),q(1/X,_).";
    let counts = same_truth(source);
    assert_eq!(counts.certified, 1);
    let (compiled, _) = compile(source, true);
    assert!(!compiled.warnings.is_empty());
}

#[test]
fn interrupted_certification_or_emission_publishes_no_theory() {
    for point in [
        Point::Head,
        Point::Certified,
        Point::Witness,
        Point::Emission,
    ] {
        let cancellation = Cancellation::default();
        let mut preparation = testing::prepare(INDEPENDENT);
        preparation.budget = preparation
            .budget
            .with_cancellation(Some(cancellation.clone()));
        let (result, counts) = route::controlled(true, Some((point, 1, cancellation)), || {
            ground(preparation, None, None)
        });
        let error = result.unwrap_err();
        assert_eq!(
            error.interruption(),
            Some(zetesis_cpu::Stop::Cancelled),
            "{error:?}"
        );
        assert!(counts.heads > 0);
    }
}

#[test]
fn factorization_replaces_cartesian_roots_with_one_producer() {
    let (factored, counts) = compile(INDEPENDENT, true);
    let (reference, _) = compile(INDEPENDENT, false);
    assert_eq!(counts.certified, 1);
    assert_eq!(counts.witnesses, 4);
    assert_eq!(counts.emissions, 1);
    assert_eq!(
        reference.theory.roots().len() - factored.theory.roots().len(),
        3
    );
    assert!(factored.theory.nodes().len() < reference.theory.nodes().len());
}

#[test]
fn completed_factoring_keeps_the_grounding_work_limit() {
    let (retained, counts) = route::scoped(true, || {
        ground_retained(testing::prepare(INDEPENDENT), None, false).unwrap()
    });
    assert_eq!(counts.certified, 1);
    let mut exact = testing::prepare(INDEPENDENT);
    exact.limits.max_work = retained.accounting.work;
    route::scoped(true, || ground_retained(exact, None, false))
        .0
        .unwrap();
    let mut short = testing::prepare(INDEPENDENT);
    short.limits.max_work = retained.accounting.work - 1;
    assert!(matches!(
        route::scoped(true, || ground_retained(short, None, false)).0,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            ..
        })
    ));
}
