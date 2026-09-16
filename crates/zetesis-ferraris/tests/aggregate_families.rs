//! Shared families are compared with independent full failing-subset formulas.

use std::time::Instant;

use proptest::prelude::*;
use zetesis_cpu::{Control, Stop};
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison as Comparison, AggregateElement as Element,
    AggregateErrorKind as Error, AggregateFamilyLimits as FamilyLimits, AggregateGuard as Guard,
    AggregateLimits, AggregateProfile, Interpretation, Limits, Node, Theory, append_aggregate,
    append_aggregate_family, models, models_reduct,
};

const COMPARISONS: [Comparison; 6] = [
    Comparison::Eq,
    Comparison::Ne,
    Comparison::Lt,
    Comparison::Le,
    Comparison::Gt,
    Comparison::Ge,
];

fn prefix() -> Vec<Node> {
    vec![
        Node::False,
        Node::Implies(0, 0),
        Node::Atom(0),
        Node::Atom(1),
        Node::Implies(2, 0),
        Node::Implies(4, 0),
        Node::Implies(3, 0),
        Node::And(2, 3),
        Node::Or(2, 3),
        Node::Implies(2, 3),
        Node::Or(2, 4),
    ]
}

// Recursive evaluation is only the small independent test interpreter. It
// materializes neither the production topological mask nor its witness formula.
fn eval(nodes: &[Node], root: usize, world: u8, frozen: Option<u8>) -> bool {
    if frozen.is_some_and(|candidate| !eval(nodes, root, candidate, None)) {
        return false;
    }
    match nodes[root] {
        Node::False => false,
        Node::Atom(atom) => world & (1 << atom) != 0,
        Node::And(a, b) => eval(nodes, a, world, frozen) && eval(nodes, b, world, frozen),
        Node::Or(a, b) => eval(nodes, a, world, frozen) || eval(nodes, b, world, frozen),
        Node::Implies(a, b) => !eval(nodes, a, world, frozen) || eval(nodes, b, world, frozen),
    }
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

fn push(nodes: &mut Vec<Node>, node: Node) -> usize {
    let index = nodes.len();
    nodes.push(node);
    index
}

fn reference(elements: &[Element], comparison: Comparison, bound: i64) -> (Vec<Node>, usize) {
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
                consequent = push(&mut nodes, Node::Or(consequent, element.condition));
            } else {
                antecedent = push(&mut nodes, Node::And(antecedent, element.condition));
            }
        }
        let implication = push(&mut nodes, Node::Implies(antecedent, consequent));
        root = push(&mut nodes, Node::And(root, implication));
    }
    (nodes, root)
}

fn world(theory: &Theory, mask: u8) -> Interpretation {
    Interpretation::new(theory, (0..2).filter(|atom| mask & (1 << atom) != 0)).unwrap()
}

fn verify(elements: &[Element], guards: &[Guard]) {
    let input = prefix();
    let mut nodes = input.clone();
    let family = append_aggregate_family(
        &mut nodes,
        elements,
        guards,
        FamilyLimits::default(),
        &Control::default(),
    )
    .unwrap();
    assert_eq!(family.roots().len(), guards.len());
    for (&root, guard) in family.roots().iter().zip(guards) {
        let theory = Theory::new(2, nodes.clone(), vec![root], AdmissionLimits::default()).unwrap();
        let (reference, reference_root) = reference(elements, guard.comparison, guard.bound);
        for outer in 0..4 {
            let sum = elements
                .iter()
                .filter(|element| eval(&input, element.condition, outer, None))
                .map(|element| i128::from(element.weight))
                .sum();
            let classical = holds(guard.comparison, sum, guard.bound);
            let candidate = world(&theory, outer);
            assert_eq!(eval(&reference, reference_root, outer, None), classical);
            assert_eq!(
                models(&theory, &candidate, Limits::default(), &Control::default()).unwrap(),
                classical
            );
            for inner in 0..4 {
                let sum = elements
                    .iter()
                    .filter(|element| eval(&input, element.condition, inner, Some(outer)))
                    .map(|element| i128::from(element.weight))
                    .sum();
                let expected = classical && holds(guard.comparison, sum, guard.bound);
                assert_eq!(
                    eval(&reference, reference_root, inner, Some(outer)),
                    expected
                );
                assert_eq!(
                    models_reduct(
                        &theory,
                        &candidate,
                        &world(&theory, inner),
                        Limits::default(),
                        &Control::default()
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
        &Control::default(),
    )
    .unwrap();
    assert_eq!(family.statistics().states, 4);
    assert_eq!(family.statistics().subsets, 0);
}

#[test]
fn ordered_duplicate_guards_use_one_threshold_table_without_subsets() {
    let input: Vec<_> = (0..16).map(Node::Atom).collect();
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
    let mut nodes = input.clone();
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
        &Control::default(),
    )
    .unwrap();
    assert_eq!(family.profile(), AggregateProfile::Threshold);
    assert_eq!(family.statistics().states, 34);
    assert_eq!(family.statistics().subsets, 0);
    let isolated_work: u64 = guards
        .iter()
        .map(|guard| {
            let mut isolated = input.clone();
            append_aggregate(
                &mut isolated,
                &elements,
                guard.comparison,
                guard.bound,
                AggregateLimits::default(),
                &Control::default(),
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
        let theory =
            Theory::new(16, nodes.clone(), vec![root], AdmissionLimits::default()).unwrap();
        for count in [0, 7, 8, 9, 16] {
            let candidate = Interpretation::new(&theory, 0..count).unwrap();
            assert_eq!(
                models(&theory, &candidate, Limits::default(), &Control::default()).unwrap(),
                i64::try_from(count).unwrap() == guard.bound
            );
        }
    }
}

fn limited(elements: &[Element], guards: &[Guard], limits: FamilyLimits) -> Error {
    let original = prefix();
    let mut nodes = original.clone();
    let error = append_aggregate_family(&mut nodes, elements, guards, limits, &Control::default())
        .unwrap_err();
    assert_eq!(
        nodes, original,
        "an incomplete family must roll back every appended node"
    );
    error.kind()
}

#[test]
fn family_guard_node_work_state_and_element_ceilings_are_inclusive() {
    let elements = [
        Element {
            weight: 1,
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
            bound: 1,
        },
        Guard {
            comparison: Comparison::Ne,
            bound: 2,
        },
    ];
    let mut nodes = prefix();
    let family = append_aggregate_family(
        &mut nodes,
        &elements,
        &guards,
        FamilyLimits::default(),
        &Control::default(),
    )
    .unwrap();
    let exact = FamilyLimits {
        aggregate: AggregateLimits {
            max_elements: elements.len(),
            max_nodes: nodes.len(),
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
            &Control::default()
        )
        .unwrap(),
        family
    );
    assert_eq!(repeated, nodes);
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
                max_elements: 1,
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
    let family =
        append_aggregate_family(&mut nodes, &elements, &guards, exact, &Control::default())
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
        &Control::default(),
    )
    .unwrap_err();
    assert_eq!(error.statistics().subsets, 8);
    assert!(error.statistics().nodes > 2);
    assert_eq!(partial, prefix());
}

#[test]
fn empty_family_still_validates_and_appends_nothing() {
    let original = prefix();
    let mut nodes = original.clone();
    let family = append_aggregate_family(
        &mut nodes,
        &[Element {
            weight: -1,
            condition: 2,
        }],
        &[],
        FamilyLimits {
            aggregate: AggregateLimits {
                max_nodes: original.len(),
                max_states: 0,
                max_subsets: 0,
                ..AggregateLimits::default()
            },
            max_guards: 0,
        },
        &Control::default(),
    )
    .unwrap();
    assert!(family.roots().is_empty());
    assert_eq!(family.appended_nodes(), 0);
    assert_eq!(nodes, original);
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
    let mut bad = vec![Node::And(0, 0)];
    assert_eq!(
        append_aggregate_family(
            &mut bad,
            &[],
            &[],
            FamilyLimits::default(),
            &Control::default()
        )
        .unwrap_err()
        .kind(),
        Error::InvalidPrefix { node: 0 }
    );
    assert_eq!(bad, vec![Node::And(0, 0)]);
}

#[test]
fn cancellation_and_deadlines_do_not_produce_empty_success() {
    let cancelled = Control::default();
    cancelled.cancel();
    for (control, expected) in [
        (cancelled, Stop::Cancelled),
        (
            Control::with_deadline(Instant::now()).unwrap(),
            Stop::Deadline,
        ),
    ] {
        let mut nodes = Vec::new();
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
            &control,
        )
        .unwrap_err();
        assert_eq!(error.kind(), Error::Control(expected));
        assert_eq!(error.statistics().work, 0);
        assert!(nodes.is_empty());
    }
}
