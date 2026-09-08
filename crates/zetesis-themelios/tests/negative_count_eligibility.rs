//! Negated count eligibility retains its original candidate and reduct meaning.

#[path = "support/finite_bindings.rs"]
mod reference;

use std::collections::BTreeSet;

use reference::{Models, atom_text, exhaustive, external, holds, native, values};
use themelios_base::source::SourceId;
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits,
    ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature,
    admit_formula,
};

const SOURCE: SourceId = SourceId::new(139);

// Tuple identity has a checked bijection with complete head identity. The
// comparison source uses ordinary choice eligibility, not a new aggregate
// assignment or candidate-dependent filter in the reference.
const CASES: &[(&str, &str)] = &[
    ("1#count{1:a:not b}1.", "1{a:not b}1."),
    ("1#count{1:a:not not b}1.", "1{a:not not b}1."),
    ("{b}.1#count{1:a:not b}1.", "{b}.1{a:not b}1."),
    ("{b}.1#count{1:a:not not b}1.", "{b}.1{a:not not b}1."),
    ("1#count{1:a:not a}1.", "1{a:not a}1."),
    ("1#count{1:a:not not a}1.", "1{a:not not a}1."),
    ("#count{1:a:not a}.", "{a:not a}."),
    ("#count{1:a:not not a}.", "{a:not not a}."),
    ("b:-a.1#count{1:a:not b}1.", "b:-a.1{a:not b}1."),
    ("b:-a.1#count{1:a:not not b}1.", "b:-a.1{a:not not b}1."),
    ("{e;b}.1#count{1:a:not b}1:-e.", "{e;b}.1{a:not b}1:-e."),
    ("a.{b}.0#count{1:a:not b}0.", "a.{b}.0{a:not b}0."),
    (
        "{b}.1#count{1:a:not b;2:c:not not b}1.",
        "{b}.1{a:not b;c:not not b}1.",
    ),
    (
        "{b;c}.1#count{1:a:not b;1:a:not not c}1.",
        "{b;c}.1{a:not b;a:not not c}1.",
    ),
    ("b:-a.1#count{1:a:b;1:a:not b}1.", "b:-a.1{a:b;a:not b}1."),
    (
        "d(1..2).{b(1)}.1#count{X:a(X):d(X),not b(X)}1.",
        "d(1..2).{b(1)}.1{a(X):d(X),not b(X)}1.",
    ),
    (
        "d(1..2).{b(1)}.1#count{X:a(X):not b(X),d(X)}1.",
        "d(1..2).{b(1)}.1{a(X):d(X),not b(X)}1.",
    ),
    (
        "d(1..2).{b(1);b(2)}.1#count{X:a(X):d(X),not not b(X)}1.",
        "d(1..2).{b(1);b(2)}.1{a(X):d(X),not not b(X)}1.",
    ),
    (
        "{b(1)}.1#count{X:a(X):X=1..2,not b(X)}1.",
        "{b(1)}.1{a(X):X=1..2,not b(X)}1.",
    ),
    (
        "{b(2)}.1#count{X:a(X):not b(X+1),X=1}1.",
        "{b(2)}.1{a(1):not b(2)}1.",
    ),
    (
        "{b(f(2))}.1#count{X:a(X):X=1,not not b(f(X+1))}1.",
        "{b(f(2))}.1{a(1):not not b(f(2))}1.",
    ),
    (
        "{-b(1)}.1#count{X: -a(X):X=1,not -b(X)}1.",
        "{-b(1)}.1{-a(1):not -b(1)}1.",
    ),
    (
        "{-b(1)}.1#count{X: -a(X):X=1,not not -b(X)}1.",
        "{-b(1)}.1{-a(1):not not -b(1)}1.",
    ),
    ("1#count{1:a:not b(_)}1.", "1{a:not b(_)}1."),
    (
        "{b(1);b(2)}.1#count{1:a:not b(_)}1.",
        "{b(1);b(2)}.1{a:not b(_)}1.",
    ),
    (
        "{b(1);b(2)}.1#count{1:a:not not b(_)}1.",
        "{b(1);b(2)}.1{a:not not b(_)}1.",
    ),
    (
        "d(1).{b(1,2)}.1#count{X:a(X):d(X),not b(X,_)}1.",
        "d(1).{b(1,2)}.1{a(X):d(X),not b(X,_)}1.",
    ),
    (
        "{b(1);d}.Y#count{1:a:not b(N)}Y:-N=#count{1:d},Y=N+1.",
        "{b(1);d}.1{a:not b(0)}1:-0=#count{1:d}.2{a:not b(1)}2:-1=#count{1:d}.",
    ),
    (
        "{b(1);d}.Y#count{1:a:not not b(N+1)}Y:-N=#count{1:d},Y=N+1.",
        "{b(1);d}.1{a:not not b(1)}1:-0=#count{1:d}.2{a:not not b(2)}2:-1=#count{1:d}.",
    ),
    (
        "d(1..2).{b}.1#count{1:a:d(X),not b}1.",
        "d(1..2).{b}.1{a:d(X),not b}1.",
    ),
];

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
fn models_match_ordinary_choice_eligibility() {
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
fn frozen_truth_matches_ordinary_choice_eligibility() {
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
            let candidate = values(left.theory(), outer, None);
            let reference = values(right.theory(), remap(outer), None);
            assert_eq!(
                holds(left.theory(), &candidate),
                holds(right.theory(), &reference),
                "{source}"
            );
            for inner in 0..1 << names.len() {
                assert_eq!(
                    holds(
                        left.theory(),
                        &values(left.theory(), inner, Some(&candidate))
                    ),
                    holds(
                        right.theory(),
                        &values(right.theory(), remap(inner), Some(&reference))
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
fn tautological_eligibility_cannot_be_erased() {
    let admitted = input("b:-a.1#count{1:a:b;1:a:not b}1.");
    assert!(native(&admitted).is_empty());
    assert_eq!(
        native(&input("b:-a.1{a}1.")),
        Models::from([BTreeSet::from(["a".to_owned(), "b".to_owned()]),])
    );
}

#[test]
fn double_negation_cannot_be_replaced_by_positive_truth() {
    assert_eq!(
        native(&input("1#count{1:a:not not a}1.")),
        Models::from([BTreeSet::from(["a".to_owned()]),])
    );
    assert!(native(&input("1#count{1:a:a}1.")).is_empty());
}

#[test]
fn negative_conditions_cannot_hide_count_aliases() {
    for source in [
        "{b}.1#count{1:a:b;1:c:not b}1.",
        "{b}.1#count{1:a:not b;2:a:not not b}1.",
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
                FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
                    feature: ProfileFeature::HeadAggregateAlias,
                    ..
                }))
            ),
            "{source}: {error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    }
}

#[test]
fn negated_conditions_cannot_supply_local_bindings() {
    for source in [
        "1#count{X:a:not b(X)}1.",
        "1#count{X:a:not not b(X)}1.",
        "1#count{1:a:not -b(_)}1.",
    ] {
        let error = admit_formula(
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
fn undefined_negative_arguments_refuse_admission() {
    let error = admit_formula(
        "1#count{1:a:X=0,not b(1/X)}1.".into(),
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
fn negative_eligibility_limits_are_inclusive() {
    let source = "d(1..2).{b(1)}.1#count{X:a(X):d(X),not b(X)}1.";
    for resource in [FormulaResource::Work, FormulaResource::Substitutions] {
        let limits = |ceiling| {
            let mut result = FormulaLimits::default();
            match resource {
                FormulaResource::Work => result.max_work = ceiling,
                FormulaResource::Substitutions => result.max_substitutions = ceiling,
                _ => unreachable!(),
            }
            result
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
            matches!(error, FormulaFailure::Limit { resource: actual, limit, observed, .. }
            if actual == resource && limit == u128::from(lower-1) && observed == u128::from(lower)),
            "{error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
        println!("inclusive_{resource:?}={lower}");
    }
}

#[test]
fn scalar_storage_limits_are_inclusive() {
    let source = "{b}.1#count{1:a:not b}1.";
    let mut lower = 0;
    let mut upper = 16_384;
    let limits = |max_scalar_bytes| ExpansionLimits {
        max_scalar_bytes,
        ..Default::default()
    };
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
        matches!(error, FormulaFailure::Expansion(ExpansionFailure::Limit { resource: ExpansionResource::ScalarBytes, limit, observed, .. })
        if limit == (lower-1) as u128 && observed == lower as u128),
        "{error}"
    );
    println!("inclusive_ScalarBytes={lower}");
}
