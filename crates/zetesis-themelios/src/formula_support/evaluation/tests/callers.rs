//! Observe the actual join consumers, independently of evaluator unit tests.

use themelios_program::program::Relation;
use themelios_program::term::BinaryOp;
use zetesis_core::Value;

use super::location;
use crate::expansion::Budget;
use crate::formula_ir::{Expression, LiteralIr, Operation};
use crate::formula_support::{Comparisons, Counters, Join, Support};
use crate::{ExpansionFailure, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource};

fn number(value: i32) -> Expression {
    Expression {
        nodes: vec![Operation::Constant(Value::Number(value))],
    }
}

fn increment(variable: usize) -> Expression {
    Expression {
        nodes: vec![
            Operation::Variable(variable),
            Operation::Constant(Value::Number(1)),
            Operation::Binary(BinaryOp::Add, 0, 1),
        ],
    }
}

#[test]
fn final_filters_use_the_join_workspace() {
    // Tuple comparison bypasses prefix arithmetic, so only the final filter can
    // populate this initially empty workspace.
    let literals = [LiteralIr::TupleCompare(
        vec![increment(0)],
        Relation::Eq,
        vec![increment(0)],
    )];
    let support = Support::default();
    let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
    let mut join = Join::new(
        &literals,
        &[Value::Number(7)],
        1,
        &support,
        &mut budget,
        location(),
    )
    .unwrap();
    let result = join
        .next(
            &FormulaLimits::default(),
            &mut budget,
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    assert_eq!(result, Some(vec![Value::Number(7)]));
    assert!(join.evaluation.values.is_empty());
    assert!(join.evaluation.values.capacity() >= 2);
}

#[test]
fn binding_generators_use_the_join_workspace() {
    let literals = [
        LiteralIr::Range {
            target: 0,
            lower: number(0),
            upper: number(2),
            binder: true,
        },
        LiteralIr::Bind {
            target: 1,
            value: increment(0),
        },
    ];
    let support = Support::default();
    let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
    let mut counters = Counters::default();
    let mut join = Join::new(&literals, &[], 2, &support, &mut budget, location()).unwrap();
    for value in 0..=2 {
        assert_eq!(
            join.next(
                &FormulaLimits::default(),
                &mut budget,
                &mut counters,
                location(),
            )
            .unwrap(),
            Some(vec![Value::Number(value), Value::Number(value + 1)])
        );
        assert!(join.evaluation.values.is_empty());
        assert!(join.evaluation.values.capacity() >= 2);
    }
    assert_eq!(
        join.next(
            &FormulaLimits::default(),
            &mut budget,
            &mut counters,
            location(),
        )
        .unwrap(),
        None
    );
}

#[test]
fn range_endpoints_use_the_join_workspace() {
    let literals = [LiteralIr::Range {
        target: 1,
        lower: increment(0),
        upper: increment(0),
        binder: true,
    }];
    let support = Support::default();
    let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
    let mut join = Join::new(
        &literals,
        &[Value::Number(2)],
        2,
        &support,
        &mut budget,
        location(),
    )
    .unwrap();
    assert_eq!(
        join.next(
            &FormulaLimits::default(),
            &mut budget,
            &mut Counters::default(),
            location(),
        )
        .unwrap(),
        Some(vec![Value::Number(2), Value::Number(3)])
    );
    assert!(join.evaluation.values.is_empty());
    assert!(join.evaluation.values.capacity() >= 2);
}

#[test]
fn false_filters_do_not_hide_later_arithmetic_errors() {
    let literals = [
        LiteralIr::TupleCompare(vec![number(0)], Relation::Eq, vec![number(1)]),
        LiteralIr::TupleCompare(
            vec![Expression {
                nodes: vec![
                    Operation::Constant(Value::Number(1)),
                    Operation::Constant(Value::Number(0)),
                    Operation::Binary(BinaryOp::Div, 0, 1),
                ],
            }],
            Relation::Eq,
            vec![number(0)],
        ),
    ];
    let support = Support::default();
    let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
    let mut join = Join::new(&literals, &[], 0, &support, &mut budget, location()).unwrap();
    assert!(matches!(
        join.next(
            &FormulaLimits::default(),
            &mut budget,
            &mut Counters::default(),
            location(),
        ),
        Err(FormulaFailure::Expansion(
            ExpansionFailure::Evaluation { .. }
        ))
    ));
}

#[test]
fn stopped_filters_release_live_workspace_values() {
    let literals = [LiteralIr::Compare(increment(0), Relation::Eq, increment(0))];
    let support = Support::default();
    let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
    let mut join = Join::new(&literals, &[], 1, &support, &mut budget, location()).unwrap();
    let failure = join.filters(
        &[Value::Number(3)],
        Comparisons::Deferred,
        &FormulaLimits {
            max_work: 5,
            ..Default::default()
        },
        &mut budget,
        &mut Counters::default(),
        location(),
    );
    assert!(matches!(
        failure,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            observed: 6,
            limit: 5,
            ..
        })
    ));
    assert!(join.evaluation.values.is_empty());
    // This exercises a fresh filter call, not resumption of a stopped Join.
    assert!(
        join.filters(
            &[Value::Number(9)],
            Comparisons::Deferred,
            &FormulaLimits::default(),
            &mut budget,
            &mut Counters::default(),
            location(),
        )
        .unwrap()
    );
    assert!(join.evaluation.values.is_empty());
}
