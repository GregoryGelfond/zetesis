//! Complete aggregate proposals enter scoped original conditional implications.

use crate::support::objective_dependency_records as objective_dependencies;

mod cases;
use crate::support::finite_bindings as reference;

use std::collections::BTreeSet;

use cases::CASES;
use reference::{Models, exhaustive, external, holds, native, values};
use themelios_base::source::SourceId;
use zetesis_reference_support::canonical;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, FormulaFailure,
    FormulaLimits, FormulaResource, admit_formula, prepare_formula,
};

const SOURCE: SourceId = SourceId::new(163);

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
fn original_sources_remain_owned() {
    for &(source, _) in CASES {
        assert_eq!(input(source).source().text(), source);
    }
}

#[test]
fn grounding_limits_are_inclusive() {
    let source = "{d;p(1)}.q(N):-N=#count{1:d};p(X):X=1..N.";
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
fn possible_conditions_do_not_establish_truth() {
    let source = "{d;p(0)}.q(N):-N=#count{};p(N):d.";
    let expected = Models::from([
        BTreeSet::from(["q(0)".into()]),
        BTreeSet::from(["p(0)".into(), "q(0)".into()]),
        BTreeSet::from(["d".into()]),
        BTreeSet::from(["d".into(), "p(0)".into(), "q(0)".into()]),
    ]);
    assert_eq!(native(&input(source)), expected);
    // d is possible in every proposal row; only the interpretation decides it.
    assert_ne!(native(&input("{d;p(0)}.q(0):-p(0).")), expected);
}

#[test]
fn conditional_success_does_not_erase_equalities() {
    let source = "{d}.q(N):-N=#count{1:d};p(N):missing.";
    let expected = Models::from([
        BTreeSet::from(["q(0)".into()]),
        BTreeSet::from(["d".into(), "q(1)".into()]),
    ]);
    assert_eq!(native(&input(source)), expected);
    assert_ne!(native(&input("{d}.q(0).q(1).")), expected);
}

#[test]
fn consequent_polarity_changes_frozen_support() {
    let expected = Models::from([BTreeSet::new(), BTreeSet::from(["p(0)".into()])]);
    assert_eq!(
        native(&input("p(N):-N=#count{};not not p(N):#true.")),
        expected
    );
    assert_ne!(native(&input("p(0):-0=#count{};p(0):#true.")), expected);
}

#[test]
fn source_order_preserves_completed_consumers() {
    let instructions = ["N=#count{}", "Y=N+1", "p(Y):d"];
    let expected = native(&input("{d;p(1)}.q(1):-0=#count{};p(1):d."));
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let body = order.map(|index| instructions[index]).join(";");
        assert_eq!(
            native(&input(&format!("{{d;p(1)}}.q(Y):-{body}."))),
            expected
        );
    }
}

#[test]
fn local_scopes_restart_for_equal_outer_proposals() {
    let source = "i(0..1).{d(0);p(0)}.q(I,N):-i(I),N=#count{};p(N):d(I).";
    let expanded = "i(0..1).{d(0);p(0)}.q(0,0):-0=#count{};p(0):d(0).q(1,0):-0=#count{};p(0):d(1).";
    let actual = native(&input(source));
    assert_eq!(actual, native(&input(expanded)));
    assert_eq!(actual.len(), 4);
    assert!(actual.iter().all(|model| model.contains("q(1,0)")));
    assert!(actual.iter().any(|model| !model.contains("q(0,0)")));
}

#[test]
fn local_scopes_cannot_repair_unsafe_outer_bindings() {
    for source in [
        "q(X):-N=#count{};p(X):d(X).",
        "q(N):-N=#count{};p(N):not d(X).",
        "q(N):-N=#count{};not p(N,X):d.",
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
fn conditional_consumers_preserve_scored_answers() {
    for source in [
        "q(N):-N=#count{};p(N):d.#minimize{1,N:q(N)}.",
        "{a}:-N=#count{};not p(N):d.#minimize{1:a}.",
        "#count{1:a}:-N=#count{};not not p(N):d.#minimize{1:a}.",
    ] {
        objective_dependencies::check(source);
    }
}

#[test]
fn unrelated_objectives_keep_their_priority() {
    let source = "d.q(N):-N=#count{};p(N):d.#minimize{1@7:d}.";
    assert_eq!(input(source).objectives().priorities(), &[7]);
}

#[test]
fn possible_conditions_cannot_hide_undefined_arguments() {
    let source = "{d}.q(N):-N=#count{};p(1/N):d.";
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
        "{error}"
    );
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
}
