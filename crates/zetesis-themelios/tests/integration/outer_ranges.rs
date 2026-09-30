//! Dependent outer ranges retain complete rows and original frozen equalities.

mod cases;
use crate::support::finite_bindings as reference;

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::support::objective_boundaries;
use cases::CASES;
use reference::{Models, exhaustive, external, holds, native, values};
use themelios_base::source::SourceId;
use zetesis_reference_support::canonical;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, admit_formula, prepare_formula,
};

const SOURCE: SourceId = SourceId::new(151);

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
    for &(source, expanded) in CASES.iter().chain(cases::BOUNDARIES) {
        assert_eq!(native(&input(source)), native(&input(expanded)), "{source}");
    }
}

#[test]
fn stability_matches_subset_enumeration() {
    for &(source, _) in CASES.iter().chain(cases::BOUNDARIES) {
        let admitted = input(source);
        assert_eq!(native(&admitted), exhaustive(&admitted), "{source}");
    }
}

#[test]
fn frozen_truth_matches_finite_substitutions() {
    let mut pairs = 0;
    for &(source, expanded) in CASES.iter().chain(cases::BOUNDARIES) {
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
    println!(
        "source_pairs={} frozen_pairs={pairs}",
        CASES.len() + cases::BOUNDARIES.len()
    );
}

#[test]
#[ignore = "requires an independently installed clingo"]
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
fn range_success_cannot_replace_original_equality() {
    let source = "{d}.q(K):-N=#count{1:d},K=N..N.";
    let expected = Models::from([
        BTreeSet::from(["q(0)".into()]),
        BTreeSet::from(["d".into(), "q(1)".into()]),
    ]);
    assert_eq!(native(&input(source)), expected);
    assert_ne!(native(&input("{d}.q(0).q(1).")), expected);
}

#[test]
fn repeated_predecessor_values_restart_range_cursors() {
    let source = "i(1..2).q(I,K):-i(I),N=#count{},K=N..N+1.";
    let expected = Models::from([BTreeSet::from([
        "i(1)".into(),
        "i(2)".into(),
        "q(1,0)".into(),
        "q(1,1)".into(),
        "q(2,0)".into(),
        "q(2,1)".into(),
    ])]);
    assert_eq!(native(&input(source)), expected);
}

#[test]
fn bound_range_targets_remain_membership_tests() {
    let source = "d(0..2).q(K):-d(K),N=#count{},K=N..N+1.";
    let expected = Models::from([BTreeSet::from([
        "d(0)".into(),
        "d(1)".into(),
        "d(2)".into(),
        "q(0)".into(),
        "q(1)".into(),
    ])]);
    assert_eq!(native(&input(source)), expected);
    assert_ne!(native(&input("d(0..2).q(K):-d(K),0=#count{}.")), expected);
}

#[test]
fn range_width_is_checked_before_enumeration() {
    for (source, width) in [
        ("q(K):-N=#count{},K=N..N+3.", 4_u128),
        (
            "q(K):-N=#count{},K=(-2147483647-1)..(2147483647+N).",
            4_294_967_296,
        ),
    ] {
        let error = admit_formula(
            source.into(),
            options(),
            ExpansionLimits::default(),
            FormulaLimits {
                max_assignment_values: 3,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(
            matches!(error, FormulaFailure::Limit {
            resource: FormulaResource::AssignmentValues, limit: 3, observed, ..
        } if observed == width),
            "{source}: {error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    }
}

#[test]
fn range_membership_cannot_bind_missing_endpoints() {
    for source in [
        "d(1).q(K):-d(K),N=#count{},K=N..X.",
        "q(K):-N=#count{},K=N..X.",
        "q(K):-N=#count{},not p(X),K=N..X.",
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
fn ranges_cannot_supply_cyclic_aggregate_inputs() {
    let source = "q(K):-N=#count{X:p(X,K)},K=N..N.";
    let error = prepare_formula(
        source.into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, FormulaFailure::CyclicValueInput { .. }),
        "{error}"
    );
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
}

#[test]
fn aggregate_generators_consume_complete_range_outputs() {
    let source = "q(M):-N=#count{},K=N..N+1,M=#count{K:p}.";
    let expanded = "q(0):-0=#count{},0=#count{0:p}.q(0):-0=#count{},0=#count{1:p}.";
    assert_eq!(native(&input(source)), native(&input(expanded)));
}

#[test]
fn range_consumers_preserve_scored_answers() {
    for source in [
        "q(K):-N=#count{},K=N..N+1.#minimize{1,K:q(K)}.",
        "K{q}:-N=#count{},K=N..N+1.#minimize{1:q}.",
        "K#count{1:q}:-N=#count{},K=N..N+1.#minimize{1:q}.",
    ] {
        objective_boundaries::check(source);
    }
}

#[test]
fn false_filters_cannot_hide_undefined_endpoints() {
    for source in [
        "q(K):-N=#count{},N>0,K=N..1/N.",
        "q(K):-N=#count{},N>0,K=N..2147483647+1.",
    ] {
        let error = admit_formula(
            source.into(),
            options(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })
            ),
            "{source}: {error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    }
}

#[test]
fn original_sources_remain_owned() {
    for &(source, _) in CASES.iter().chain(cases::BOUNDARIES) {
        assert_eq!(input(source).source().text(), source);
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(32))]
    #[test]
    fn affine_ranges_match_finite_substitutions(offset in -2_i32..3, width in 0_i32..3) {
        let source = format!("{{d}}.q(K):-K=Y..Y+{width},Y=N+({offset}),N=#count{{1:d}}.");
        let mut expanded = "{d}.".to_owned();
        for count in 0..=1 {
            for value in count+offset..=count+offset+width {
                write!(expanded, "q({value}):-{count}=#count{{1:d}}.").unwrap();
            }
        }
        proptest::prop_assert_eq!(native(&input(&source)), native(&input(&expanded)));
    }
}
#[test]
fn grounding_limits_are_inclusive() {
    let source = "q(K):-N=#count{},K=N..N+2.";
    for resource in [
        FormulaResource::AssignmentValues,
        FormulaResource::Substitutions,
        FormulaResource::Work,
    ] {
        let limits = |ceiling| {
            let mut limits = FormulaLimits::default();
            match resource {
                FormulaResource::AssignmentValues => {
                    limits.max_assignment_values = usize::try_from(ceiling).unwrap();
                }
                FormulaResource::Substitutions => limits.max_substitutions = ceiling,
                FormulaResource::Work => limits.max_work = ceiling,
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
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
        println!("inclusive_{resource:?}={lower}");
    }
}

#[test]
fn scalar_storage_limit_is_inclusive() {
    let source = "q(K):-N=#count{},K=N..N+2.";
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
