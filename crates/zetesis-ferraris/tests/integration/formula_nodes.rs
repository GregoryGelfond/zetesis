//! Retained topology checks preserve the aggregate compiler's exact output.

use std::time::Instant;

use proptest::prelude::*;
use zetesis_core::Value;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{
    AggregateBuild, AggregateComparison as Comparison, AggregateElement as Element, AggregateError,
    AggregateErrorKind as Error, AggregateExtremum as Extremum, AggregateFamilyBuild,
    AggregateFamilyLimits as FamilyLimits, AggregateGuard as Guard, AggregateLimits, FormulaNodes,
    Node, ValueExtremumElement, append_aggregate, append_aggregate_family, append_extremum,
    append_value_extremum, append_value_extremum_refs,
};

use crate::support::aggregate_theories::{COMPARISONS, prefix};
use crate::support::worlds::eval;

fn scan(
    nodes: &mut FormulaNodes,
    limits: AggregateLimits,
) -> Result<AggregateFamilyBuild, AggregateError> {
    nodes.append_aggregate_family(
        &[],
        &[],
        FamilyLimits {
            aggregate: limits,
            max_guards: 0,
        },
        &Cancellation::default(),
    )
}

fn checked() -> FormulaNodes {
    let mut nodes = FormulaNodes::new(prefix());
    scan(&mut nodes, AggregateLimits::default()).unwrap();
    nodes
}

fn elements(weight: i32) -> [Element; 2] {
    [
        Element {
            weight,
            condition: 4,
        },
        Element {
            weight: 2,
            condition: 9,
        },
    ]
}

fn guards() -> [Guard; 2] {
    [
        Guard {
            comparison: Comparison::Eq,
            bound: 1,
        },
        Guard {
            comparison: Comparison::Ne,
            bound: 2,
        },
    ]
}

#[test]
fn completed_scans_charge_only_new_nodes() {
    let input = prefix();
    let mut nodes = FormulaNodes::new(input.clone());
    let first = scan(&mut nodes, AggregateLimits::default()).unwrap();
    assert_eq!(
        first.statistics().work,
        1 + u64::try_from(input.len()).unwrap()
    );
    assert_eq!(
        scan(&mut nodes, AggregateLimits::default())
            .unwrap()
            .statistics()
            .work,
        1
    );
    nodes.push(Node::And(2, 3));
    nodes.push(Node::Implies(0, 0));
    assert_eq!(
        scan(&mut nodes, AggregateLimits::default())
            .unwrap()
            .statistics()
            .work,
        3
    );
    assert_eq!(&nodes[..input.len()], &input);
}

#[test]
fn independent_families_preserve_frozen_reducts() {
    let mut nodes = FormulaNodes::new(prefix());
    let mut raw = prefix();
    let mut previously_checked = 0;
    for weight in [-1, 0, 1] {
        for comparison in COMPARISONS {
            let guards = [
                Guard {
                    comparison,
                    bound: 1,
                },
                Guard {
                    comparison,
                    bound: -1,
                },
            ];
            let old_len = nodes.len();
            let actual = nodes
                .append_aggregate_family(
                    &elements(weight),
                    &guards,
                    FamilyLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            let expected = append_aggregate_family(
                &mut raw,
                &elements(weight),
                &guards,
                FamilyLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            assert_eq!(&*nodes, raw);
            assert_eq!(actual.roots(), expected.roots());
            assert_eq!(actual.profile(), expected.profile());
            let mut statistics = expected.statistics();
            statistics.work -= u64::try_from(previously_checked).unwrap();
            assert_eq!(actual.statistics(), statistics);
            previously_checked = old_len;
            for (&root, &reference) in actual.roots().iter().zip(expected.roots()) {
                for outer in 0..4 {
                    assert_eq!(
                        eval(&nodes, root, outer, None),
                        eval(&raw, reference, outer, None)
                    );
                    for inner in 0..4 {
                        assert_eq!(
                            eval(&nodes, root, inner, Some(outer)),
                            eval(&raw, reference, inner, Some(outer))
                        );
                    }
                }
            }
        }
    }
}

fn compare_build(actual: AggregateBuild, expected: AggregateBuild, checked: usize) {
    assert_eq!(actual.root(), expected.root());
    assert_eq!(actual.profile(), expected.profile());
    let mut statistics = expected.statistics();
    statistics.work -= u64::try_from(checked).unwrap();
    assert_eq!(actual.statistics(), statistics);
}

#[test]
fn numeric_compilers_share_completed_prefix_checks() {
    let mut nodes = checked();
    let mut raw = prefix();
    let limits = AggregateLimits::default();
    let control = Cancellation::default();
    let mut retained = raw.len();
    let previous = raw.len();
    compare_build(
        nodes
            .append_aggregate(&elements(-1), Comparison::Ne, 1, limits, &control)
            .unwrap(),
        append_aggregate(&mut raw, &elements(-1), Comparison::Ne, 1, limits, &control).unwrap(),
        retained,
    );
    retained = previous;
    let previous = raw.len();
    compare_build(
        nodes
            .append_extremum(
                &elements(1),
                Extremum::Min,
                Comparison::Ne,
                1,
                limits,
                &control,
            )
            .unwrap(),
        append_extremum(
            &mut raw,
            &elements(1),
            Extremum::Min,
            Comparison::Ne,
            1,
            limits,
            &control,
        )
        .unwrap(),
        retained,
    );
    assert_eq!(&*nodes, raw);
    let unchecked = nodes.len() - previous;
    assert_eq!(
        scan(&mut nodes, limits).unwrap().statistics().work,
        1 + u64::try_from(unchecked).unwrap()
    );
}

#[test]
fn typed_compilers_share_completed_prefix_checks() {
    let mut nodes = checked();
    let mut raw = prefix();
    let limits = AggregateLimits::default();
    let control = Cancellation::default();
    let retained = raw.len();
    let values = [
        ValueExtremumElement {
            value: Value::Symbol("a".into()),
            condition: 4,
        },
        ValueExtremumElement {
            value: Value::Number(1),
            condition: 9,
        },
    ];
    let bound = Value::Symbol("a".into());
    compare_build(
        nodes
            .append_value_extremum(
                &values,
                Extremum::Max,
                Comparison::Ne,
                &bound,
                limits,
                &control,
            )
            .unwrap(),
        append_value_extremum(
            &mut raw,
            &values,
            Extremum::Max,
            Comparison::Ne,
            &bound,
            limits,
            &control,
        )
        .unwrap(),
        retained,
    );
    let last_input = raw.len();
    let borrowed = || {
        values.iter().map(|element| ValueExtremumElement {
            value: (&element.value).into(),
            condition: element.condition,
        })
    };
    compare_build(
        nodes
            .append_value_extremum_refs(
                borrowed(),
                Extremum::Min,
                Comparison::Eq,
                (&bound).into(),
                limits,
                &control,
            )
            .unwrap(),
        append_value_extremum_refs(
            &mut raw,
            borrowed(),
            Extremum::Min,
            Comparison::Eq,
            (&bound).into(),
            limits,
            &control,
        )
        .unwrap(),
        retained,
    );
    assert_eq!(&*nodes, raw);
    let unchecked = nodes.len() - last_input;
    assert_eq!(
        scan(&mut nodes, limits).unwrap().statistics().work,
        1 + u64::try_from(unchecked).unwrap()
    );
}

#[test]
fn incomplete_scans_publish_no_new_frontier() {
    let mut nodes = FormulaNodes::new(prefix());
    let failure = scan(
        &mut nodes,
        AggregateLimits {
            max_work: 3,
            ..AggregateLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(failure.kind(), Error::WorkLimit);
    assert_eq!(failure.statistics().work, 3);
    let expected = 1 + u64::try_from(nodes.len()).unwrap();
    assert_eq!(
        scan(&mut nodes, AggregateLimits::default())
            .unwrap()
            .statistics()
            .work,
        expected
    );
}

#[test]
fn malformed_suffix_survives_failed_validation() {
    let mut nodes = checked();
    let original = nodes.to_vec();
    let invalid = nodes.len();
    nodes.push(Node::And(0, invalid));
    for _ in 0..2 {
        let failure = scan(&mut nodes, AggregateLimits::default()).unwrap_err();
        assert_eq!(failure.kind(), Error::InvalidPrefix { node: invalid });
        assert_eq!(failure.statistics().work, 2);
        assert_eq!(nodes[invalid], Node::And(0, invalid));
    }
    nodes.truncate(invalid);
    nodes.push(Node::And(2, 3));
    assert_eq!(
        scan(&mut nodes, AggregateLimits::default())
            .unwrap()
            .statistics()
            .work,
        2
    );
    assert_eq!(&nodes[..invalid], original);
}

#[test]
fn removed_suffixes_discard_their_validation() {
    for split in [false, true] {
        let mut nodes = checked();
        if split {
            assert_eq!(nodes.split_off(4), prefix()[4..]);
        } else {
            nodes.truncate(4);
        }
        nodes.push(Node::Or(4, 0));
        let failure = scan(&mut nodes, AggregateLimits::default()).unwrap_err();
        assert_eq!(failure.kind(), Error::InvalidPrefix { node: 4 });
        assert_eq!(failure.statistics().work, 2);
    }
}

#[test]
fn extracted_vectors_carry_no_validation_evidence() {
    let nodes = checked();
    let address = nodes.as_ptr();
    let capacity = nodes.capacity();
    let raw = nodes.into_vec();
    assert_eq!(raw.as_ptr(), address);
    assert_eq!(raw.capacity(), capacity);
    let mut wrapped = FormulaNodes::new(raw);
    let expected = 1 + u64::try_from(wrapped.len()).unwrap();
    assert_eq!(
        scan(&mut wrapped, AggregateLimits::default())
            .unwrap()
            .statistics()
            .work,
        expected
    );
}

#[test]
fn reused_prefix_still_obeys_control() {
    let cancelled = Cancellation::default();
    cancelled.cancel();
    for (control, stop) in [
        (cancelled, Stop::Cancelled),
        (
            Cancellation::with_deadline(Instant::now()).unwrap(),
            Stop::Deadline,
        ),
    ] {
        let mut nodes = checked();
        let original = nodes.to_vec();
        let failure = nodes
            .append_aggregate_family(&[], &[], FamilyLimits::default(), &control)
            .unwrap_err();
        assert_eq!(failure.kind(), Error::Control(stop));
        assert_eq!(failure.statistics().work, 0);
        assert_eq!(&*nodes, original);
    }
}

#[test]
fn reused_prefix_still_obeys_total_node_ceiling() {
    let mut nodes = checked();
    let count = nodes.len();
    let failure = scan(
        &mut nodes,
        AggregateLimits {
            max_nodes: count - 1,
            ..AggregateLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(failure.kind(), Error::NodeLimit);
    assert_eq!(failure.statistics().work, 1);
    assert_eq!(
        scan(
            &mut nodes,
            AggregateLimits {
                max_nodes: count,
                max_work: 1,
                ..AggregateLimits::default()
            }
        )
        .unwrap()
        .statistics()
        .work,
        1
    );
}

#[test]
fn every_work_refusal_restores_original_nodes() {
    let compile = |nodes: &mut FormulaNodes, max_work| {
        nodes.append_aggregate_family(
            &elements(1),
            &guards(),
            FamilyLimits {
                aggregate: AggregateLimits {
                    max_work,
                    ..AggregateLimits::default()
                },
                max_guards: 2,
            },
            &Cancellation::default(),
        )
    };
    let mut complete = checked();
    let exact = compile(&mut complete, u64::MAX).unwrap();
    let mut saw_append = false;
    for max_work in 0..exact.statistics().work {
        let mut nodes = checked();
        let original = nodes.to_vec();
        let failure = compile(&mut nodes, max_work).unwrap_err();
        assert_eq!(failure.kind(), Error::WorkLimit);
        assert_eq!(failure.statistics().work, max_work);
        saw_append |= failure.statistics().nodes > 0;
        assert_eq!(&*nodes, original);
        assert_eq!(
            scan(&mut nodes, AggregateLimits::default())
                .unwrap()
                .statistics()
                .work,
            1
        );
    }
    assert!(saw_append);
    let mut nodes = checked();
    assert_eq!(compile(&mut nodes, exact.statistics().work).unwrap(), exact);
    assert_eq!(&*nodes, &*complete);
}

#[test]
fn completed_validation_survives_later_refusal() {
    let mut nodes = FormulaNodes::new(prefix());
    let original = nodes.to_vec();
    let failure = nodes
        .append_aggregate(
            &elements(1),
            Comparison::Eq,
            1,
            AggregateLimits {
                max_states: 0,
                ..AggregateLimits::default()
            },
            &Cancellation::default(),
        )
        .unwrap_err();
    assert_eq!(failure.kind(), Error::StateLimit);
    assert!(failure.statistics().nodes > 0);
    assert_eq!(&*nodes, original);
    assert_eq!(
        scan(&mut nodes, AggregateLimits::default())
            .unwrap()
            .statistics()
            .work,
        1
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]
    #[test]
    fn suffix_edits_preserve_prefix_rejection(actions in prop::collection::vec((0u8..4, 0usize..24), 0..48)) {
        let mut nodes = FormulaNodes::default();
        let mut raw = Vec::new();
        for (action, index) in actions {
            match action {
                0 | 1 => {
                    let node = if action == 0 { Node::False } else { Node::Implies(index, 0) };
                    nodes.push(node);
                    raw.push(node);
                }
                2 => { nodes.truncate(index); raw.truncate(index); }
                _ => {
                    let at = index.min(raw.len());
                    prop_assert_eq!(nodes.split_off(at), raw.split_off(at));
                }
            }
            let actual = scan(&mut nodes, AggregateLimits::default()).map(|_| ()).map_err(AggregateError::kind);
            let expected = append_aggregate_family(&mut raw, &[], &[], FamilyLimits::default(), &Cancellation::default()).map(|_| ()).map_err(AggregateError::kind);
            prop_assert_eq!(actual, expected);
            prop_assert_eq!(&*nodes, &raw);
        }
    }
}
