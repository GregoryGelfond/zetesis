//! Outer body substitutions preserve complete choice/count groups and reducts.

#[path = "support/objective_dependency_records.rs"]
mod objective_dependencies;

#[path = "support/choice_consumers.rs"]
mod cases;
#[path = "support/finite_bindings.rs"]
mod reference;

use std::collections::BTreeSet;
use std::fmt::Write as _;

use cases::CASES;
use reference::{Models, atom_text, exhaustive, external, holds, native, values};
use themelios_base::source::SourceId;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, admit_formula, prepare_formula,
};

const SOURCE: SourceId = SourceId::new(137);

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
            // Arbitrary M/J includes both proper subsets and unrelated worlds.
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
fn proposal_success_cannot_replace_original_equality() {
    let source = "a.Y{a;b}Y:-N=#count{1:a},Y=N+1.";
    let expected = Models::from([BTreeSet::from(["a".to_owned(), "b".to_owned()])]);
    assert_eq!(native(&input(source)), expected);
    // Both finite proposals are generated, but only N=1 realizes its equality.
    let unguarded = "a.1{a;b}1.2{a;b}2.";
    assert_ne!(native(&input(unguarded)), expected);
}

#[test]
fn possible_argument_support_cannot_replace_truth() {
    let source = "{p(1)}.1{a}1:-N=#count{},p(N+1).";
    let expected = Models::from([
        BTreeSet::new(),
        BTreeSet::from(["p(1)".to_owned(), "a".to_owned()]),
    ]);
    assert_eq!(native(&input(source)), expected);
    assert_ne!(native(&input("{p(1)}.1{a}1.")), expected);
}

#[test]
fn outer_activation_guards_the_complete_group() {
    let source = "{e}.Y{a;b}Y:-e,N=#count{},Y=N+1.";
    let expected = Models::from([
        BTreeSet::new(),
        BTreeSet::from(["e".to_owned(), "a".to_owned()]),
        BTreeSet::from(["e".to_owned(), "b".to_owned()]),
    ]);
    assert_eq!(native(&input(source)), expected);
    assert_ne!(native(&input("{e}.1{a}1:-e.1{b}1:-e.")), expected);
}

#[test]
fn conditional_consumers_preserve_complete_groups() {
    for (body, expanded) in [
        ("N=#count{},not p(N):d", "0=#count{},not p(0):d"),
        ("N=#count{},Y=N+1,p(Y):d", "0=#count{},p(1):d"),
    ] {
        for head in ["{a}", "#count{1:a}"] {
            assert_eq!(
                native(&input(&format!("{head}:-{body}."))),
                native(&input(&format!("{head}:-{expanded}."))),
            );
        }
    }
}

#[test]
fn negative_count_eligibility_uses_outer_consumers() {
    for condition in ["not b", "not not b"] {
        assert_eq!(
            native(&input(&format!(
                "{{b}}.Y#count{{1:a:{condition}}}Y:-N=#count{{}},Y=N+1."
            ))),
            native(&input(&format!("{{b}}.1{{a:{condition}}}1:-0=#count{{}}."))),
        );
    }
}

#[test]
fn choice_consumers_preserve_scored_answers() {
    for source in [
        "Y{p}Y:-N=#count{},Y=N+1.#minimize{1:p}.",
        "{p}:-N=#count{},N>0.#minimize{1:p}.",
        "Y#count{1:p}Y:-N=#count{},Y=N+1.#minimize{1:p}.",
        "p(1).{a}:-N=#count{},p(N+1).#minimize{1:a}.",
    ] {
        objective_dependencies::check(source);
    }
}

#[test]
fn count_aliases_consume_completed_outer_bindings() {
    for elements in ["1:a;1:b", "1:a;2:a", "1:a:b;2:a:c"] {
        let source = format!("{{b;c}}.Y#count{{{elements}}}Y:-N=#count{{}},Y=N+1.");
        assert_eq!(
            native(&input(&source)),
            native(&input(&format!("{{b;c}}.1#count{{{elements}}}1."))),
            "{source}"
        );
    }
}

#[test]
fn local_witnesses_cannot_bind_outer_inputs() {
    for head in ["Y{p:X=1}", "Y#count{1:p:X=1}"] {
        let source = format!("{head}:-N=#count{{}},Y=N+X.");
        let error = prepare_formula(
            source.clone(),
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
fn captured_arguments_cannot_bind_missing_inputs() {
    for head in ["Y{a}Y", "Y#count{1:a}Y"] {
        let source = format!("p(1).{head}:-N=#count{{}},Y=N+1,p(X+1).");
        let error = prepare_formula(
            source.clone(),
            options(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, FormulaFailure::UnboundArgumentInput { .. }),
            "{source}: {error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    }
}

#[test]
fn cyclic_outer_consumers_remain_refused() {
    for head in ["Y{a}", "Y#count{1:a}"] {
        let source = format!("{head}:-N=#count{{X:p(X,Y)}},Y=N+1.");
        let error = prepare_formula(
            source.clone(),
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
fn false_filters_cannot_hide_undefined_consumers() {
    for head in ["Y{a}Y", "Y#count{1:a}Y"] {
        let source = format!("{{d}}.{head}:-N=#count{{1:d}},N>0,Y=1/N.");
        let error = admit_formula(
            source.clone(),
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
    for &(source, _) in CASES {
        assert_eq!(input(source).source().text(), source);
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(32))]
    #[test]
    fn affine_bounds_match_finite_substitutions(offset in -2_i32..3, factor in 1_i32..3) {
        let source = format!("{{d}}.Y{{a;b}}Y:-Y=Z*{factor},Z=N+({offset}),N=#count{{1:d}}.");
        let mut expanded = "{d}.".to_owned();
        for count in 0..=1 {
            let bound = (count + offset) * factor;
            write!(expanded, "{bound}{{a;b}}{bound}:-{count}=#count{{1:d}}.").unwrap();
        }
        proptest::prop_assert_eq!(native(&input(&source)), native(&input(&expanded)));
    }
}

#[test]
fn preparation_limits_are_inclusive() {
    for source in [
        "Y{p}Y:-N=#count{},Y=N+1.",
        "Y#count{1:p}Y:-N=#count{},Y=N+1.",
    ] {
        for resource in [ExpansionResource::TermWork, ExpansionResource::ScalarBytes] {
            let limits = |ceiling| {
                let mut result = ExpansionLimits::default();
                match resource {
                    ExpansionResource::TermWork => result.max_term_work = ceiling,
                    ExpansionResource::ScalarBytes => result.max_scalar_bytes = ceiling,
                    _ => unreachable!(),
                }
                result
            };
            let mut lower = 0;
            let mut upper = 16_384;
            assert!(
                prepare_formula(
                    source.into(),
                    options(),
                    limits(upper),
                    FormulaLimits::default()
                )
                .is_ok()
            );
            while lower < upper {
                let middle = lower + (upper - lower) / 2;
                if prepare_formula(
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
            assert!(
                prepare_formula(
                    source.into(),
                    options(),
                    limits(lower),
                    FormulaLimits::default()
                )
                .is_ok()
            );
            let error = prepare_formula(
                source.into(),
                options(),
                limits(lower - 1),
                FormulaLimits::default(),
            )
            .unwrap_err();
            assert!(
                matches!(error, FormulaFailure::Expansion(ExpansionFailure::Limit {
                resource: actual, limit, observed, ..
            }) if actual == resource && limit == (lower-1) as u128 && observed == lower as u128),
                "{error}"
            );
            assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
            println!("{source} inclusive_preparation_{resource:?}={lower}");
        }
    }
}

#[test]
fn grounding_limits_are_inclusive() {
    for source in [
        "{d}.Y{p}Y:-N=#count{1:d},Y=N+1.",
        "{d}.Y#count{1:p}Y:-N=#count{1:d},Y=N+1.",
    ] {
        for resource in [
            FormulaResource::AssignmentValues,
            FormulaResource::Substitutions,
            FormulaResource::Work,
        ] {
            let limits = |ceiling| {
                let mut result = FormulaLimits::default();
                match resource {
                    FormulaResource::AssignmentValues => {
                        result.max_assignment_values = usize::try_from(ceiling).unwrap();
                    }
                    FormulaResource::Substitutions => result.max_substitutions = ceiling,
                    FormulaResource::Work => result.max_work = ceiling,
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
                matches!(error, FormulaFailure::Limit {
                resource: actual, limit, observed, ..
            } if actual == resource && limit == u128::from(lower-1) && observed == u128::from(lower)),
                "{error}"
            );
            assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
            println!("{source} inclusive_grounding_{resource:?}={lower}");
        }
    }
}
