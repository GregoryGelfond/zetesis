//! Dependent outer ranges retain complete rows and original frozen equalities.

use crate::support::admission;
use crate::support::limits;
use zetesis_test_support::thresholds::first_success;
mod cases;
use crate::support::finite_bindings as reference;

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::support::objective_boundaries;
use cases::CASES;
use reference::{Models, holds, native, values};
use themelios_base::source::SourceId;
use zetesis_reference_support::canonical;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, admit_formula, prepare_formula,
};

const SOURCE: SourceId = SourceId::new(151);

fn options() -> AdmissionOptions {
    admission::options(SOURCE)
}

fn input(source: &str) -> AdmittedFormula {
    admission::admitted(SOURCE, source)
}

#[test]
fn models_match_finite_substitutions() {
    reference::models_match_substitutions(CASES.iter().chain(cases::BOUNDARIES).copied(), input);
}

#[test]
fn stability_matches_subset_enumeration() {
    reference::stability_matches_exhaustion(
        CASES
            .iter()
            .chain(cases::BOUNDARIES)
            .map(|&(source, _)| source),
        input,
    );
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
#[ignore = "requires clingo: original sources match clingo full models"]
fn original_sources_match_clingo_full_models() {
    reference::original_sources_match_clingo_full_models(
        CASES.iter().map(|&(source, _)| source),
        input,
    );
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
    reference::sources_remain_owned(
        CASES
            .iter()
            .chain(cases::BOUNDARIES)
            .map(|&(source, _)| source),
        input,
    );
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
        let lower = first_success(limits::formula_default(resource), |middle| {
            admit_formula(
                source.into(),
                options(),
                ExpansionLimits::default(),
                limits::formula(resource, middle),
            )
            .is_ok()
        });
        assert_eq!(
            native(
                &admit_formula(
                    source.into(),
                    options(),
                    ExpansionLimits::default(),
                    limits::formula(resource, lower)
                )
                .unwrap()
            ),
            native(&input(source))
        );
        let error = admit_formula(
            source.into(),
            options(),
            ExpansionLimits::default(),
            limits::formula(resource, lower - 1),
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
    let lower = first_success(
        limits::expansion_default(ExpansionResource::ScalarBytes),
        |middle| {
            admit_formula(
                source.into(),
                options(),
                limits::expansion(ExpansionResource::ScalarBytes, middle),
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
                limits::expansion(ExpansionResource::ScalarBytes, lower),
                FormulaLimits::default()
            )
            .unwrap()
        ),
        native(&input(source))
    );
    let error = admit_formula(
        source.into(),
        options(),
        limits::expansion(ExpansionResource::ScalarBytes, lower - 1),
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
