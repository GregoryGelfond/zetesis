//! Public aggregate failures retain evidence and never damage reusable input DAGs.

use crate::support::aggregate_theories::{copy, raw, snapshot};
use std::error::Error as _;
use std::time::Instant;

use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison as Comparison, AggregateElement as Element,
    AggregateErrorKind, AggregateFamilyLimits, AggregateGuard, AggregateLimits, Interpretation,
    Limits, Node, Theory, append_aggregate, append_aggregate_family, models, models_reduct,
};

fn retry_and_verify(nodes: &mut zetesis_ferraris::FormulaNodes, elements: &[Element]) {
    let built = append_aggregate(
        nodes,
        elements,
        Comparison::Eq,
        1,
        AggregateLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let theory = Theory::new(
        2,
        copy(nodes).into_parts(),
        vec![built.root()],
        AdmissionLimits::default(),
    )
    .unwrap();
    for outer in 0_u8..4 {
        let candidate =
            Interpretation::new(&theory, (0..2).filter(|atom| outer & (1 << atom) != 0)).unwrap();
        let sum = |mask: u8| {
            elements
                .iter()
                .filter(|element| mask & (1 << element.condition) != 0)
                .map(|element| i64::from(element.weight))
                .sum::<i64>()
        };
        assert_eq!(
            models(
                &theory,
                &candidate,
                Limits::default(),
                &Cancellation::default()
            )
            .unwrap(),
            sum(outer) == 1
        );
        for inner in 0_u8..4 {
            let tested =
                Interpretation::new(&theory, (0..2).filter(|atom| inner & (1 << atom) != 0))
                    .unwrap();
            assert_eq!(
                models_reduct(
                    &theory,
                    &candidate,
                    &tested,
                    Limits::default(),
                    &Cancellation::default()
                )
                .unwrap(),
                sum(outer) == 1 && sum(inner & outer) == 1
            );
        }
    }
}

#[test]
fn refused_aggregate_resources_keep_a_reusable_prefix_and_named_reason() {
    let prefix = raw(vec![Node::atom(0), Node::atom(1)]);
    for (weights, limits, kind, phrase) in [
        (
            [1, 2],
            AggregateLimits {
                max_elements: 1,
                ..Default::default()
            },
            AggregateErrorKind::ElementLimit,
            "element limit",
        ),
        (
            [1, 2],
            AggregateLimits {
                max_nodes: 2,
                ..Default::default()
            },
            AggregateErrorKind::NodeLimit,
            "node limit",
        ),
        (
            [1, 2],
            AggregateLimits {
                max_work: 8,
                ..Default::default()
            },
            AggregateErrorKind::WorkLimit,
            "work limit",
        ),
        (
            [1, 2],
            AggregateLimits {
                max_states: 0,
                ..Default::default()
            },
            AggregateErrorKind::StateLimit,
            "state limit",
        ),
        (
            [-1, 2],
            AggregateLimits {
                max_subsets: 3,
                ..Default::default()
            },
            AggregateErrorKind::SubsetLimit,
            "subset limit",
        ),
    ] {
        let elements = [
            Element {
                weight: weights[0],
                condition: 0,
            },
            Element {
                weight: weights[1],
                condition: 1,
            },
        ];
        let mut nodes = copy(&prefix);
        let error = append_aggregate(
            &mut nodes,
            &elements,
            Comparison::Eq,
            1,
            limits,
            &Cancellation::default(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), kind);
        assert!(error.to_string().contains(phrase), "{error}");
        assert!(error.source().is_none());
        assert_eq!(
            snapshot(&nodes),
            snapshot(&prefix),
            "no new root may escape a failed transaction"
        );
        assert!(error.statistics().work <= limits.max_work);
        retry_and_verify(&mut nodes, &elements);
    }
}

#[test]
fn invalid_indices_and_guard_capacity_have_actionable_local_evidence() {
    let mut malformed = raw(vec![Node::and_pair([0, 0])]);
    let error = append_aggregate(
        &mut malformed,
        &[],
        Comparison::Eq,
        0,
        AggregateLimits::default(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), AggregateErrorKind::InvalidPrefix { node: 0 });
    assert!(
        error
            .to_string()
            .contains("invalid storage or edges at node 0")
    );
    assert_eq!(snapshot(&malformed), (vec![Node::and_pair([0, 0])], vec![]));
    let mut nodes = raw(vec![Node::atom(0), Node::atom(1)]);
    let before = copy(&nodes);
    let invalid = [Element {
        weight: 1,
        condition: 2,
    }];
    let error = append_aggregate(
        &mut nodes,
        &invalid,
        Comparison::Eq,
        1,
        AggregateLimits::default(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        AggregateErrorKind::InvalidCondition { element: 0 }
    );
    assert!(error.to_string().contains("element 0"));
    assert_eq!(snapshot(&nodes), snapshot(&before));
    let elements = [Element {
        weight: 1,
        condition: 0,
    }];
    let guards = [AggregateGuard {
        comparison: Comparison::Eq,
        bound: 1,
    }];
    let error = append_aggregate_family(
        &mut nodes,
        &elements,
        &guards,
        AggregateFamilyLimits {
            max_guards: 0,
            ..Default::default()
        },
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), AggregateErrorKind::GuardLimit);
    assert!(error.to_string().contains("guard limit"));
    assert_eq!(snapshot(&nodes), snapshot(&before));
    retry_and_verify(&mut nodes, &elements);
}

#[test]
fn aggregate_control_causes_remain_downcastable_and_do_not_poison_retry() {
    let cancelled = Cancellation::default();
    cancelled.cancel();
    for (cancellation, expected) in [
        (cancelled, Stop::Cancelled),
        (
            Cancellation::with_deadline(Instant::now()).unwrap(),
            Stop::Deadline,
        ),
    ] {
        let mut nodes = raw(vec![Node::atom(0), Node::atom(1)]);
        let original = copy(&nodes);
        let elements = [Element {
            weight: 1,
            condition: 0,
        }];
        let error = append_aggregate(
            &mut nodes,
            &elements,
            Comparison::Eq,
            1,
            AggregateLimits::default(),
            &cancellation,
        )
        .unwrap_err();
        assert_eq!(error.kind(), AggregateErrorKind::Control(expected));
        assert_eq!(
            error.source().unwrap().downcast_ref::<Stop>(),
            Some(&expected)
        );
        assert_eq!(error.to_string(), expected.to_string());
        assert_eq!(error.statistics().work, 0);
        assert_eq!(snapshot(&nodes), snapshot(&original));
        retry_and_verify(&mut nodes, &elements);
    }
}
