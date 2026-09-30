//! Weighted head permissions remain independent of numeric contributions.

mod cases;
use crate::support::finite_bindings as reference;
mod alias_semantics;

use std::collections::BTreeSet;

use crate::support::objective_boundaries;
use cases::CASES;
use reference::{Models, exhaustive, external, holds, native, values};
use themelios_base::source::SourceId;
use zetesis_cpu::Cancellation;
use zetesis_reference_support::canonical;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, CountPlanLimits, CountPlanStatus, ExpansionFailure,
    ExpansionLimits, ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource,
    admit_formula, prepare_formula,
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
fn neutral_contributions_keep_head_permission() {
    let expected = Models::from([BTreeSet::new(), BTreeSet::from(["a".into()])]);
    for source in ["0#sum{0:a}0.", "0#sum+{0:a}0."] {
        assert_eq!(native(&input(source)), expected, "{source}");
    }
    assert_ne!(native(&input("")), expected);
}

#[test]
fn nonnumeric_weights_keep_head_permission() {
    for function in ["#sum", "#sum+"] {
        for weight in ["s", "#inf", "#sup", "f(1)", "(1,2)"] {
            for condition in ["d", "not d"] {
                assert_eq!(
                    native(&input(&format!(
                        "d.0{function}{{{weight}:a:{condition}}}0."
                    ))),
                    native(&input(&format!("d.{{a:{condition}}}."))),
                );
            }
        }
    }
}

#[test]
fn neutral_aliases_keep_every_head_permission() {
    let all = Models::from([
        BTreeSet::new(),
        BTreeSet::from(["a".into()]),
        BTreeSet::from(["b".into()]),
        BTreeSet::from(["a".into(), "b".into()]),
    ]);
    for source in ["0#sum{0:a;0:b}0.", "0#sum+{0:a;0:b}0."] {
        assert_eq!(native(&input(source)), all, "{source}");
    }
    assert_eq!(
        native(&input("0#sum+{0,k:a;0,l:a}0.")),
        Models::from([BTreeSet::new(), BTreeSet::from(["a".into()])])
    );
}

#[test]
fn tuple_identity_changes_the_selected_sum() {
    let shared = native(&input("1#sum{1:a;1:b}1."));
    assert_eq!(
        shared,
        Models::from([
            BTreeSet::from(["a".into()]),
            BTreeSet::from(["b".into()]),
            BTreeSet::from(["a".into(), "b".into()]),
        ])
    );
    // Counting each selected atom would incorrectly omit {a,b}.
    assert_ne!(shared, native(&input("1#sum{1,a:a;1,b:b}1.")));
    for source in ["3#sum{1:a;2:a}3.", "2#sum{1,k:a;1,l:a}2."] {
        assert_eq!(
            native(&input(source)),
            Models::from([BTreeSet::from(["a".into()])]),
            "{source}"
        );
    }
    // Coalescing by the atom or first tuple component would lose a contribution.
    assert!(native(&input("3#sum{1:a}3.")).is_empty());
    assert!(native(&input("2#sum{1,k:a}2.")).is_empty());
}

#[test]
fn recursive_aliases_require_actual_eligibility() {
    assert!(native(&input("1#sum{1:a:a;1:b:b}1.")).is_empty());
    assert_ne!(
        native(&input("1#sum{1:a:a;1:b:b}1.")),
        native(&input("1#sum{1:a;1:b}1."))
    );
}

#[test]
fn alias_order_preserves_full_models() {
    let elements = ["1:a", "1:b", "2:a"];
    for function in ["#sum", "#sum+"] {
        let expected = native(&input(&format!("3{function}{{1:a;1:b;2:a}}3.")));
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let tuples = order.map(|index| elements[index]).join(";");
            assert_eq!(
                native(&input(&format!("3{function}{{{tuples}}}3."))),
                expected
            );
        }
    }
}

#[test]
fn aliased_groups_select_each_resolved_weight() {
    for (source, explicit) in [
        ("0#sum{0:a;0:b;s:c}0.", "{a;b;c}."),
        ("d(0;s).0#sum{0:a;0:b;W:c:d(W)}0.", "d(0;s).{a;b;c}."),
        (
            "e.d(0;s).0#sum{0:a;0:b;W:c:d(W),not e}0.",
            "e.d(0;s).{a;b}.",
        ),
        ("0#sum+{0:a;0:b;-1:c}0.", "{a;b;c}."),
        ("d(0;-1).0#sum+{0:a;0:b;W:c:d(W)}0.", "d(0;-1).{a;b;c}."),
    ] {
        assert_eq!(native(&input(source)), native(&input(explicit)), "{source}");
    }
}

#[test]
fn weighted_head_producers_preserve_scored_answers() {
    for function in ["#sum", "#sum+"] {
        objective_boundaries::check(&format!("1{function}{{1:a}}1.#minimize{{1:a}}."));
    }
}

#[test]
fn signed_neutral_weights_supply_no_support() {
    for source in ["0#sum{0:not a}0.", "0#sum+{0:not not a}0."] {
        assert_eq!(native(&input(source)), Models::from([BTreeSet::new()]));
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
    for source in [
        "d(1..2).0#sum+{0,X:p(X):d(X)}0.",
        "1#sum{1:a;1:b}1.",
        "{b;c}.1#sum{1:a:b;2:a:c}1.",
        "0#sum+{0,k:a;0,l:a}0.",
    ] {
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
            println!("source={source} inclusive_{resource:?}={lower}");
        }
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
                .map(|(_, atom)| canonical(atom))
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
fn negative_sumplus_weights_keep_neutral_permission() {
    for (source, explicit) in [
        ("0#sum+{-2:a}0.", "{a}."),
        ("2#sum+{-2:a;2:b}2.", "{a;b}.:-not b."),
        ("0#sum+{-2:a;2:b}0.", "{a;b}.:-b."),
        ("b.2#sum+{-2:a;2:b}2.", "b.{a;b}.:-not b."),
        ("d(-1).0#sum+{W:a:d(W)}0.", "d(-1).{a}."),
    ] {
        assert_eq!(native(&input(source)), native(&input(explicit)), "{source}");
    }
}

#[test]
fn false_outer_guards_cannot_hide_undefined_weights() {
    for source in ["0#sum{1/0:a}0:-#false.", "0#sum+{1/0:a}0:-#false."] {
        let error = admit_formula(
            source.into(),
            options(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .expect_err("undefined source arithmetic is not a missing value");
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
        let names: Vec<_> = admitted.atoms().iter().map(canonical).collect();
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
fn variable_weights_select_completed_rows() {
    for function in ["#sum", "#sum+"] {
        assert_eq!(
            native(&input(&format!("d(s).0{function}{{W:a:d(W)}}0."))),
            native(&input("d(s).{a}.")),
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

#[test]
fn optional_planning_declines_weighted_aliases() {
    for source in [
        "1#sum{1:a;1:b}1.",
        "3#sum{1:a;2:a}3.",
        "0#sum+{0,k:a;0,l:a}0.",
    ] {
        let ordinary = input(source);
        let planned = prepare_formula(
            source.into(),
            options(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap()
        .ground_with_count_plan(CountPlanLimits::default(), &Cancellation::default(), None)
        .unwrap();
        assert!(
            matches!(planned.count_plan(), CountPlanStatus::NoPlan(_)),
            "{source}"
        );
        assert_eq!(ordinary.atoms(), planned.atoms());
        assert_eq!(ordinary.theory().nodes(), planned.theory().nodes());
        assert_eq!(ordinary.theory().roots(), planned.theory().roots());
        assert_eq!(ordinary.formula_origins(), planned.formula_origins());
        assert_eq!(native(&ordinary), native(&planned));
    }
}

#[test]
fn weighted_aliases_preserve_other_count_certificates() {
    let source = "2{a;b;c;d}2.{a;b}1.{c;d}1.1#sum{1:e;1:f}1.";
    let ordinary = input(source);
    let planned = prepare_formula(
        source.into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_with_count_plan(CountPlanLimits::default(), &Cancellation::default(), None)
    .unwrap();
    let CountPlanStatus::Ready(plan) = planned.count_plan() else {
        panic!("the disjoint count partition remains applicable");
    };
    assert!(planned.theory().same_instance(plan.original_theory()));
    assert_eq!(plan.consequence_count(), 2);
    assert_eq!(planned.theory().nodes(), ordinary.theory().nodes());
    assert_eq!(planned.theory().roots(), ordinary.theory().roots());
    assert_eq!(native(&planned), native(&ordinary));
    for mask in 0..1 << planned.atoms().len() {
        if holds(planned.theory(), &values(planned.theory(), mask, None)) {
            assert!(holds(
                plan.restriction(),
                &values(plan.restriction(), mask, None)
            ));
        }
    }
}

#[test]
fn tuple_limits_count_complete_weighted_keys() {
    let mut limits = FormulaLimits::default();
    limits.aggregate.max_elements = 1;
    assert!(
        admit_formula(
            "1#sum{1:a;1:b}1.".into(),
            options(),
            ExpansionLimits::default(),
            limits,
        )
        .is_ok()
    );
    let error = admit_formula(
        "0#sum+{0,k:a;0,l:a}0.".into(),
        options(),
        ExpansionLimits::default(),
        limits,
    )
    .unwrap_err();
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
fn alias_storage_limits_are_inclusive() {
    for source in ["1#sum{1:a;1:b}1.", "{b;c}.1#sum{1:a:b;2:a:c}1."] {
        let attempt = |maximum| {
            admit_formula(
                source.into(),
                options(),
                ExpansionLimits {
                    max_scalar_bytes: maximum,
                    ..Default::default()
                },
                FormulaLimits::default(),
            )
        };
        let mut lower = 0;
        let mut upper = 65_536;
        assert!(attempt(upper).is_ok());
        while lower + 1 < upper {
            let middle = lower + (upper - lower) / 2;
            if attempt(middle).is_ok() {
                upper = middle;
            } else {
                lower = middle;
            }
        }
        let error = attempt(upper - 1).unwrap_err();
        assert!(matches!(
            error,
            FormulaFailure::Expansion(ExpansionFailure::Limit {
                resource: ExpansionResource::ScalarBytes, limit, observed, ..
            }) if limit == (upper - 1) as u128 && observed == upper as u128
        ));
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
        assert_eq!(native(&attempt(upper).unwrap()), native(&input(source)));
        println!("source={source} inclusive_ScalarBytes={upper}");
    }
}
