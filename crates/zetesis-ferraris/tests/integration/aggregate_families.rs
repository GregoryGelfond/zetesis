//! Shared families are compared with independent full failing-subset formulas.

use std::time::Instant;

use crate::support::aggregate_theories::{COMPARISONS, copy, prefix, push, raw, snapshot};
use crate::support::worlds::{eval, interpretation};
use proptest::prelude::*;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison as Comparison, AggregateElement as Element,
    AggregateErrorKind as Error, AggregateFamilyLimits as FamilyLimits, AggregateGuard as Guard,
    AggregateLimits, AggregateProfile, Interpretation, Limits, Narrower, Narrowing,
    NarrowingScratch, Node, NodeView, OriginalSubject, Region, RegionLimits, Theory,
    append_aggregate, append_aggregate_family, models, models_reduct,
};

// Recursive evaluation is only the small independent test interpreter. It
// materializes neither the production topological mask nor its witness formula.

fn boundary_consequence(weights: [i32; 3], bounds: &[i64], premise: bool) -> Region {
    let cancellation = Cancellation::default();
    let mut nodes = raw((0..4).map(Node::atom).collect());
    let elements = [0, 1, 2].map(|condition| Element {
        weight: weights[condition],
        condition,
    });
    let guards: Vec<_> = bounds
        .iter()
        .map(|&bound| Guard {
            comparison: Comparison::Ge,
            bound,
        })
        .collect();
    let family = append_aggregate_family(
        &mut nodes,
        &elements,
        &guards,
        FamilyLimits::default(),
        &cancellation,
    )
    .unwrap();
    let given = family.roots()[0];
    let consequence = family.roots()[1];
    // The observer makes an inferred guard truth visible as an atom decision.
    // Neither premise fixes an individual contribution without that inference.
    let roots = if premise {
        vec![given, push(&mut nodes, NodeView::Implies(consequence, 3))]
    } else {
        let falsum = push(&mut nodes, NodeView::False);
        vec![
            push(&mut nodes, NodeView::Implies(given, falsum)),
            push(&mut nodes, NodeView::Implies(3, consequence)),
        ]
    };
    let theory = Theory::new(4, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap();
    let narrower = Narrower::new(&theory);
    let mut region = Region::all_open(4);
    let (outcome, _) = narrower
        .narrow_known(
            OriginalSubject::new(&theory, None),
            &mut region,
            &mut narrower.knowledge(),
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &cancellation,
        )
        .unwrap();
    assert_ne!(outcome, Narrowing::Refuted);
    region
}

#[test]
fn false_interior_guard_refutes_a_stronger_boundary_guard() {
    let region = boundary_consequence([1; 3], &[2, 3], false);
    assert_eq!(region.decision(3), Some(false));
    assert!((0..3).all(|atom| region.is_open(atom)));
}

#[test]
fn true_interior_guard_implies_a_weaker_boundary_guard() {
    let region = boundary_consequence([1; 3], &[2, 1], true);
    assert_eq!(region.decision(3), Some(true));
    assert!((0..3).all(|atom| region.is_open(atom)));
}

#[test]
fn equivalent_all_guards_share_false_consequences() {
    let region = boundary_consequence([2; 3], &[6, 5, 3], false);
    assert_eq!(region.decision(3), Some(false));
    assert!((0..3).all(|atom| region.is_open(atom)));
}

#[test]
fn equivalent_any_guards_share_true_consequences() {
    let region = boundary_consequence([2; 3], &[1, 2, 3], true);
    assert_eq!(region.decision(3), Some(true));
    assert!((0..3).all(|atom| region.is_open(atom)));
}

#[test]
fn an_interval_between_attainable_sums_is_refuted_without_decisions() {
    let cancellation = Cancellation::default();
    let mut nodes = raw((0..3).map(Node::atom).collect());
    let elements = [0, 1, 2].map(|condition| Element {
        weight: 2,
        condition,
    });
    let family = append_aggregate_family(
        &mut nodes,
        &elements,
        &[
            Guard {
                comparison: Comparison::Ge,
                bound: 3,
            },
            Guard {
                comparison: Comparison::Lt,
                bound: 4,
            },
        ],
        FamilyLimits::default(),
        &cancellation,
    )
    .unwrap();
    let theory = Theory::new(
        3,
        nodes.into_parts(),
        family.roots().to_vec(),
        AdmissionLimits::default(),
    )
    .unwrap();
    let narrower = Narrower::new(&theory);
    let mut region = Region::all_open(3);
    let (outcome, _) = narrower
        .narrow_known(
            OriginalSubject::new(&theory, None),
            &mut region,
            &mut narrower.knowledge(),
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &cancellation,
        )
        .unwrap();
    assert_eq!(outcome, Narrowing::Refuted);
    assert!((0..3).all(|atom| region.is_open(atom)));
}

fn holds(comparison: Comparison, value: i128, bound: i64) -> bool {
    let bound = i128::from(bound);
    match comparison {
        Comparison::Eq => value == bound,
        Comparison::Ne => value != bound,
        Comparison::Lt => value < bound,
        Comparison::Le => value <= bound,
        Comparison::Gt => value > bound,
        Comparison::Ge => value >= bound,
    }
}

fn reference(
    elements: &[Element],
    comparison: Comparison,
    bound: i64,
) -> (zetesis_ferraris::FormulaNodes, usize) {
    let mut nodes = prefix();
    let mut root = 1;
    for subset in 0usize..(1 << elements.len()) {
        let selected = elements
            .iter()
            .enumerate()
            .filter(|(index, _)| subset & (1 << index) != 0)
            .map(|(_, element)| element.weight);
        if holds(comparison, selected.map(i128::from).sum(), bound) {
            continue;
        }
        let mut antecedent = 1;
        let mut consequent = 0;
        for (index, element) in elements.iter().enumerate() {
            if subset & (1 << index) == 0 {
                consequent = push(
                    &mut nodes,
                    zetesis_ferraris::NodeView::Or(&[consequent, element.condition]),
                );
            } else {
                antecedent = push(
                    &mut nodes,
                    zetesis_ferraris::NodeView::And(&[antecedent, element.condition]),
                );
            }
        }
        let implication = push(
            &mut nodes,
            zetesis_ferraris::NodeView::Implies(antecedent, consequent),
        );
        root = push(
            &mut nodes,
            zetesis_ferraris::NodeView::And(&[root, implication]),
        );
    }
    (nodes, root)
}

fn verify(elements: &[Element], guards: &[Guard]) {
    let input = prefix();
    let mut nodes = copy(&input);
    let family = append_aggregate_family(
        &mut nodes,
        elements,
        guards,
        FamilyLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(family.roots().len(), guards.len());
    for (&root, guard) in family.roots().iter().zip(guards) {
        let theory = Theory::new(
            2,
            copy(&nodes).into_parts(),
            vec![root],
            AdmissionLimits::default(),
        )
        .unwrap();
        let (reference, reference_root) = reference(elements, guard.comparison, guard.bound);
        for outer in 0..4 {
            let sum = elements
                .iter()
                .filter(|element| eval(input.view(), element.condition, outer, None))
                .map(|element| i128::from(element.weight))
                .sum();
            let classical = holds(guard.comparison, sum, guard.bound);
            let candidate = interpretation(&theory, outer);
            assert_eq!(
                eval(reference.view(), reference_root, outer, None),
                classical
            );
            assert_eq!(
                models(
                    &theory,
                    &candidate,
                    Limits::default(),
                    &Cancellation::default()
                )
                .unwrap(),
                classical
            );
            for inner in 0..4 {
                let sum = elements
                    .iter()
                    .filter(|element| eval(input.view(), element.condition, inner, Some(outer)))
                    .map(|element| i128::from(element.weight))
                    .sum();
                let expected = classical && holds(guard.comparison, sum, guard.bound);
                assert_eq!(
                    eval(reference.view(), reference_root, inner, Some(outer)),
                    expected
                );
                assert_eq!(
                    models_reduct(
                        &theory,
                        &candidate,
                        &interpretation(&theory, inner),
                        Limits::default(),
                        &Cancellation::default()
                    )
                    .unwrap(),
                    expected,
                    "family={guards:?}, elements={elements:?}, guard={guard:?}, M={outer}, J={inner}"
                );
            }
        }
    }
}

#[test]
fn exhaustive_families_preserve_each_guards_complete_frozen_reduct() {
    let guards: Vec<_> = COMPARISONS
        .into_iter()
        .flat_map(|comparison| (-3..=3).map(move |bound| Guard { comparison, bound }))
        .collect();
    for left in -2..=2 {
        for right in -2..=2 {
            for second_condition in [2, 3, 4, 5, 9, 10] {
                verify(
                    &[
                        Element {
                            weight: left,
                            condition: 2,
                        },
                        Element {
                            weight: right,
                            condition: second_condition,
                        },
                    ],
                    &guards,
                );
            }
        }
    }
}

#[test]
fn scaled_guards_preserve_unattainable_bounds_in_every_frozen_reduct() {
    let guards: Vec<_> = COMPARISONS
        .into_iter()
        .flat_map(|comparison| (-1..=13).map(move |bound| Guard { comparison, bound }))
        .collect();
    // Repeated and composite conditions do not change the divisibility of any
    // sum. In particular, equality at an odd bound is false in every reduct,
    // while inequality retains its implication reading.
    verify(
        &[
            Element {
                weight: 2,
                condition: 2,
            },
            Element {
                weight: 4,
                condition: 9,
            },
            Element {
                weight: 6,
                condition: 2,
            },
            Element {
                weight: 0,
                condition: 10,
            },
        ],
        &guards,
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(192))]
    #[test]
    fn generated_family_order_duplicates_and_arbitrary_conditions_preserve_reducts(
        entries in prop::collection::vec((-3i32..=3, 0usize..11), 0..=5),
        guards in prop::collection::vec((0usize..6, -9i64..=9), 0..=8),
    ) {
        let elements: Vec<_> = entries.into_iter().map(|(weight, condition)| Element { weight, condition }).collect();
        let guards: Vec<_> = guards.into_iter().map(|(comparison,bound)| Guard { comparison: COMPARISONS[comparison], bound }).collect();
        verify(&elements, &guards);
    }
}

#[test]
fn empty_zero_signed_and_extreme_guards_have_no_numeric_wraparound() {
    let guards: Vec<_> = COMPARISONS
        .into_iter()
        .flat_map(|comparison| {
            [i64::MIN, -1, 0, 1, i64::MAX].map(move |bound| Guard { comparison, bound })
        })
        .collect();
    for elements in [
        vec![],
        vec![Element {
            weight: 0,
            condition: 10,
        }],
        vec![
            Element {
                weight: i32::MIN,
                condition: 4,
            },
            Element {
                weight: i32::MAX,
                condition: 3,
            },
        ],
    ] {
        verify(&elements, &guards);
    }
    // Very large constant-false thresholds must not size a dynamic-program row.
    let mut nodes = prefix();
    let family = append_aggregate_family(
        &mut nodes,
        &[Element {
            weight: i32::MAX,
            condition: 2,
        }],
        &[
            Guard {
                comparison: Comparison::Ge,
                bound: 1,
            },
            Guard {
                comparison: Comparison::Eq,
                bound: i64::MAX,
            },
        ],
        FamilyLimits {
            aggregate: AggregateLimits {
                max_states: 4,
                max_subsets: 0,
                ..AggregateLimits::default()
            },
            max_guards: 2,
        },
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(family.statistics().states, 0);
    assert_eq!(family.statistics().subsets, 0);
}

#[test]
fn ordered_guards_share_one_contribution_row() {
    let input = raw((0..16).map(Node::atom).collect());
    let elements: Vec<_> = (0..16)
        .map(|condition| Element {
            weight: 1,
            condition,
        })
        .collect();
    let mut guards: Vec<_> = (0..=16)
        .map(|bound| Guard {
            comparison: Comparison::Eq,
            bound,
        })
        .collect();
    guards.push(guards[8]);
    let mut nodes = copy(&input);
    let family = append_aggregate_family(
        &mut nodes,
        &elements,
        &guards,
        FamilyLimits {
            aggregate: AggregateLimits {
                max_subsets: 0,
                ..AggregateLimits::default()
            },
            max_guards: guards.len(),
        },
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(family.profile(), AggregateProfile::Threshold);
    assert_eq!(family.statistics().states, 66);
    assert_eq!(nodes.parts().rows().len(), 1);
    assert_eq!(nodes.parts().conditions().len(), 16);
    assert_eq!(family.statistics().subsets, 0);
    let isolated_work: u64 = guards
        .iter()
        .map(|guard| {
            let mut isolated = copy(&input);
            append_aggregate(
                &mut isolated,
                &elements,
                guard.comparison,
                guard.bound,
                AggregateLimits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .statistics()
            .work
        })
        .sum();
    assert!(
        family.statistics().work * 5 < isolated_work,
        "shared work={} isolated={isolated_work}",
        family.statistics().work
    );
    for (&root, guard) in family.roots().iter().zip(&guards) {
        let theory = Theory::new(
            16,
            copy(&nodes).into_parts(),
            vec![root],
            AdmissionLimits::default(),
        )
        .unwrap();
        for count in [0, 7, 8, 9, 16] {
            let candidate = Interpretation::new(&theory, 0..count).unwrap();
            assert_eq!(
                models(
                    &theory,
                    &candidate,
                    Limits::default(),
                    &Cancellation::default()
                )
                .unwrap(),
                i64::try_from(count).unwrap() == guard.bound
            );
        }
    }
}

#[test]
fn common_divisors_scale_one_shared_row_and_round_each_bound_up() {
    let mut nodes = raw((0..3).map(Node::atom).collect());
    let elements = [6, 10, 14].map(|weight| Element {
        weight,
        condition: 0,
    });
    let family = append_aggregate_family(
        &mut nodes,
        &elements,
        &[
            Guard {
                comparison: Comparison::Ge,
                bound: 7,
            },
            Guard {
                comparison: Comparison::Ge,
                bound: 8,
            },
        ],
        FamilyLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(nodes.parts().rows().len(), 1);
    assert_eq!(nodes.parts().conditions(), &[0, 0, 0]);
    assert_eq!(nodes.parts().numeric_values(), &[3, 5, 7]);
    assert_eq!(
        nodes
            .parts()
            .thresholds()
            .iter()
            .map(|threshold| threshold.bound)
            .collect::<Vec<_>>(),
        vec![4, 4]
    );
    assert_eq!(family.statistics().states, 8);
}

fn limited(elements: &[Element], guards: &[Guard], limits: FamilyLimits) -> Error {
    let original = prefix();
    let mut nodes = copy(&original);
    let error = append_aggregate_family(
        &mut nodes,
        elements,
        guards,
        limits,
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(
        snapshot(&nodes),
        snapshot(&original),
        "an incomplete family must roll back every appended node"
    );
    error.kind()
}

#[test]
fn family_guard_node_work_state_and_element_ceilings_are_inclusive() {
    let elements =
        [(6, 2), (10, 3), (14, 4)].map(|(weight, condition)| Element { weight, condition });
    let guards = [
        Guard {
            comparison: Comparison::Eq,
            bound: 5,
        },
        Guard {
            comparison: Comparison::Ne,
            bound: 7,
        },
    ];
    let mut nodes = prefix();
    let family = append_aggregate_family(
        &mut nodes,
        &elements,
        &guards,
        FamilyLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let exact = FamilyLimits {
        aggregate: AggregateLimits {
            max_elements: elements.len(),
            max_nodes: nodes.view().len(),
            max_operands: nodes.parts().occurrences(),
            max_work: family.statistics().work,
            max_states: family.statistics().states,
            max_subsets: 0,
        },
        max_guards: guards.len(),
    };
    let mut repeated = prefix();
    assert_eq!(
        append_aggregate_family(
            &mut repeated,
            &elements,
            &guards,
            exact,
            &Cancellation::default()
        )
        .unwrap(),
        family
    );
    assert_eq!(snapshot(&repeated), snapshot(&nodes));
    assert_eq!(
        limited(
            &elements,
            &guards,
            FamilyLimits {
                max_guards: 1,
                ..exact
            }
        ),
        Error::GuardLimit
    );
    for (aggregate, expected) in [
        (
            AggregateLimits {
                max_elements: elements.len() - 1,
                ..exact.aggregate
            },
            Error::ElementLimit,
        ),
        (
            AggregateLimits {
                max_nodes: exact.aggregate.max_nodes - 1,
                ..exact.aggregate
            },
            Error::NodeLimit,
        ),
        (
            AggregateLimits {
                max_work: exact.aggregate.max_work - 1,
                ..exact.aggregate
            },
            Error::WorkLimit,
        ),
        (
            AggregateLimits {
                max_states: exact.aggregate.max_states - 1,
                ..exact.aggregate
            },
            Error::StateLimit,
        ),
    ] {
        assert_eq!(
            limited(&elements, &guards, FamilyLimits { aggregate, ..exact }),
            expected
        );
    }
}

#[test]
fn signed_subset_budget_is_cumulative_and_late_refusal_is_atomic() {
    let elements = [
        Element {
            weight: -1,
            condition: 2,
        },
        Element {
            weight: 2,
            condition: 3,
        },
    ];
    let guards = [
        Guard {
            comparison: Comparison::Eq,
            bound: 0,
        },
        Guard {
            comparison: Comparison::Ne,
            bound: 1,
        },
        Guard {
            comparison: Comparison::Ge,
            bound: -1,
        },
    ];
    let exact = FamilyLimits {
        aggregate: AggregateLimits {
            max_subsets: 12,
            max_states: 2,
            ..AggregateLimits::default()
        },
        max_guards: 3,
    };
    let mut nodes = prefix();
    let family = append_aggregate_family(
        &mut nodes,
        &elements,
        &guards,
        exact,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(family.profile(), AggregateProfile::SubsetImplications);
    assert_eq!(family.statistics().subsets, 12);
    assert_eq!(family.statistics().states, 2);
    assert_eq!(
        limited(
            &elements,
            &guards,
            FamilyLimits {
                aggregate: AggregateLimits {
                    max_subsets: 11,
                    ..exact.aggregate
                },
                ..exact
            }
        ),
        Error::SubsetLimit
    );
    let mut partial = prefix();
    let error = append_aggregate_family(
        &mut partial,
        &elements,
        &guards,
        FamilyLimits {
            aggregate: AggregateLimits {
                max_subsets: 11,
                ..exact.aggregate
            },
            ..exact
        },
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(error.statistics().subsets, 8);
    assert!(error.statistics().nodes > 2);
    assert_eq!(snapshot(&partial), snapshot(&prefix()));
}

#[test]
fn empty_family_still_validates_and_appends_nothing() {
    let original = prefix();
    let mut nodes = copy(&original);
    let family = append_aggregate_family(
        &mut nodes,
        &[Element {
            weight: -1,
            condition: 2,
        }],
        &[],
        FamilyLimits {
            aggregate: AggregateLimits {
                max_nodes: original.view().len(),
                max_states: 0,
                max_subsets: 0,
                ..AggregateLimits::default()
            },
            max_guards: 0,
        },
        &Cancellation::default(),
    )
    .unwrap();
    assert!(family.roots().is_empty());
    assert_eq!(family.appended_nodes(), 0);
    assert_eq!(snapshot(&nodes), snapshot(&original));
    assert_eq!(
        limited(
            &[Element {
                weight: 1,
                condition: usize::MAX
            }],
            &[],
            FamilyLimits::default()
        ),
        Error::InvalidCondition { element: 0 }
    );
    let mut bad = raw(vec![Node::and_pair([0, 0])]);
    assert_eq!(
        append_aggregate_family(
            &mut bad,
            &[],
            &[],
            FamilyLimits::default(),
            &Cancellation::default()
        )
        .unwrap_err()
        .kind(),
        Error::InvalidPrefix { node: 0 }
    );
    assert_eq!(
        snapshot(&bad),
        (
            vec![Node::and_pair([0, 0])],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![]
        )
    );
}

#[test]
fn cancellation_and_deadlines_do_not_produce_empty_success() {
    let cancelled = Cancellation::default();
    cancelled.cancel();
    for (cancellation, expected) in [
        (cancelled, Stop::Cancelled),
        (
            Cancellation::with_deadline(Instant::now()).unwrap(),
            Stop::Deadline,
        ),
    ] {
        let mut nodes = zetesis_ferraris::FormulaNodes::default();
        let error = append_aggregate_family(
            &mut nodes,
            &[],
            &[],
            FamilyLimits {
                aggregate: AggregateLimits {
                    max_work: 0,
                    ..AggregateLimits::default()
                },
                max_guards: 0,
            },
            &cancellation,
        )
        .unwrap_err();
        assert_eq!(error.kind(), Error::Control(expected));
        assert_eq!(error.statistics().work, 0);
        assert!(nodes.view().is_empty());
        assert!(nodes.parts().operands().is_empty());
    }
}
