//! Completed outer values select original negative formulas, never support tests.

#[path = "support/outer_negative_consumers.rs"]
mod cases;
#[path = "support/finite_bindings.rs"]
mod reference;

use std::collections::BTreeSet;

use cases::CASES;
use reference::{Models, atom_text, exhaustive, external, holds, native, values};
use themelios_base::source::SourceId;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, admit_formula, prepare_formula,
};

const SOURCE: SourceId = SourceId::new(149);

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
        let names: Vec<_> = left.atoms().iter().map(atom_text).collect();
        let other: Vec<_> = right.atoms().iter().map(atom_text).collect();
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
fn negative_gates_cannot_read_possible_support() {
    let source = "{p(0)}.q(N):-N=#count{},not p(N).";
    let expected = Models::from([
        BTreeSet::from(["p(0)".into()]),
        BTreeSet::from(["q(0)".into()]),
    ]);
    assert_eq!(native(&input(source)), expected);
    // p(0) is possible in both candidates. Treating possibility as truth
    // would discard the q(0) clause even when the candidate omits p(0).
    assert_ne!(native(&input("{p(0)}.")), expected);
}

#[test]
fn double_negation_cannot_be_replaced_by_positive_truth() {
    let expected = Models::from([BTreeSet::new(), BTreeSet::from(["p(0)".into()])]);
    assert_eq!(native(&input("p(N):-N=#count{},not not p(N).")), expected);
    assert_ne!(native(&input("p(0):-0=#count{},p(0).")), expected);
}

#[test]
fn negative_success_cannot_replace_original_equalities() {
    let source = "{d}.q(N):-N=#count{1:d},not p(N).";
    let unguarded = "{d}.q(0):-not p(0).q(1):-not p(1).";
    assert_ne!(native(&input(source)), native(&input(unguarded)));
}

#[test]
fn negative_atoms_cannot_supply_missing_bindings() {
    for source in [
        "q(N):-N=#count{},not p(N,X).",
        "q(N):-N=#count{},not not p(N,X).",
        "q(N):-N=#count{},not p(X,_).",
        "q(N):-N=#count{},not -p(N,_).",
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
fn conditionals_consume_completed_outer_values() {
    for (source, expanded) in [
        ("q(N):-N=#count{},p(N):d.", "q(0):-0=#count{},p(0):d."),
        (
            "q(N):-N=#count{},not p(N):d.",
            "q(0):-0=#count{},not p(0):d.",
        ),
    ] {
        assert_eq!(native(&input(source)), native(&input(expanded)));
        assert_eq!(input(source).source().text(), source);
    }
}

#[test]
fn negative_consumers_preserve_scored_answers() {
    for source in [
        "q(N):-N=#count{},not p(N).#minimize{1,N:q(N)}.",
        "{q}:-N=#count{},not not p(N).#minimize{1:q}.",
        "#count{1:q}:-N=#count{},not p(N,_).#minimize{1:q}.",
    ] {
        objective_boundaries::check(source);
    }
}
#[path = "support/objective_boundaries.rs"]
mod objective_boundaries;
#[path = "support/source_records.rs"]
mod source_records;

#[test]
fn false_gates_cannot_hide_undefined_arguments() {
    let source = "p(0).q(N):-N=#count{},not p(N),not p(1/N).";
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

#[test]
fn original_sources_remain_owned() {
    for &(source, _) in CASES {
        assert_eq!(input(source).source().text(), source);
    }
}

#[test]
fn grounding_limits_are_inclusive() {
    let source = "{p(0,a);p(0,b)}.q(Y):-N=#count{},Y=N+1,not p(N,_).";
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
    let source = "{p(0,a)}.q(N):-N=#count{},not p(N,_).";
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
