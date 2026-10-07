//! Retained topology checks preserve aggregate output and final theory admission.

mod admission;

use std::time::Instant;

use proptest::prelude::*;
use zetesis_core::Value;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{
    AggregateBuild, AggregateComparison as Comparison, AggregateElement as Element, AggregateError,
    AggregateErrorKind as Error, AggregateExtremum as Extremum, AggregateFamilyBuild,
    AggregateFamilyLimits as FamilyLimits, AggregateGuard as Guard, AggregateLimits, FormulaNodes,
    FormulaParts, Node, NodeView, ValueExtremumElement, append_aggregate, append_aggregate_family,
    append_extremum, append_value_extremum, append_value_extremum_refs,
};

use crate::support::aggregate_theories::{COMPARISONS, copy, prefix, push, snapshot};
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
    let mut nodes = prefix();
    scan(&mut nodes, AggregateLimits::default()).unwrap();
    nodes
}

// A fresh raw owner must check every node and every logical child occurrence.
fn size(nodes: &FormulaNodes) -> u64 {
    u64::try_from(nodes.view().len() + nodes.parts().occurrences()).unwrap()
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
fn checked_appends_retain_completed_validation() {
    let input = prefix();
    let mut nodes = copy(&input);
    let first = scan(&mut nodes, AggregateLimits::default()).unwrap();
    assert_eq!(first.statistics().work, 1 + size(&input));
    assert_eq!(
        scan(&mut nodes, AggregateLimits::default())
            .unwrap()
            .statistics()
            .work,
        1
    );
    push(&mut nodes, NodeView::And(&[2, 3]));
    push(&mut nodes, NodeView::Implies(0, 0));
    assert_eq!(
        scan(&mut nodes, AggregateLimits::default())
            .unwrap()
            .statistics()
            .work,
        1
    );
    assert_eq!(
        &nodes.parts().nodes()[..input.view().len()],
        input.parts().nodes()
    );
    assert_eq!(nodes.parts().operands(), input.parts().operands());
}

#[test]
fn independent_families_preserve_frozen_reducts() {
    let mut nodes = prefix();
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
            raw = copy(&raw);
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
            assert_eq!(snapshot(&nodes), snapshot(&raw));
            assert_eq!(actual.roots(), expected.roots());
            assert_eq!(actual.profile(), expected.profile());
            let mut statistics = expected.statistics();
            statistics.work -= previously_checked;
            assert_eq!(actual.statistics(), statistics);
            previously_checked = size(&nodes);
            for (&root, &reference) in actual.roots().iter().zip(expected.roots()) {
                for outer in 0..4 {
                    assert_eq!(
                        eval(nodes.view(), root, outer, None),
                        eval(raw.view(), reference, outer, None)
                    );
                    for inner in 0..4 {
                        assert_eq!(
                            eval(nodes.view(), root, inner, Some(outer)),
                            eval(raw.view(), reference, inner, Some(outer))
                        );
                    }
                }
            }
        }
    }
}

fn compare_build(actual: AggregateBuild, expected: AggregateBuild, checked: u64) {
    assert_eq!(actual.root(), expected.root());
    assert_eq!(actual.profile(), expected.profile());
    let mut statistics = expected.statistics();
    statistics.work -= checked;
    assert_eq!(actual.statistics(), statistics);
}

#[test]
fn numeric_compilers_share_completed_prefix_checks() {
    let mut nodes = checked();
    let mut raw = prefix();
    let limits = AggregateLimits::default();
    let control = Cancellation::default();
    let mut retained = size(&raw);
    compare_build(
        nodes
            .append_aggregate(&elements(-1), Comparison::Ne, 1, limits, &control)
            .unwrap(),
        append_aggregate(&mut raw, &elements(-1), Comparison::Ne, 1, limits, &control).unwrap(),
        retained,
    );
    retained = size(&raw);
    raw = copy(&raw);
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
    assert_eq!(snapshot(&nodes), snapshot(&raw));
    assert_eq!(scan(&mut nodes, limits).unwrap().statistics().work, 1);
}

#[test]
fn typed_compilers_share_completed_prefix_checks() {
    let mut nodes = checked();
    let mut raw = prefix();
    let limits = AggregateLimits::default();
    let control = Cancellation::default();
    let mut retained = size(&raw);
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
    retained = size(&raw);
    raw = copy(&raw);
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
    assert_eq!(snapshot(&nodes), snapshot(&raw));
    assert_eq!(scan(&mut nodes, limits).unwrap().statistics().work, 1);
}

#[test]
fn incomplete_scans_publish_no_new_frontier() {
    let mut nodes = prefix();
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
    let expected = 1 + size(&nodes);
    assert_eq!(
        scan(&mut nodes, AggregateLimits::default())
            .unwrap()
            .statistics()
            .work,
        expected
    );
}

#[test]
fn malformed_raw_suffix_survives_failed_validation() {
    let input = prefix();
    let (mut raw, operands) = snapshot(&input);
    let invalid = raw.len();
    raw.push(Node::and_pair([0, invalid]));
    let mut nodes = FormulaNodes::new(FormulaParts::new(raw, operands).unwrap());
    let original = snapshot(&nodes);
    for _ in 0..2 {
        let failure = scan(&mut nodes, AggregateLimits::default()).unwrap_err();
        assert_eq!(failure.kind(), Error::InvalidPrefix { node: invalid });
        assert_eq!(snapshot(&nodes), original);
    }
}

#[test]
fn detached_suffix_restores_paired_input() {
    let mut nodes = checked();
    let original = snapshot(&nodes);
    let first = nodes.view().len();
    let mut transaction = nodes.transaction();
    let row = [2, 3, 4, 5];
    assert_eq!(
        transaction
            .push(NodeView::And(&row), usize::MAX, usize::MAX)
            .unwrap(),
        first
    );
    let suffix = transaction.detach().unwrap();
    assert_eq!(suffix.first(), first);
    assert_eq!(suffix.view().node(0).unwrap(), NodeView::And(&row));
    assert_eq!(snapshot(&nodes), original);
    assert_eq!(
        scan(&mut nodes, AggregateLimits::default())
            .unwrap()
            .statistics()
            .work,
        1
    );
}

#[test]
fn extracted_parts_carry_no_validation_evidence() {
    let nodes = checked();
    let address = nodes.parts().nodes().as_ptr();
    let operands = nodes.parts().operands().as_ptr();
    let capacity = nodes.parts().node_capacity();
    let operand_capacity = nodes.parts().operand_capacity();
    let parts = nodes.into_parts();
    assert_eq!(parts.nodes().as_ptr(), address);
    assert_eq!(parts.operands().as_ptr(), operands);
    assert_eq!(parts.node_capacity(), capacity);
    assert_eq!(parts.operand_capacity(), operand_capacity);
    let mut wrapped = FormulaNodes::new(parts);
    let expected = 1 + size(&wrapped);
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
        let original = snapshot(&nodes);
        let failure = nodes
            .append_aggregate_family(&[], &[], FamilyLimits::default(), &control)
            .unwrap_err();
        assert_eq!(failure.kind(), Error::Control(stop));
        assert_eq!(failure.statistics().work, 0);
        assert_eq!(snapshot(&nodes), original);
    }
}

#[test]
fn reused_prefix_still_obeys_total_node_ceiling() {
    let mut nodes = checked();
    let count = nodes.view().len();
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
        let original = snapshot(&nodes);
        let failure = compile(&mut nodes, max_work).unwrap_err();
        assert_eq!(failure.kind(), Error::WorkLimit);
        assert_eq!(failure.statistics().work, max_work);
        saw_append |= failure.statistics().nodes > 0;
        assert_eq!(snapshot(&nodes), original);
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
    assert_eq!(snapshot(&nodes), snapshot(&complete));
}

#[test]
fn refusal_restores_the_initial_validation_frontier() {
    let mut nodes = prefix();
    let original = snapshot(&nodes);
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
    assert_eq!(snapshot(&nodes), original);
    assert_eq!(
        scan(&mut nodes, AggregateLimits::default())
            .unwrap()
            .statistics()
            .work,
        1 + size(&nodes)
    );
}

#[test]
fn reused_prefix_obeys_the_operand_ceiling() {
    let mut nodes = checked();
    push(&mut nodes, NodeView::Or(&[2, 3, 4, 5]));
    scan(&mut nodes, AggregateLimits::default()).unwrap();
    let original = snapshot(&nodes);
    let count = nodes.parts().occurrences();
    let failure = scan(
        &mut nodes,
        AggregateLimits {
            max_operands: count - 1,
            ..AggregateLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(failure.kind(), Error::OperandLimit);
    assert_eq!(failure.statistics().work, 1);
    assert_eq!(snapshot(&nodes), original);
    assert_eq!(
        scan(
            &mut nodes,
            AggregateLimits {
                max_operands: count,
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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]
    #[test]
    fn suffix_transactions_preserve_retained_validation(actions in prop::collection::vec((any::<bool>(), prop::collection::vec(0usize..11, 2..8)), 0..24)) {
        let mut nodes = checked();
        for (commit, row) in actions {
            let before = snapshot(&nodes);
            {
                let mut transaction = nodes.transaction();
                transaction.push(NodeView::And(&row), usize::MAX, usize::MAX).unwrap();
                if commit { transaction.commit(); }
            }
            if !commit { prop_assert_eq!(snapshot(&nodes), before); }
            let actual = scan(&mut nodes, AggregateLimits::default()).unwrap();
            prop_assert_eq!(actual.statistics().work, 1);
            let mut cold = copy(&nodes);
            let expected = scan(&mut cold, AggregateLimits::default()).unwrap();
            prop_assert_eq!(expected.statistics().work, 1 + size(&cold));
            prop_assert_eq!(snapshot(&nodes), snapshot(&cold));
        }
    }
}
