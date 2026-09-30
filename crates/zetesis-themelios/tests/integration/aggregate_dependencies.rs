//! Dependent aggregate proposals retain their original equality and frozen truth.

use crate::support::objective_dependency_records as objective_dependencies;

mod cases;
use crate::support::finite_bindings as reference;

use std::collections::BTreeSet;

use cases::CASES;
use reference::{Models, exhaustive, external, holds, native, values};
use themelios_base::source::SourceId;
use zetesis_reference_support::canonical;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, FormulaWarning, admit_formula, prepare_formula,
};

const SOURCE: SourceId = SourceId::new(157);

fn options() -> AdmissionOptions {
    AdmissionOptions {
        source_id: SOURCE,
        ..Default::default()
    }
}

fn input(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}

#[test]
fn models_match_finite_substitutions() {
    for &(source, expanded) in CASES {
        assert_eq!(native(&input(source)), native(&input(expanded)), "{source}");
    }
}

#[test]
fn stability_matches_subset_enumeration() {
    for &(source, _) in CASES {
        let admitted = input(source);
        assert_eq!(native(&admitted), exhaustive(&admitted), "{source}");
    }
}

#[test]
fn frozen_truth_matches_finite_substitutions() {
    let mut pairs = 0;
    for &(source, expanded) in CASES {
        let left = input(source);
        let right = input(expanded);
        let names: Vec<_> = left.atoms().iter().map(canonical).collect();
        let other: Vec<_> = right.atoms().iter().map(canonical).collect();
        assert_eq!(
            names.iter().collect::<BTreeSet<_>>(),
            other.iter().collect(),
            "{source}"
        );
        assert!(names.len() <= 7, "bounded exhaustive interpretation pairs");
        let remap = |mask: usize| {
            names.iter().enumerate().fold(0, |result, (index, name)| {
                result
                    | (usize::from(mask & (1 << index) != 0)
                        << other.iter().position(|other| other == name).unwrap())
            })
        };
        for outer in 0..1 << names.len() {
            let a = values(left.theory(), outer, None);
            let b = values(right.theory(), remap(outer), None);
            assert_eq!(
                holds(left.theory(), &a),
                holds(right.theory(), &b),
                "{source}"
            );
            for inner in 0..1 << names.len() {
                assert_eq!(
                    holds(left.theory(), &values(left.theory(), inner, Some(&a))),
                    holds(
                        right.theory(),
                        &values(right.theory(), remap(inner), Some(&b))
                    ),
                    "{source}: M={outer} J={inner}"
                );
                pairs += 1;
            }
        }
    }
    println!("source_pairs={} frozen_pairs={pairs}", CASES.len());
}

#[test]
#[ignore = "requires clingo: original sources match clingo full models"]
fn original_sources_match_clingo_full_models() {
    let mut total = 0;
    for &(source, _) in CASES {
        let result = external(source, true);
        assert_eq!(result["Models"]["More"], "no");
        let mut expected = Models::new();
        let mut count = 0;
        for call in result["Call"].as_array().unwrap() {
            if let Some(witnesses) = call["Witnesses"].as_array() {
                for witness in witnesses {
                    count += 1;
                    assert!(
                        expected.insert(
                            witness["Value"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|atom| atom.as_str().unwrap().to_owned())
                                .collect()
                        )
                    );
                }
            }
        }
        assert_eq!(result["Models"]["Number"].as_u64(), Some(count));
        assert_eq!(native(&input(source)), expected, "{source}");
        total += count;
    }
    println!("complete_sources={} full_models={total}", CASES.len());
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
    // The first aggregate actually equals one. Its unrealizable zero proposal
    // remains warning evidence; the original equalities decide which rows hold.
    let source = "q(M):-N=#count{1},Y=1/N,M=#sum{Y}.";
    let admitted = input(source);
    let [FormulaWarning::ZeroDivisor { location }] = admitted.warnings() else {
        panic!("one zero-divisor warning: {:?}", admitted.warnings());
    };
    assert_eq!(location.source, SOURCE);
    assert_eq!(admitted.source().slice(location.span).unwrap(), source);
    let expected = Models::from([BTreeSet::from(["q(1)".into()])]);
    assert_eq!(native(&admitted), expected);
    assert_eq!(exhaustive(&admitted), expected);
}

#[test]
fn original_sources_remain_owned() {
    for &(source, _) in CASES {
        assert_eq!(input(source).source().text(), source);
    }
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
        let limits = |ceiling| {
            let mut limits = FormulaLimits::default();
            match resource {
                FormulaResource::AssignmentValues => {
                    limits.max_assignment_values = usize::try_from(ceiling).unwrap();
                }
                FormulaResource::Substitutions => limits.max_substitutions = ceiling,
                FormulaResource::Work => limits.max_work = ceiling,
                FormulaResource::AggregateCacheRows => {
                    limits.max_aggregate_cache_rows = usize::try_from(ceiling).unwrap();
                }
                FormulaResource::AggregateCacheRoots => {
                    limits.max_aggregate_cache_roots = usize::try_from(ceiling).unwrap();
                }
                _ => unreachable!(),
            }
            limits
        };
        let mut lower = 0;
        let mut upper = 16_384;
        assert!(
            admit_formula(
                source.into(),
                options(),
                ExpansionLimits::default(),
                limits(upper)
            )
            .is_ok()
        );
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if admit_formula(
                source.into(),
                options(),
                ExpansionLimits::default(),
                limits(middle),
            )
            .is_ok()
            {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        assert!(lower > 0);
        assert_eq!(
            native(
                &admit_formula(
                    source.into(),
                    options(),
                    ExpansionLimits::default(),
                    limits(lower)
                )
                .unwrap()
            ),
            native(&input(source))
        );
        let error = admit_formula(
            source.into(),
            options(),
            ExpansionLimits::default(),
            limits(lower - 1),
        )
        .unwrap_err();
        assert!(
            matches!(error, FormulaFailure::Limit { resource: actual, limit, observed, .. } if actual == resource && limit == u128::from(lower-1) && observed == u128::from(lower)),
            "{error}"
        );
        let location = error.diagnostics()[0].primary().location;
        assert_eq!(location.source, SOURCE);
        assert_eq!(input(source).source().slice(location.span).unwrap(), source);
        println!("inclusive_{resource:?}={lower}");
    }
}

#[test]
fn scalar_storage_limit_is_inclusive() {
    let source = "q(M):-N=#count{},Y=N+1,M=#sum{Y}.";
    let limits = |ceiling| ExpansionLimits {
        max_scalar_bytes: ceiling,
        ..Default::default()
    };
    let mut lower = 0;
    let mut upper = 16_384;
    assert!(
        admit_formula(
            source.into(),
            options(),
            limits(upper),
            FormulaLimits::default()
        )
        .is_ok()
    );
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if admit_formula(
            source.into(),
            options(),
            limits(middle),
            FormulaLimits::default(),
        )
        .is_ok()
        {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    assert!(lower > 0);
    assert_eq!(
        native(
            &admit_formula(
                source.into(),
                options(),
                limits(lower),
                FormulaLimits::default()
            )
            .unwrap()
        ),
        native(&input(source))
    );
    let error = admit_formula(
        source.into(),
        options(),
        limits(lower - 1),
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Expansion(ExpansionFailure::Limit { resource: ExpansionResource::ScalarBytes, limit, observed, .. }) if limit == (lower-1) as u128 && observed == lower as u128),
        "{error}"
    );
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    println!("inclusive_scalar_bytes={lower}");
}
