//! Extrema heads retain permissions independently of selected logical values.

#[path = "support/extrema_heads.rs"]
mod cases;
#[path = "support/finite_bindings.rs"]
mod reference;
#[path = "support/logical_extrema_semantics.rs"]
mod logical;

use std::collections::BTreeSet;

use cases::sources;
use reference::{Models, atom_text, exhaustive, external, holds, native, values};
use themelios_base::source::SourceId;
use themelios_program::term::EvalError;
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits,
    FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature, admit_formula, prepare_formula,
};

const SOURCE: SourceId = SourceId::new(179);

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
    for (source, expanded) in sources() {
        assert_eq!(
            native(&input(&source)),
            native(&input(&expanded)),
            "{source}"
        );
    }
}

#[test]
fn stability_matches_subset_enumeration() {
    for (source, _) in sources() {
        let admitted = input(&source);
        assert_eq!(native(&admitted), exhaustive(&admitted), "{source}");
    }
}

#[test]
fn frozen_truth_matches_finite_substitutions() {
    let mut pairs = 0;
    for (source, expanded) in sources() {
        let left = input(&source);
        let right = input(&expanded);
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
    println!("source_pairs={} frozen_pairs={pairs}", sources().len());
}

#[test]
#[ignore = "requires an independently installed clingo"]
fn original_sources_match_clingo_full_models() {
    let mut total = 0;
    for (source, _) in sources() {
        let result = external(&source, true);
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
        assert_eq!(native(&input(&source)), expected, "{source}");
        total += count;
    }
    println!("complete_sources={} full_models={total}", sources().len());
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
        matches!(error,
        FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {feature, ..}))
        if feature == expected),
        "{source}: {error}"
    );
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
}

#[test]
fn bounds_do_not_create_recursive_head_support() {
    for function in ["#min", "#max"] {
        assert!(native(&input(&format!("0{function}{{0:a:a}}0."))).is_empty());
    }
}

#[test]
fn coalesced_eligibility_remains_a_frozen_formula() {
    let source = "0<=#min{0:a:a;0:a:not a}.";
    assert_eq!(native(&input(source)), Models::from([BTreeSet::new()]));
    assert_ne!(native(&input(source)), native(&input("0<=#min{0:a}.")));
}

#[test]
fn numeric_values_keep_head_permission() {
    let expected = Models::from([
        BTreeSet::new(),
        BTreeSet::from(["a".into()]),
        BTreeSet::from(["b".into()]),
        BTreeSet::from(["a".into(), "b".into()]),
    ]);
    for source in ["-2<=#min{-2:a;0:b}.", "#max{-2:a;0:b}<=0."] {
        assert_eq!(native(&input(source)), expected);
    }
}

#[test]
fn equal_values_do_not_merge_distinct_tuple_permissions() {
    let expected = Models::from([
        BTreeSet::from(["a".into()]),
        BTreeSet::from(["b".into()]),
        BTreeSet::from(["a".into(), "b".into()]),
    ]);
    for function in ["#min", "#max"] {
        assert_eq!(
            native(&input(&format!("1{function}{{1,k:a;1,l:b}}1."))),
            expected
        );
    }
}

#[test]
fn element_order_preserves_full_models() {
    let elements = ["-1:a", "0:b", "2:c"];
    for function in ["#min", "#max"] {
        let expected = native(&input(&format!("{function}{{-1:a;0:b;2:c}}>=0.")));
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
                native(&input(&format!("{function}{{{tuples}}}>=0."))),
                expected
            );
        }
    }
}

#[test]
fn aliased_tuples_retain_each_atom_permission() {
    for function in ["#min", "#max"] {
        assert_eq!(
            native(&input(&format!("1{function}{{1:a;1:b}}1."))),
            Models::from([
                BTreeSet::from(["a".into()]),
                BTreeSet::from(["b".into()]),
                BTreeSet::from(["a".into(), "b".into()]),
            ])
        );
    }
}

#[test]
fn aliased_atoms_select_every_eligible_tuple() {
    for (function, bound) in [("#min", 1), ("#max", 2)] {
        assert_eq!(
            native(&input(&format!("{bound}{function}{{1:a;2:a}}{bound}."))),
            Models::from([BTreeSet::from(["a".into()])])
        );
    }
}

#[test]
fn logical_values_remain_admitted_under_false_activation() {
    for function in ["#min", "#max"] {
        for value in ["word", "#inf", "#sup", "f(1)", "(1,2)"] {
            assert_eq!(
                native(&input(&format!("0<={function}{{{value}:a}}:-#false."))),
                Models::from([BTreeSet::new()]),
            );
        }
    }
}

#[test]
fn completed_logical_values_keep_element_permission() {
    let expected = Models::from([BTreeSet::from(["v(word)".into(), "a".into()])]);
    for function in ["#min", "#max"] {
        assert_eq!(
            native(&input(&format!("v(word).{function}{{X:a:v(X)}}=word."))),
            expected,
        );
    }
}

#[test]
fn real_empty_sentinels_do_not_erase_head_permission() {
    for source in ["#min{#sup:a}=#sup.", "#max{#inf:a}=#inf."] {
        assert_eq!(
            native(&input(source)),
            Models::from([BTreeSet::new(), BTreeSet::from(["a".into()])]),
        );
    }
}

#[test]
fn logical_term_order_is_distinct_from_storage_order() {
    assert_eq!(
        native(&input("#min{word:a;\"word\":b}=word.")),
        Models::from([
            BTreeSet::from(["a".into()]),
            BTreeSet::from(["a".into(), "b".into()]),
        ]),
    );
}

#[test]
fn logical_aliases_retain_each_positive_permission() {
    for function in ["#min", "#max"] {
        assert_eq!(
            native(&input(&format!("{function}{{f(1),k:a;f(1),k:b}}=f(1)."))),
            Models::from([
                BTreeSet::from(["a".into()]),
                BTreeSet::from(["b".into()]),
                BTreeSet::from(["a".into(), "b".into()]),
            ]),
        );
    }
}

#[test]
fn logical_boolean_values_create_no_atoms() {
    for function in ["#min", "#max"] {
        for head in ["#true", "not #false", "not not #true"] {
            let admitted = input(&format!("{function}{{f(1):{head}}}=f(1)."));
            assert!(admitted.atoms().is_empty());
            assert_eq!(native(&admitted), Models::from([BTreeSet::new()]));
        }
    }
}

#[test]
fn logical_keys_retain_the_complete_tuple_budget() {
    for function in ["#min", "#max"] {
        let limits = FormulaLimits {
            aggregate: zetesis_ferraris::AggregateLimits {
                max_elements: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            admit_formula(
                format!("{function}{{f(1),k:a;f(1),k:b}}=f(1)."),
                options(),
                ExpansionLimits::default(),
                limits,
            )
            .is_ok()
        );
        let error = admit_formula(
            format!("{function}{{f(1),k:a;f(1),l:a}}=f(1)."),
            options(),
            ExpansionLimits::default(),
            limits,
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Limit {
                    resource: FormulaResource::AggregateElements,
                    limit: 1,
                    observed: 2,
                    ..
                }
            ),
            "{error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    }
}

#[test]
fn logical_values_retain_scalar_storage_limits() {
    let source = "#min{f(\"retained payload\"),k:a}=f(\"retained payload\").";
    let error = admit_formula(
        source.into(),
        options(),
        ExpansionLimits {
            max_scalar_bytes: 0,
            ..Default::default()
        },
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            FormulaFailure::Expansion(ExpansionFailure::Limit {
                resource: zetesis_themelios::ExpansionResource::ScalarBytes,
                ..
            })
        ),
        "{error}"
    );
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
}

#[test]
fn logical_extrema_do_not_certify_atom_counts() {
    for source in [
        "#min{f(1):a;f(1):b}=f(1).{a;c}1.{b;d}1.",
        "#max{word:#true;word:a;word:b}=word.{a;c}1.{b;d}1.",
    ] {
        let ordinary = input(source);
        let planned = prepare_formula(
            source.into(),
            options(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap()
        .ground_with_count_plan(
            zetesis_themelios::CountPlanLimits::default(),
            &zetesis_cpu::Control::default(),
            None,
        )
        .unwrap();
        assert!(matches!(
            planned.count_plan(),
            zetesis_themelios::CountPlanStatus::NoPlan(_)
        ));
        assert_eq!(planned.atoms(), ordinary.atoms());
        assert_eq!(planned.theory().nodes(), ordinary.theory().nodes());
        assert_eq!(planned.theory().roots(), ordinary.theory().roots());
    }
}

#[test]
fn bounded_empty_head_tuples_have_named_refusals() {
    for function in ["#min", "#max"] {
        profile(
            &format!("{function}{{:a}}<=0."),
            ProfileFeature::HeadAggregateMissingValue,
        );
    }
}

#[test]
fn predecessor_rows_keep_separate_activated_groups() {
    let source = "d(0..1).I#min{I:p(I)}I:-d(I).";
    assert_eq!(
        native(&input(source)),
        Models::from([BTreeSet::from([
            "d(0)".into(),
            "d(1)".into(),
            "p(0)".into(),
            "p(1)".into(),
        ])])
    );
    assert_ne!(
        native(&input(source)),
        native(&input("d(0..1).0#min{0:p(0);1:p(1)}1."))
    );
}

#[test]
fn numeric_endpoints_retain_the_zetesis_boundary() {
    for function in ["#min", "#max"] {
        for endpoint in ["(-2147483647-1)", "2147483647"] {
            for source in [
                format!("0<={function}{{{endpoint}:a}}."),
                format!("{function}{{0:a}}<={endpoint}."),
            ] {
                profile(&source, ProfileFeature::Aggregate);
            }
        }
    }
}

#[test]
fn numeric_neighbors_remain_distinct_from_empty_values() {
    for source in [
        "#min{2147483646:a}=2147483646.",
        "#max{-2147483647:a}=-2147483647.",
    ] {
        assert_eq!(
            native(&input(source)),
            Models::from([BTreeSet::from(["a".into()])])
        );
    }
}

#[test]
fn extrema_producers_preserve_scored_answers() {
    for function in ["#min", "#max"] {
        for value in ["1", "word", "f(1)", "#inf", "#sup"] {
            objective_boundaries::check(&format!(
                "{function}{{{value}:a}}={value}.#minimize{{1:a}}."
            ));
        }
    }
}
#[path = "support/objective_boundaries.rs"]
mod objective_boundaries;
#[path = "support/source_records.rs"]
mod source_records;

#[test]
fn signed_extrema_do_not_supply_atom_support() {
    for function in ["#min", "#max"] {
        for sign in ["not", "not not"] {
            let result = native(&input(&format!("0{function}{{0:{sign} a}}0.")));
            let expected = if sign == "not" {
                Models::from([BTreeSet::new()])
            } else {
                Models::new()
            };
            assert_eq!(result, expected, "{function} {sign}");
        }
    }
}

#[test]
fn local_values_require_independent_bindings() {
    for function in ["#min", "#max"] {
        let source = format!("0<={function}{{X:a:not p(X)}}.");
        let error = prepare_formula(
            source,
            options(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    }
}

#[test]
fn undefined_generated_values_refuse_the_whole_source() {
    let error = admit_formula(
        "0<=#min{Y:a:d(X),Y=1/X}.d(0).".into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                error: EvalError::Undefined,
                ..
            })
        ),
        "{error}"
    );
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
}

#[test]
fn original_sources_remain_owned() {
    for (source, _) in sources() {
        assert_eq!(input(&source).source().text(), source);
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(128))]
    #[test]
    fn logical_extrema_match_independent_frozen_worlds(
        rows in proptest::collection::vec((0_u8..9, 0_u8..3, 0_u8..12, 0_u8..10), 1..7),
        bound in 0_u8..9, relation in 0_u8..6, body in 0_u8..4,
        maximum in proptest::bool::ANY,
    ) {
        logical::Selection { rows, bound, relation, body, maximum }.check_frozen();
    }

    #[test]
    fn duplicate_logical_rows_preserve_full_models(
        rows in proptest::collection::vec((0_u8..9, 0_u8..3, 0_u8..12, 0_u8..10), 1..6),
        bound in 0_u8..9, relation in 0_u8..6, body in 0_u8..4,
        maximum in proptest::bool::ANY,
    ) {
        let mut selection = logical::Selection { rows, bound, relation, body, maximum };
        let expected = native(&input(&selection.source()));
        selection.rows.extend(selection.rows.clone());
        selection.rows.reverse();
        proptest::prop_assert_eq!(native(&input(&selection.source())), expected);
    }

    #[test]
    fn extrema_bounds_match_direct_selection_truth(
        first in -3_i32..4, second in -3_i32..4, bound in -3_i32..4,
        minimum in proptest::bool::ANY,
        relation in proptest::sample::select(vec!["=", "!=", "<", "<=", ">", ">="]),
    ) {
        let function = if minimum { "#min" } else { "#max" };
        let source = format!("{function}{{{first},k:a;{second},l:b}}{relation}{bound}.");
        let admitted = input(&source);
        let names: Vec<_> = admitted.atoms().iter().map(atom_text).collect();
        proptest::prop_assert_eq!(names.iter().map(String::as_str).collect::<BTreeSet<_>>(), BTreeSet::from(["a", "b"]));
        for outer in 0..1 << names.len() {
            let selected: Vec<_> = names.iter().enumerate()
                .filter(|(index, _)| outer & (1 << index) != 0)
                .map(|(_, name)| if name == "a" { first } else { second })
                .collect();
            let chosen = if minimum { selected.iter().min() } else { selected.iter().max() };
            // No integer sentinel: the empty minimum is above every number;
            // the empty maximum is below every number.
            let order = chosen.map_or_else(
                || if minimum { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less },
                |value| value.cmp(&bound),
            );
            let permitted = match relation {
                "=" => order.is_eq(), "!=" => !order.is_eq(),
                "<" => order.is_lt(), "<=" => !order.is_gt(),
                ">" => order.is_gt(), ">=" => !order.is_lt(),
                _ => unreachable!(),
            };
            let original = values(admitted.theory(), outer, None);
            proptest::prop_assert_eq!(holds(admitted.theory(), &original), permitted);
            for inner in 0..1 << names.len() {
                // Unconditional choice permissions freeze each selected atom.
                // The activated bound is a candidate-only constraint.
                proptest::prop_assert_eq!(
                    holds(admitted.theory(), &values(admitted.theory(), inner, Some(&original))),
                    permitted && outer & inner == outer,
                );
            }
        }
    }
}

#[test]
fn grounding_limits_are_inclusive() {
    let source = "d(-1..1).0#max{X:p(X):d(X)}1.";
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
