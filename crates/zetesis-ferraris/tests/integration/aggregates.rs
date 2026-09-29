//! Formula and frozen-reduct comparison with independent finite aggregate semantics.

use std::time::Instant;

use crate::support::aggregate_theories::{COMPARISONS, prefix};
use crate::support::worlds::{eval, interpretation};
use proptest::prelude::*;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison as Comparison, AggregateElement as Element,
    AggregateErrorKind as Error, AggregateLimits, AggregateProfile, Interpretation, Limits, Node,
    Theory, append_aggregate, models, models_reduct,
};

fn holds(comparison: Comparison, sum: i64, bound: i64) -> bool {
    match comparison {
        Comparison::Eq => sum == bound,
        Comparison::Ne => sum != bound,
        Comparison::Lt => sum < bound,
        Comparison::Le => sum <= bound,
        Comparison::Gt => sum > bound,
        Comparison::Ge => sum >= bound,
    }
}

// Recursive interpretation is confined to this fixed shallow test prefix.
// It does not share the production kernel's topological masks or aggregate DAG.

fn verify(elements: &[Element], comparison: Comparison, bound: i64) {
    let input = prefix();
    let mut nodes = input.clone();
    let built = append_aggregate(
        &mut nodes,
        elements,
        comparison,
        bound,
        AggregateLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let theory = Theory::new(2, nodes, vec![built.root()], AdmissionLimits::default()).unwrap();
    for outer in 0..4 {
        let sum: i64 = elements
            .iter()
            .filter(|element| eval(&input, element.condition, outer, None))
            .map(|element| i64::from(element.weight))
            .sum();
        let classical = holds(comparison, sum, bound);
        let candidate = interpretation(&theory, outer);
        assert_eq!(
            models(
                &theory,
                &candidate,
                Limits::default(),
                &Cancellation::default()
            )
            .unwrap(),
            classical,
            "classical: {elements:?}, {comparison:?}, bound={bound}, M={outer}"
        );
        // Includes tested interpretations that are NOT subsets of M. The
        // aggregate abbreviation must agree with the explicit reduct there too.
        for inner in 0..4 {
            let sum: i64 = elements
                .iter()
                .filter(|element| eval(&input, element.condition, inner, Some(outer)))
                .map(|element| i64::from(element.weight))
                .sum();
            let expected = classical && holds(comparison, sum, bound);
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
                "reduct: {elements:?}, {comparison:?}, bound={bound}, M={outer}, J={inner}"
            );
        }
    }
}

#[test]
fn exhaustive_small_signed_and_nonnegative_aggregates_match_frozen_condition_semantics() {
    for left in -2..=2 {
        for right in -2..=2 {
            for second_condition in [2, 3, 4, 5, 9, 10] {
                let elements = [
                    Element {
                        weight: left,
                        condition: 2,
                    },
                    Element {
                        weight: right,
                        condition: second_condition,
                    },
                ];
                for comparison in COMPARISONS {
                    for bound in -3..=3 {
                        verify(&elements, comparison, bound);
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(192))]

    #[test]
    fn generated_composite_conditions_preserve_all_frozen_reduct_worlds(
        entries in prop::collection::vec((-3i32..=3, 0usize..11), 0..=5),
        comparison in 0usize..6,
        bound in -9i64..=9,
    ) {
        let elements: Vec<_> = entries.into_iter()
            .map(|(weight, condition)| Element { weight, condition }).collect();
        verify(&elements, COMPARISONS[comparison], bound);
    }
}

#[test]
fn empty_zero_weight_and_extreme_scalar_guards_have_exact_truth_and_reducts() {
    for comparison in COMPARISONS {
        for bound in [i64::MIN, -1, 0, 1, i64::MAX] {
            verify(&[], comparison, bound);
            verify(
                &[Element {
                    weight: 0,
                    condition: 10,
                }],
                comparison,
                bound,
            );
            verify(
                &[Element {
                    weight: i32::MIN,
                    condition: 4,
                }],
                comparison,
                bound,
            );
        }
    }
}

#[test]
fn count_sixty_four_uses_threshold_states_without_subset_enumeration() {
    let mut nodes: Vec<_> = (0..64).map(Node::Atom).collect();
    let elements: Vec<_> = (0..64)
        .map(|condition| Element {
            weight: 1,
            condition,
        })
        .collect();
    let built = append_aggregate(
        &mut nodes,
        &elements,
        Comparison::Eq,
        8,
        AggregateLimits {
            max_subsets: 0,
            ..AggregateLimits::default()
        },
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(built.profile(), AggregateProfile::Threshold);
    assert_eq!(built.statistics().subsets, 0);
    assert_eq!(built.statistics().states, 20);
    assert!(built.appended_nodes() < 2_200);
    let theory = Theory::new(64, nodes, vec![built.root()], AdmissionLimits::default()).unwrap();
    for size in [0, 7, 8, 9, 64] {
        let candidate = Interpretation::new(&theory, 0..size).unwrap();
        assert_eq!(
            models(
                &theory,
                &candidate,
                Limits::default(),
                &Cancellation::default()
            )
            .unwrap(),
            size == 8
        );
    }
}

fn limited(elements: &[Element], limits: AggregateLimits) -> Error {
    let original = prefix();
    let mut nodes = original.clone();
    let error = append_aggregate(
        &mut nodes,
        elements,
        Comparison::Eq,
        1,
        limits,
        &Cancellation::default(),
    )
    .expect_err("one inclusive budget is below required work");
    assert_eq!(
        nodes, original,
        "rollback must preserve the entire existing DAG"
    );
    error.kind()
}

#[test]
fn exact_compilation_ceilings_and_rollback_are_observable() {
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
    let mut nodes = prefix();
    let built = append_aggregate(
        &mut nodes,
        &elements,
        Comparison::Eq,
        1,
        AggregateLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let exact = AggregateLimits {
        max_elements: elements.len(),
        max_nodes: nodes.len(),
        max_work: built.statistics().work,
        max_states: built.statistics().states,
        max_subsets: 0,
    };
    let mut second = prefix();
    let repeated = append_aggregate(
        &mut second,
        &elements,
        Comparison::Eq,
        1,
        exact,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(built, repeated);
    assert_eq!(nodes, second);
    assert_eq!(
        limited(
            &elements,
            AggregateLimits {
                max_elements: 1,
                ..exact
            }
        ),
        Error::ElementLimit
    );
    assert_eq!(
        limited(
            &elements,
            AggregateLimits {
                max_nodes: exact.max_nodes - 1,
                ..exact
            }
        ),
        Error::NodeLimit
    );
    assert_eq!(
        limited(
            &elements,
            AggregateLimits {
                max_work: exact.max_work - 1,
                ..exact
            }
        ),
        Error::WorkLimit
    );
    assert_eq!(
        limited(
            &elements,
            AggregateLimits {
                max_states: exact.max_states - 1,
                ..exact
            }
        ),
        Error::StateLimit
    );
}

#[test]
fn exact_subset_ceiling_and_temporary_state_are_independent() {
    let signed = [
        Element {
            weight: -1,
            condition: 2,
        },
        Element {
            weight: 2,
            condition: 3,
        },
    ];
    let mut nodes = prefix();
    let built = append_aggregate(
        &mut nodes,
        &signed,
        Comparison::Eq,
        1,
        AggregateLimits {
            max_subsets: 4,
            max_states: 2,
            ..AggregateLimits::default()
        },
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(built.profile(), AggregateProfile::SubsetImplications);
    assert_eq!(built.statistics().subsets, 4);
    assert_eq!(
        limited(
            &signed,
            AggregateLimits {
                max_subsets: 3,
                ..AggregateLimits::default()
            }
        ),
        Error::SubsetLimit
    );
    assert_eq!(
        limited(
            &signed,
            AggregateLimits {
                max_states: 1,
                ..AggregateLimits::default()
            }
        ),
        Error::StateLimit
    );
}

#[test]
fn hostile_dimensions_and_edges_are_refused_without_mutation() {
    assert_eq!(
        limited(
            &[Element {
                weight: 1,
                condition: usize::MAX
            }],
            AggregateLimits::default()
        ),
        Error::InvalidCondition { element: 0 }
    );
    let mut nodes = vec![Node::And(0, 0)];
    let before = nodes.clone();
    assert_eq!(
        append_aggregate(
            &mut nodes,
            &[],
            Comparison::Eq,
            0,
            AggregateLimits::default(),
            &Cancellation::default()
        )
        .unwrap_err()
        .kind(),
        Error::InvalidPrefix { node: 0 }
    );
    assert_eq!(nodes, before);
    let signed = vec![
        Element {
            weight: -1,
            condition: 2
        };
        64
    ];
    assert_eq!(
        limited(
            &signed,
            AggregateLimits {
                max_subsets: u64::MAX,
                ..AggregateLimits::default()
            }
        ),
        Error::SubsetLimit
    );
    let huge = [Element {
        weight: i32::MAX,
        condition: 2,
    }];
    let mut nodes = prefix();
    assert_eq!(
        append_aggregate(
            &mut nodes,
            &huge,
            Comparison::Ge,
            i64::from(i32::MAX),
            AggregateLimits::default(),
            &Cancellation::default()
        )
        .unwrap_err()
        .kind(),
        Error::StateLimit
    );
    assert_eq!(nodes, prefix());
}

#[test]
fn cancellation_and_deadlines_precede_work_even_for_an_empty_aggregate() {
    let cancelled = Cancellation::default();
    cancelled.cancel();
    for (cancellation, expected) in [
        (cancelled, Stop::Cancelled),
        (
            Cancellation::with_deadline(Instant::now()).unwrap(),
            Stop::Deadline,
        ),
    ] {
        let mut nodes = Vec::new();
        let error = append_aggregate(
            &mut nodes,
            &[],
            Comparison::Eq,
            0,
            AggregateLimits {
                max_work: 0,
                ..AggregateLimits::default()
            },
            &cancellation,
        )
        .unwrap_err();
        assert_eq!(error.kind(), Error::Control(expected));
        assert_eq!(error.statistics().work, 0);
        assert!(nodes.is_empty());
    }
}
