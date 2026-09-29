//! Count-head tuple activity is independent of atom permission.

mod cases;
use crate::support::finite_bindings as reference;

use std::collections::BTreeSet;
use std::fmt::Write as _;

use cases::CASES;
use reference::{Models, exhaustive, external, holds, native, values};
use zetesis_cpu::Cancellation;
use zetesis_reference_support::{canonical, formula};
use zetesis_themelios::{
    AdmissionOptions, CountPlanLimits, CountPlanStatus, ExpansionFailure, ExpansionLimits,
    ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource, admit_formula,
    prepare_formula,
};

#[test]
fn models_match_separate_permissions() {
    for &(source, expanded) in CASES {
        assert_eq!(
            native(&formula(source)),
            native(&formula(expanded)),
            "{source}"
        );
    }
}

#[test]
fn stability_matches_subset_enumeration() {
    for &(source, _) in CASES {
        let admitted = formula(source);
        assert_eq!(native(&admitted), exhaustive(&admitted), "{source}");
    }
}

#[test]
fn frozen_truth_matches_separate_permissions() {
    let mut pairs = 0;
    for &(source, expanded) in CASES {
        let left = formula(source);
        let right = formula(expanded);
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
            let candidate = values(left.theory(), outer, None);
            let reference = values(right.theory(), remap(outer), None);
            assert_eq!(
                holds(left.theory(), &candidate),
                holds(right.theory(), &reference),
                "{source}: M={outer}"
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
        assert_eq!(native(&formula(source)), expected, "{source}");
        total += count;
    }
    println!("complete_sources={} full_models={total}", CASES.len());
}

#[test]
fn distinct_tuples_can_share_a_selected_atom() {
    assert_eq!(
        native(&formula("2#count{1:a;2:a}2.")),
        Models::from([BTreeSet::from(["a".into()])])
    );
    assert!(native(&formula("1#count{1:a;2:a}1.")).is_empty());
}

#[test]
fn distinct_atoms_can_activate_one_tuple() {
    assert_eq!(
        native(&formula("1#count{1:a;1:b}1.")),
        Models::from([
            BTreeSet::from(["a".into()]),
            BTreeSet::from(["b".into()]),
            BTreeSet::from(["a".into(), "b".into()]),
        ])
    );
}

#[test]
fn optional_planning_declines_nonbijective_groups() {
    for &(source, _) in CASES {
        let ordinary = formula(source);
        let planned = prepare_formula(
            source.into(),
            AdmissionOptions::default(),
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
        assert_eq!(ordinary.source().text(), planned.source().text());
        assert_eq!(native(&ordinary), native(&planned));
    }
}

#[test]
fn alias_groups_preserve_other_count_certificates() {
    let source = "2{a;b;c;d}2.{a;b}1.{c;d}1.1#count{1:e;1:f}1.";
    let ordinary = formula(source);
    let planned = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_with_count_plan(CountPlanLimits::default(), &Cancellation::default(), None)
    .unwrap();
    let CountPlanStatus::Ready(plan) = planned.count_plan() else {
        panic!("the original disjoint atom partition remains applicable");
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
fn tuple_limits_count_distinct_complete_keys() {
    let mut limits = FormulaLimits::default();
    limits.aggregate.max_elements = 1;
    assert!(
        admit_formula(
            "1#count{1:a;1:b}1.".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits,
        )
        .is_ok()
    );
    assert!(matches!(
        admit_formula(
            "1#count{1:a;2:a}1.".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits,
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::AggregateElements,
            limit: 1,
            observed: 2,
            ..
        })
    ));
}

#[test]
fn original_sources_remain_owned() {
    for &(source, _) in CASES {
        assert_eq!(formula(source).source().text(), source);
    }
}

fn threshold(mut succeeds: impl FnMut(u64) -> bool) -> u64 {
    let mut lower = 0;
    let mut upper = 65_536;
    assert!(succeeds(upper));
    while lower + 1 < upper {
        let middle = lower + (upper - lower) / 2;
        if succeeds(middle) {
            upper = middle;
        } else {
            lower = middle;
        }
    }
    upper
}

#[test]
fn alias_work_limits_are_inclusive() {
    for source in ["1#count{1:a;1:b}1.", "{b;c}.1#count{1:a:b;2:a:c}1."] {
        for resource in [FormulaResource::Work, FormulaResource::Substitutions] {
            let attempt = |maximum| {
                let mut limits = FormulaLimits::default();
                if resource == FormulaResource::Work {
                    limits.max_work = maximum;
                } else {
                    limits.max_substitutions = maximum;
                }
                admit_formula(
                    source.into(),
                    AdmissionOptions::default(),
                    ExpansionLimits::default(),
                    limits,
                )
            };
            let maximum = threshold(|limit| attempt(limit).is_ok());
            assert!(matches!(attempt(maximum - 1), Err(FormulaFailure::Limit {
                resource: actual, limit, observed, ..
            }) if actual == resource && limit == u128::from(maximum - 1)
                && observed == u128::from(maximum)));
            assert_eq!(native(&attempt(maximum).unwrap()), native(&formula(source)));
            println!("source={source} inclusive_{resource:?}={maximum}");
        }
    }
}

#[test]
fn alias_storage_limits_are_inclusive() {
    for source in ["1#count{1:a;1:b}1.", "{b;c}.1#count{1:a:b;2:a:c}1."] {
        let attempt = |maximum| {
            admit_formula(
                source.into(),
                AdmissionOptions::default(),
                ExpansionLimits {
                    max_scalar_bytes: usize::try_from(maximum).unwrap(),
                    ..Default::default()
                },
                FormulaLimits::default(),
            )
        };
        let maximum = threshold(|limit| attempt(limit).is_ok());
        assert!(
            matches!(attempt(maximum - 1), Err(FormulaFailure::Expansion(
            ExpansionFailure::Limit {
                resource: ExpansionResource::ScalarBytes, limit, observed, ..
            }
        )) if limit == u128::from(maximum - 1) && observed == u128::from(maximum))
        );
        assert_eq!(native(&attempt(maximum).unwrap()), native(&formula(source)));
        println!("source={source} inclusive_ScalarBytes={maximum}");
    }
}

fn eligibility(mode: u8, candidate: bool, tested: bool, frozen: bool) -> bool {
    match mode {
        0 => true,
        1 => tested && (!frozen || candidate),
        2 => !candidate,
        3 => candidate,
        _ => unreachable!("four generated eligibility forms"),
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 128,
        ..Default::default()
    })]

    #[test]
    fn count_activity_matches_independent_truth(
        rows in proptest::collection::vec((0_u8..3, proptest::bool::ANY, 0_u8..4), 1..7),
        lower in 0_usize..5,
        upper in 0_usize..5,
    ) {
        let mut source = format!("{{b}}.{lower}#count{{");
        for (index, &(key, head, mode)) in rows.iter().enumerate() {
            if index != 0 { source.push(';'); }
            let name = if head { "c" } else { "a" };
            let condition = ["", ":b", ":not b", ":not not b"][usize::from(mode)];
            write!(source, "{key}:{name}{condition}").unwrap();
        }
        write!(source, "}}{upper}.").unwrap();
        let admitted = formula(&source);
        let names: Vec<_> = admitted.atoms().iter().map(canonical).collect();
        let member = |mask: usize, name: &str| {
            names.iter().position(|atom| atom == name)
                .is_some_and(|index| mask & (1 << index) != 0)
        };
        for outer in 0..1 << names.len() {
            let b = member(outer, "b");
            let selected: BTreeSet<_> = rows.iter().filter_map(|&(key, head, mode)| {
                let name = if head { "c" } else { "a" };
                (member(outer, name) && eligibility(mode, b, b, false)).then_some(key)
            }).collect();
            // The compiler also adds candidate-only necessary-support guards.
            // They constrain M, not the availability of support in J's reduct.
            let supported = [false, true].into_iter().all(|head| {
                let name = if head { "c" } else { "a" };
                !member(outer, name) || rows.iter().any(|&(_, other, mode)| {
                    other == head && eligibility(mode, b, b, false)
                })
            });
            let original = supported && lower <= selected.len() && selected.len() <= upper;
            let candidate = values(admitted.theory(), outer, None);
            proptest::prop_assert_eq!(holds(admitted.theory(), &candidate), original,
                "{}: M={}", source, outer);
            for inner in 0..1 << names.len() {
                let b_inner = member(inner, "b");
                let permissions = rows.iter().all(|&(_, head, mode)| {
                    let name = if head { "c" } else { "a" };
                    !member(outer, name)
                        || !eligibility(mode, b, b_inner, true)
                        || member(inner, name)
                });
                let frozen = original && (!b || b_inner) && permissions;
                proptest::prop_assert_eq!(
                    holds(admitted.theory(), &values(admitted.theory(), inner, Some(&candidate))),
                    frozen, "{}: M={} J={}", source, outer, inner
                );
            }
        }
    }
}
