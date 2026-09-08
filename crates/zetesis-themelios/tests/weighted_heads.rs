//! Weighted head permissions remain independent of numeric contributions.

#[path = "support/weighted_heads.rs"]
mod cases;
#[path = "support/finite_bindings.rs"]
mod reference;

use std::collections::BTreeSet;

use cases::CASES;
use reference::{Models, atom_text, exhaustive, external, holds, native, values};
use themelios_base::source::SourceId;
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits,
    FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature, admit_formula, prepare_formula,
};

const SOURCE: SourceId = SourceId::new(167);

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

fn profile(source: &str, expected: ProfileFeature) {
    let error = prepare_formula(
        source.into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .and_then(zetesis_themelios::PreparedFormula::ground)
    .unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile { feature, .. })) if feature == expected),
        "{source}: {error}"
    );
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
}

#[test]
fn neutral_contributions_keep_head_permission() {
    let expected = Models::from([BTreeSet::new(), BTreeSet::from(["a".into()])]);
    for source in ["0#sum{0:a}0.", "0#sum+{0:a}0."] {
        assert_eq!(native(&input(source)), expected, "{source}");
    }
    assert_ne!(native(&input("")), expected);
}

#[test]
fn negative_eligibility_cannot_hide_weight_errors() {
    for function in ["#sum", "#sum+"] {
        for weight in ["s", "#inf", "#sup", "f(1)", "(1,2)"] {
            profile(
                &format!("d.0{function}{{{weight}:a:not d}}0."),
                ProfileFeature::HeadAggregateWeight,
            );
        }
    }
}

#[test]
fn neutral_contributions_cannot_hide_aliases() {
    for source in [
        "0#sum{0:a;0:b}0.",
        "0#sum+{0:a;0:b}0.",
        "0#sum+{0,k:a;0,l:a}0.",
        "d.0#sum{0,k:a:not d;0,k:b:not d}0.",
    ] {
        profile(source, ProfileFeature::HeadAggregateAlias);
    }
}

#[test]
fn weighted_heads_retain_objective_restrictions() {
    for function in ["#sum", "#sum+"] {
        profile(
            &format!("1{function}{{1:a}}1.#minimize{{1:a}}."),
            ProfileFeature::ObjectiveAggregateDependency,
        );
    }
}

#[test]
fn unsupported_head_forms_keep_located_refusals() {
    for source in ["0#min{0:a}0.", "0#max{0:a}0."] {
        profile(source, ProfileFeature::Head);
    }
    for source in ["0#sum{0:not a}0.", "0#sum+{0:not not a}0."] {
        profile(source, ProfileFeature::NegatedHead);
    }
}

#[test]
fn original_sources_remain_owned() {
    for &(source, _) in CASES {
        assert_eq!(input(source).source().text(), source);
    }
}

#[test]
fn grounding_limits_are_inclusive() {
    let source = "d(1..2).0#sum+{0,X:p(X):d(X)}0.";
    for resource in [
        FormulaResource::AggregateElements,
        FormulaResource::Substitutions,
        FormulaResource::Work,
    ] {
        let limits = |ceiling| {
            let mut limits = FormulaLimits::default();
            match resource {
                FormulaResource::AggregateElements => {
                    limits.aggregate.max_elements = usize::try_from(ceiling).unwrap();
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

fn manual_truth(
    formula: &serde_json::Value,
    outer: &BTreeSet<String>,
    inner: Option<&BTreeSet<String>>,
) -> bool {
    if inner.is_some() && !manual_truth(formula, outer, None) {
        return false;
    }
    match formula {
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::String(atom) => inner.unwrap_or(outer).contains(atom),
        serde_json::Value::Array(parts) => {
            assert_eq!(parts.len(), 3);
            let left = manual_truth(&parts[1], outer, inner);
            let right = manual_truth(&parts[2], outer, inner);
            match parts[0].as_str().unwrap() {
                "and" => left && right,
                "or" => left || right,
                "imp" => !left || right,
                tag => panic!("unreviewed test formula: {tag}"),
            }
        }
        other => panic!("unreviewed test formula: {other}"),
    }
}

#[test]
fn numeric_head_truth_matches_independent_formulas() {
    use serde_json::json;
    for (source, roots) in [
        ("0#sum{0:a}0.", json!([["or", "a", ["imp", "a", false]]])),
        ("0#sum+{0:a}0.", json!([["or", "a", ["imp", "a", false]]])),
        (
            "1#sum{1:a;2:b}1.",
            json!([
                ["or", "a", ["imp", "a", false]],
                ["or", "b", ["imp", "b", false]],
                [
                    "imp",
                    ["imp", ["and", "a", ["imp", "b", false]], false],
                    false
                ]
            ]),
        ),
    ] {
        let admitted = input(source);
        let world = |mask: usize| {
            admitted
                .atoms()
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, atom)| atom_text(atom))
                .collect::<BTreeSet<_>>()
        };
        let roots = roots.as_array().unwrap();
        for outer in 0..1 << admitted.atoms().len() {
            let original = values(admitted.theory(), outer, None);
            let m = world(outer);
            assert_eq!(
                holds(admitted.theory(), &original),
                roots.iter().all(|root| manual_truth(root, &m, None)),
                "{source}: M={outer}"
            );
            for inner in 0..1 << admitted.atoms().len() {
                let j = world(inner);
                assert_eq!(
                    holds(
                        admitted.theory(),
                        &values(admitted.theory(), inner, Some(&original))
                    ),
                    roots.iter().all(|root| manual_truth(root, &m, Some(&j))),
                    "{source}: M={outer}, J={inner}"
                );
            }
        }
    }
}

#[test]
fn negative_sumplus_weights_have_located_refusals() {
    for source in [
        "0#sum+{-2:a}0.",
        "2#sum+{-2:a;2:b}2.",
        "0#sum+{-2:a;2:b}0.",
        "b.2#sum+{-2:a;2:b}2.",
        "d(-1).0#sum+{W:a:d(W)}0.",
    ] {
        profile(source, ProfileFeature::HeadAggregateWeight);
    }
}

#[test]
fn false_outer_guards_cannot_hide_closed_weight_errors() {
    for source in ["0#sum{s:a}0:-#false.", "0#sum+{-1:a}0:-#false."] {
        profile(source, ProfileFeature::HeadAggregateWeight);
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(128))]
    #[test]
    fn weighted_bounds_match_finite_selection_semantics(
        left in -3_i32..=3, right in -3_i32..=3,
        lower in -3_i32..=3, upper in -3_i32..=3, positive in proptest::bool::ANY,
    ) {
        let function = if positive { "#sum+" } else { "#sum" };
        let left = if positive { left.abs() } else { left };
        let right = if positive { right.abs() } else { right };
        let source = format!("{lower}{function}{{{left},a:a;{right},b:b}}{upper}.");
        let admitted = input(&source);
        let names: Vec<_> = admitted.atoms().iter().map(atom_text).collect();
        proptest::prop_assert_eq!(names.len(), 2);
        let a = names.iter().position(|name| name == "a").unwrap();
        let b = names.iter().position(|name| name == "b").unwrap();
        for outer in 0_usize..4 {
            let total = i64::from(left) * i64::from(outer & (1 << a) != 0)
                + i64::from(right) * i64::from(outer & (1 << b) != 0);
            let allowed = total >= i64::from(lower) && total <= i64::from(upper);
            let original = values(admitted.theory(), outer, None);
            proptest::prop_assert_eq!(holds(admitted.theory(), &original), allowed, "{}: M={}", source, outer);
            for inner in 0_usize..4 {
                // Original choices require every atom selected by M in J.
                // The double-negated bound is a candidate test, not J's sum.
                let expected = allowed && outer & !inner == 0;
                proptest::prop_assert_eq!(holds(admitted.theory(), &values(admitted.theory(), inner, Some(&original))), expected, "{}: M={}, J={}", source, outer, inner);
            }
        }
    }
}

#[test]
fn variable_weights_are_checked_on_completed_rows() {
    for function in ["#sum", "#sum+"] {
        profile(
            &format!("d(s).0{function}{{W:a:d(W)}}0."),
            ProfileFeature::HeadAggregateWeight,
        );
    }
}

#[test]
fn negative_eligibility_cannot_hide_undefined_weights() {
    let source = "d(0).e.0#sum{W:a:d(X),W=1/X,not e}0.";
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
