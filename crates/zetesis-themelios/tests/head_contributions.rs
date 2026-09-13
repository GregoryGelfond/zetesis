//! Complete source contracts for neutral aggregate-head elements.

#[path = "support/head_contributions.rs"]
mod cases;
#[path = "support/finite_bindings.rs"]
mod reference;

use std::collections::BTreeSet;

use cases::{CASES, CLINGO_DIFFERENCES};
use reference::{Models, atom_text, exhaustive, external, holds, native, values};
use themelios_base::source::SourceId;
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits,
    ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature,
    admit_formula,
};

const SOURCE: SourceId = SourceId::new(317);

fn limited(
    source: &str,
    expansion: ExpansionLimits,
    formula: &FormulaLimits,
) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions {
            source_id: SOURCE,
            ..Default::default()
        },
        expansion,
        *formula,
    )
}

fn input(source: &str) -> AdmittedFormula {
    limited(
        source,
        ExpansionLimits::default(),
        &FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}

#[test]
fn source_models_match_explicit_permissions() {
    for &(source, expanded) in CASES {
        assert_eq!(native(&input(source)), native(&input(expanded)), "{source}");
    }
}

#[test]
fn source_stability_matches_subset_enumeration() {
    for &(source, _) in CASES {
        let admitted = input(source);
        assert_eq!(native(&admitted), exhaustive(&admitted), "{source}");
    }
}

#[test]
fn source_reducts_match_explicit_permissions() {
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
        assert!(names.len() <= 7, "finite exhaustive interpretation pairs");
        let remap = |mask: usize| {
            names.iter().enumerate().fold(0, |result, (index, name)| {
                result
                    | (usize::from(mask & (1 << index) != 0)
                        << other.iter().position(|other| other == name).unwrap())
            })
        };
        for outer in 0..1 << names.len() {
            let original = values(left.theory(), outer, None);
            let explicit = values(right.theory(), remap(outer), None);
            assert_eq!(
                holds(left.theory(), &original),
                holds(right.theory(), &explicit)
            );
            for inner in 0..1 << names.len() {
                assert_eq!(
                    holds(
                        left.theory(),
                        &values(left.theory(), inner, Some(&original))
                    ),
                    holds(
                        right.theory(),
                        &values(right.theory(), remap(inner), Some(&explicit))
                    ),
                    "{source}: M={outer}, J={inner}"
                );
            }
        }
    }
}

#[test]
fn reference_differences_identify_unique_sources() {
    let sources: BTreeSet<_> = CASES.iter().map(|(source, _)| source).collect();
    assert_eq!(sources.len(), CASES.len());
    let differences: BTreeSet<_> = CLINGO_DIFFERENCES
        .iter()
        .map(|(source, _)| source)
        .collect();
    assert_eq!(differences.len(), CLINGO_DIFFERENCES.len());
    assert!(differences.is_subset(&sources));
}

#[test]
fn satisfied_facts_preserve_numeric_permissions() {
    for (source, facts, selected) in [
        ("0#sum+{-2:a;2:b}0.", "a.", &["a"][..]),
        ("2#sum+{-2:a;2:b}2.", "a.b.", &["a", "b"][..]),
    ] {
        let selected = selected.iter().map(|atom| (*atom).to_owned()).collect();
        assert!(native(&input(source)).contains(&selected));
        assert!(native(&input(&format!("{facts}{source}"))).contains(&selected));
    }
}

#[test]
fn original_sources_retain_their_identity() {
    for &(source, _) in CASES {
        let admitted = input(source);
        assert_eq!(admitted.source().id(), SOURCE);
        assert_eq!(admitted.source().text(), source);
    }
}

#[test]
fn neutral_tuples_retain_complete_key_limits() {
    let mut limits = FormulaLimits::default();
    limits.aggregate.max_elements = 1;
    let duplicate = limited(
        "#sum{word:a;word:b}=0.",
        ExpansionLimits::default(),
        &limits,
    )
    .unwrap();
    assert_eq!(native(&duplicate), native(&input("{a;b}.")));
    let error = limited(
        "#sum{word,1:a;word,2:b}=0.",
        ExpansionLimits::default(),
        &limits,
    )
    .expect_err("two source keys still require bounded storage");
    assert!(matches!(
        error,
        FormulaFailure::Limit {
            resource: FormulaResource::AggregateElements,
            limit: 1,
            observed: 2,
            ..
        }
    ));
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
}

#[test]
fn missing_extrema_retain_complete_key_limits() {
    for (function, empty) in [("#min", "#sup"), ("#max", "#inf")] {
        let mut limits = FormulaLimits::default();
        limits.aggregate.max_elements = 1;
        let source = format!("{function}{{:a;:b}}={empty}.");
        let admitted = limited(&source, ExpansionLimits::default(), &limits).unwrap();
        assert_eq!(native(&admitted), native(&input("{a;b}.")));
        limits.aggregate.max_elements = 0;
        let error = limited(&source, ExpansionLimits::default(), &limits).unwrap_err();
        assert!(matches!(
            error,
            FormulaFailure::Limit {
                resource: FormulaResource::AggregateElements,
                limit: 0,
                observed: 1,
                ..
            }
        ));
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
        limits.aggregate.max_elements = 1;
        let mixed = format!("{function}{{:a;{empty}:b}}={empty}.");
        let error = limited(&mixed, ExpansionLimits::default(), &limits).unwrap_err();
        assert!(matches!(
            error,
            FormulaFailure::Limit {
                resource: FormulaResource::AggregateElements,
                limit: 1,
                observed: 2,
                ..
            }
        ));
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
        limits.aggregate.max_elements = 2;
        assert_eq!(
            native(&limited(&mixed, ExpansionLimits::default(), &limits).unwrap()),
            native(&input("{a;b}."))
        );
    }
}

#[test]
fn neutral_rows_cannot_hide_undefined_bindings() {
    for source in [
        "#sum{1/0:a}.",
        "#min{1/0:a}.",
        "d(0).#sum{word:p(Y):d(X),Y=1/X}=0.",
        "d(0).e.#sum{word:p(Y):d(X),Y=1/X,not e}=0.",
        "d(0).#min{:p(Y):d(X),Y=1/X}.",
        "d(0).#min{:p(Y):d(X),Y=1/X}=#sup.",
        "d(0).e.#max{:p(Y):d(X),Y=1/X,not e}=#inf.",
        "#max{1/0:a}=#inf:-#false.",
        "d(0).#sum+{-1:p(Y):d(X),Y=1/X}=0.",
    ] {
        let error = limited(
            source,
            ExpansionLimits::default(),
            &FormulaLimits::default(),
        )
        .expect_err("source evaluation precedes contribution selection");
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
fn neutral_rows_cannot_hide_unsupported_tuple_terms() {
    let error = limited(
        "d(0).#sum{word,1/X:a:d(X)}=0.",
        ExpansionLimits::default(),
        &FormulaLimits::default(),
    )
    .expect_err("unsupported tuple syntax remains a profile failure");
    assert!(matches!(
        error,
        FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
            feature: ProfileFeature::Aggregate,
            ..
        }))
    ));
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
}

#[test]
fn unbounded_extrema_need_no_numeric_envelope() {
    for function in ["#min", "#max"] {
        for value in ["(-2147483647-1)", "2147483647"] {
            let source = format!("{function}{{{value}:a}}.");
            assert_eq!(native(&input(&source)), native(&input("{a}.")), "{source}");
        }
    }
}

#[test]
fn missing_measures_do_not_hide_invalid_present_values() {
    for function in ["#min", "#max"] {
        for value in ["(-2147483647-1)", "2147483647"] {
            let source = format!("{function}{{:a;{value}:b}}<=0:-#false.");
            let error = limited(
                &source,
                ExpansionLimits::default(),
                &FormulaLimits::default(),
            )
            .expect_err("complete values keep the existing endpoint profile");
            assert!(matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Admission(
                    AdmissionFailure::ExtremumEndpoint { .. }
                ))
            ));
            assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
        }
    }
}

#[test]
fn missing_body_extrema_keep_their_admission_boundary() {
    for source in ["a.p:-#min{:a}=#sup.", "a.p:-#max{:a}=#inf."] {
        let error = limited(
            source,
            ExpansionLimits::default(),
            &FormulaLimits::default(),
        )
        .expect_err("the head extension does not change body admission");
        assert!(matches!(
            error,
            FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
                feature: ProfileFeature::Aggregate,
                ..
            }))
        ));
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    }
}

#[test]
fn neutral_rows_cannot_repair_unsafe_variables() {
    for source in [
        "#sum{word:p(X)}=0.",
        "#min{:p(X)}.",
        "#min{:p(X)}=#sup.",
        "#max{:p(X)}=#inf.",
        "#sum+{-1:p(X)}=0.",
    ] {
        let error = limited(
            source,
            ExpansionLimits::default(),
            &FormulaLimits::default(),
        )
        .expect_err("a noncontributing row does not establish a binding");
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{source}: {error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    }
}

fn first_success(mut accepts: impl FnMut(u64) -> bool) -> u64 {
    let mut lower = 0;
    let mut upper = 1;
    assert!(!accepts(lower), "the operation must charge this resource");
    while !accepts(upper) {
        upper *= 2;
        assert!(upper <= 1 << 20, "bounded reference fixture");
    }
    while lower + 1 < upper {
        let middle = lower + (upper - lower) / 2;
        if accepts(middle) {
            upper = middle;
        } else {
            lower = middle;
        }
    }
    upper
}

#[test]
fn selected_rows_obey_inclusive_work_limits() {
    for source in [
        "d(1..3).#sum{word,X:p(X):d(X)}=0.",
        "d(1..3).#sum+{-1,X:p(X):d(X)}=0.",
        "d(1..3).#min{:p(X):d(X)}=#sup.",
        "d(1..3).#max{:p(X):d(X)}=#inf.",
    ] {
        for resource in [FormulaResource::Substitutions, FormulaResource::Work] {
            let attempt = |maximum| {
                let mut limits = FormulaLimits::default();
                match resource {
                    FormulaResource::Substitutions => limits.max_substitutions = maximum,
                    FormulaResource::Work => limits.max_work = maximum,
                    _ => unreachable!(),
                }
                limited(source, ExpansionLimits::default(), &limits)
            };
            let exact = first_success(|maximum| attempt(maximum).is_ok());
            let error = attempt(exact - 1).expect_err("inclusive boundary");
            assert!(
                matches!(error, FormulaFailure::Limit { resource: actual, limit, observed, .. } if actual == resource && limit == u128::from(exact - 1) && observed == u128::from(exact)),
                "{source}: {error}"
            );
            assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
            assert_eq!(native(&attempt(exact).unwrap()), native(&input(source)));
        }
    }
}

#[test]
fn neutral_rows_obey_inclusive_expansion_limits() {
    for source in [
        "d(1..3).#sum{word,X:p(X):d(X)}=0.",
        "d(1..3).#min{:p(X):d(X)}=#sup.",
        "d(1..3).#max{:p(X):d(X)}=#inf.",
    ] {
        for resource in [ExpansionResource::TermWork, ExpansionResource::ScalarBytes] {
            let attempt = |maximum| {
                let mut limits = ExpansionLimits::default();
                match resource {
                    ExpansionResource::TermWork => {
                        limits.max_term_work = usize::try_from(maximum).unwrap();
                    }
                    ExpansionResource::ScalarBytes => {
                        limits.max_scalar_bytes = usize::try_from(maximum).unwrap();
                    }
                    _ => unreachable!(),
                }
                limited(source, limits, &FormulaLimits::default())
            };
            let exact = first_success(|maximum| attempt(maximum).is_ok());
            let error = attempt(exact - 1).expect_err("inclusive source boundary");
            assert!(
                matches!(error, FormulaFailure::Expansion(ExpansionFailure::Limit { resource: actual, limit, observed, .. }) if actual == resource && limit == u128::from(exact - 1) && observed == u128::from(exact)),
                "{error}"
            );
            assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
            assert_eq!(native(&attempt(exact).unwrap()), native(&input(source)));
        }
    }
}

#[test]
#[ignore = "requires an independently installed clingo"]
fn source_comparisons_preserve_complete_records() {
    for &(source, _) in CASES {
        let result = external(source, true);
        assert_eq!(result["Solver"], "clingo version 5.8.2");
        assert_eq!(result["Models"]["More"], "no");
        let mut expected = Models::new();
        let mut count = 0;
        for call in result["Call"].as_array().unwrap() {
            if let Some(witnesses) = call["Witnesses"].as_array() {
                for witness in witnesses {
                    count += 1;
                    assert!(witness.get("Costs").is_none());
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
        let actual = native(&input(source));
        if let Some((_, records)) = CLINGO_DIFFERENCES.iter().find(|(case, _)| *case == source) {
            let recorded: Models = records
                .iter()
                .map(|model| model.iter().map(|atom| (*atom).to_owned()).collect())
                .collect();
            assert_eq!(
                expected, recorded,
                "recorded reference difference: {source}"
            );
            assert_ne!(
                actual, expected,
                "reference difference disappeared: {source}"
            );
        } else {
            assert_eq!(actual, expected, "{source}");
        }
    }
}
