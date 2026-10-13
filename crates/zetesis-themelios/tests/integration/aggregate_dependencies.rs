//! Dependent aggregate proposals retain their original equality and frozen truth.

use crate::support::admission;
use crate::support::objective_dependency_records as objective_dependencies;
use zetesis_reference_support::resource_limits;
use zetesis_test_support::thresholds::first_success;

mod cases;
use crate::support::finite_bindings as reference;

use std::collections::BTreeSet;

use cases::CASES;
use reference::{Models, exhaustive, native};
use themelios_base::source::SourceId;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, FormulaWarning, admit_formula, prepare_formula,
};

const SOURCE: SourceId = SourceId::new(157);

fn options() -> AdmissionOptions {
    admission::options(SOURCE)
}

fn input(source: &str) -> AdmittedFormula {
    admission::admitted(SOURCE, source)
}

#[test]
fn models_match_finite_substitutions() {
    reference::models_match_substitutions(CASES.iter().copied(), input);
}

#[test]
fn stability_matches_subset_enumeration() {
    reference::stability_matches_exhaustion(CASES.iter().map(|&(source, _)| source), input);
}

#[test]
fn frozen_truth_matches_finite_substitutions() {
    reference::frozen_truth_matches(CASES.iter().copied(), input);
}

#[test]
#[ignore = "requires clingo: original sources match clingo full models"]
fn original_sources_match_clingo_full_models() {
    reference::original_sources_match_clingo_full_models(
        CASES.iter().map(|&(source, _)| source),
        input,
    );
}

#[test]
fn every_producer_equality_remains_semantic() {
    let source = "{p}.q(N,M):-N=#count{1:p},M=#sum{N}.";
    let expected = Models::from([
        BTreeSet::from(["q(0,0)".into()]),
        BTreeSet::from(["p".into(), "q(1,1)".into()]),
    ]);
    assert_eq!(native(&input(source)), expected);
    for weakened in [
        "{p}.q(0,M):-M=#sum{0}.q(1,M):-M=#sum{1}.",
        "{p}.q(0,0):-0=#count{1:p}.q(1,0):-1=#count{1:p}.q(1,1):-1=#count{1:p}.",
    ] {
        assert_ne!(native(&input(weakened)), expected);
    }
}

#[test]
fn source_order_preserves_dependent_values() {
    let instructions = ["N=#count{1:p}", "Y=N+1", "M=#sum{Y}"];
    let expected = Models::from([
        BTreeSet::from(["q(1)".into()]),
        BTreeSet::from(["p".into(), "q(2)".into()]),
    ]);
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let body = order.map(|index| instructions[index]).join(",");
        assert_eq!(native(&input(&format!("{{p}}.q(M):-{body}."))), expected);
    }
}

#[test]
fn repeated_predecessors_restart_dependent_carriers() {
    let source = "i(1..2).q(I,M):-i(I),N=#count{},M=#sum{N}.";
    assert_eq!(
        native(&input(source)),
        Models::from([BTreeSet::from([
            "i(1)".into(),
            "i(2)".into(),
            "q(1,0)".into(),
            "q(2,0)".into(),
        ])])
    );
}

#[test]
fn full_outer_values_distinguish_cached_families() {
    let source = "{p}.q(N,M):-N=#count{1:p},M=#count{N:p;0:p}.";
    assert_eq!(
        native(&input(source)),
        Models::from([
            BTreeSet::from(["q(0,0)".into()]),
            BTreeSet::from(["p".into(), "q(1,2)".into()]),
        ])
    );
    // N=0 coalesces the keys; N=1 keeps two. Reusing the first
    // family's eligibility or equality roots would lose q(1,2).
    assert_ne!(
        native(&input("{p}.q(N,M):-N=#count{1:p},M=#count{0:p;0:p}.")),
        native(&input(source))
    );
}

#[test]
fn choice_activation_keeps_the_complete_dependent_group() {
    let source = "{e}.M{a;b}M:-e,N=#count{},Y=N+1,M=#sum{Y}.";
    let expected = Models::from([
        BTreeSet::new(),
        BTreeSet::from(["e".into(), "a".into()]),
        BTreeSet::from(["e".into(), "b".into()]),
    ]);
    assert_eq!(native(&input(source)), expected);
    assert_ne!(native(&input("{e}.1{a}1:-e.1{b}1:-e.")), expected);
}

#[test]
fn cyclic_producers_have_located_refusals() {
    for source in [
        "q(N,M):-N=#count{M:p},M=#count{N:q}.",
        "q(M):-N=#count{Y:p},M=#sum{N},Y=M+1.",
        "q(M):-N=#count{K:p},K=M..M+1,M=#sum{N}.",
    ] {
        let error = prepare_formula(
            source.into(),
            options(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, FormulaFailure::CyclicValueInput { .. }),
            "{source}: {error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    }
}

#[test]
fn producer_inputs_require_independent_bindings() {
    for source in [
        "q(N):-N=#count{N:p}.",
        "q(N,M):-N=#count{},M=#count{X:p(M,X)}.",
        "q(M):-N=#count{},M=#count{X:not p(X,N)}.",
        "q(M):-N=#count{},M=#count{X:p(X),X<Y}.",
    ] {
        let error = prepare_formula(
            source.into(),
            options(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{source}: {error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    }
}

#[test]
fn additional_guards_do_not_become_duplicate_producers() {
    for source in [
        "q(M):-N=#count{},M=#sum{N},M=#count{}.",
        "q(M):-N=#count{},M=#sum{N},M<=#count{}.",
    ] {
        assert_eq!(
            native(&input(source)),
            Models::from([BTreeSet::from(["q(0)".into()])])
        );
    }
}

#[test]
fn dependent_producers_preserve_scored_answers() {
    for source in [
        "q(M):-N=#count{},M=#sum{N}.#minimize{M:q(M)}.",
        "M{q}M:-N=#count{},M=#sum{N}.#minimize{1:q}.",
        "M#count{1:q}M:-N=#count{},M=#sum{N}.#minimize{1:q}.",
    ] {
        objective_dependencies::check(source);
    }
}

#[test]
fn unrelated_objectives_keep_their_priority() {
    let source = "{p}.q(M):-N=#count{},M=#sum{N}.#minimize{1@3:p}.";
    assert_eq!(input(source).objectives().priorities(), &[3]);
}

#[test]
fn mixed_proposals_keep_the_defined_family() {
    // The count's element is optional, so its zero proposal is realizable and
    // divides by zero; the defined one-proposal keeps the family admitted with
    // a warning, and the original equalities decide which rows hold.
    let source = "{p}.q(M):-N=#count{1:p},Y=1/N,M=#sum{Y}.";
    let admitted = input(source);
    let [FormulaWarning::ZeroDivisor { location }] = admitted.warnings() else {
        panic!("one zero-divisor warning: {:?}", admitted.warnings());
    };
    assert_eq!(location.location().expect("parsed source").source, SOURCE);
    assert_eq!(
        admitted
            .source()
            .expect("source input")
            .slice(location.location().expect("parsed source").span)
            .unwrap(),
        "q(M):-N=#count{1:p},Y=1/N,M=#sum{Y}."
    );
    let expected = Models::from([BTreeSet::new(), BTreeSet::from(["p".into(), "q(1)".into()])]);
    assert_eq!(native(&admitted), expected);
    assert_eq!(exhaustive(&admitted), expected);
}

#[test]
fn a_required_count_proposes_no_unrealizable_zero() {
    // The element holds for every binding, so the count proposes one alone:
    // no instance divides by zero and no warning is retained.
    let admitted = input("q(M):-N=#count{1},Y=1/N,M=#sum{Y}.");
    assert!(admitted.warnings().is_empty(), "{:?}", admitted.warnings());
    let expected = Models::from([BTreeSet::from(["q(1)".into()])]);
    assert_eq!(native(&admitted), expected);
    assert_eq!(exhaustive(&admitted), expected);
}

#[test]
fn original_sources_remain_owned() {
    reference::sources_remain_owned(CASES.iter().map(|&(source, _)| source), input);
}

#[test]
fn predicate_recursion_cannot_bootstrap_positive_support() {
    let admitted = input("q(M):-N=#count{1:q(1)},M=#count{N:q(1)}.");
    assert_eq!(
        native(&admitted),
        Models::from([BTreeSet::from(["q(0)".into()])])
    );
    assert_eq!(native(&admitted), exhaustive(&admitted));
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(32))]
    #[test]
    fn affine_weights_preserve_dependent_sums(offset in -2_i32..3) {
        let source = format!("{{p}}.q(N,M):-M=#sum{{Y}},Y=N+({offset}),N=#count{{1:p}}.");
        let expected = Models::from([
            BTreeSet::from([format!("q(0,{offset})")]),
            BTreeSet::from(["p".into(), format!("q(1,{})", offset + 1)]),
        ]);
        proptest::prop_assert_eq!(native(&input(&source)), expected);
    }
}

#[test]
fn grounding_limits_are_inclusive() {
    let source = "q(M):-N=#count{},Y=N+1,M=#sum{Y}.";
    for resource in [
        FormulaResource::AssignmentValues,
        FormulaResource::Substitutions,
        FormulaResource::Work,
        FormulaResource::AggregateCacheRows,
        FormulaResource::AggregateCacheRoots,
    ] {
        let lower = first_success(resource_limits::formula_default(resource), |middle| {
            admit_formula(
                source.into(),
                options(),
                ExpansionLimits::default(),
                resource_limits::formula(resource, middle),
            )
            .is_ok()
        });
        assert_eq!(
            native(
                &admit_formula(
                    source.into(),
                    options(),
                    ExpansionLimits::default(),
                    resource_limits::formula(resource, lower)
                )
                .unwrap()
            ),
            native(&input(source))
        );
        let error = admit_formula(
            source.into(),
            options(),
            ExpansionLimits::default(),
            resource_limits::formula(resource, lower - 1),
        )
        .unwrap_err();
        assert!(
            matches!(error, FormulaFailure::Limit { resource: actual, limit, observed, .. } if actual == resource && limit == u128::from(lower-1) && observed == u128::from(lower)),
            "{error}"
        );
        let location = error.diagnostics()[0].primary().location;
        assert_eq!(location.source, SOURCE);
        assert_eq!(
            input(source)
                .source()
                .expect("source input")
                .slice(location.span)
                .unwrap(),
            source
        );
        println!("inclusive_{resource:?}={lower}");
    }
}

#[test]
fn scalar_storage_limit_is_inclusive() {
    let source = "q(M):-N=#count{},Y=N+1,M=#sum{Y}.";
    let lower = first_success(
        resource_limits::expansion_default(ExpansionResource::ScalarBytes),
        |middle| {
            admit_formula(
                source.into(),
                options(),
                resource_limits::expansion(ExpansionResource::ScalarBytes, middle),
                FormulaLimits::default(),
            )
            .is_ok()
        },
    );
    assert_eq!(
        native(
            &admit_formula(
                source.into(),
                options(),
                resource_limits::expansion(ExpansionResource::ScalarBytes, lower),
                FormulaLimits::default()
            )
            .unwrap()
        ),
        native(&input(source))
    );
    let error = admit_formula(
        source.into(),
        options(),
        resource_limits::expansion(ExpansionResource::ScalarBytes, lower - 1),
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Expansion(ExpansionFailure::Limit { resource: ExpansionResource::ScalarBytes, limit, observed, .. }) if limit == u128::from(lower - 1) && observed == u128::from(lower)),
        "{error}"
    );
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    println!("inclusive_scalar_bytes={lower}");
}
