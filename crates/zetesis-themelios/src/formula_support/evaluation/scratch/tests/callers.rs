//! Actual join consumers share one canonical computation and leased scratch.
use super::{empty, location};
use crate::expansion::Budget;
use crate::formula_ir::{Expression, LiteralIr, Operation};
use crate::formula_support::{
    Comparisons, Join,
    testing::{Fixture, binding},
};
use crate::{ExpansionFailure, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource};
use themelios_program::{program::Relation, term::BinaryOp};
use zetesis_core::{Value, ValueNodeRef};

use crate::formula_support::Context;
fn number(fixture: &mut Fixture, value: i32) -> Expression {
    Expression {
        nodes: vec![Operation::Constant(
            fixture.scalar(&Value::Number(value), location()),
        )],
    }
}
fn increment(fixture: &mut Fixture, variable: usize) -> Expression {
    Expression {
        nodes: vec![
            Operation::Variable(variable),
            Operation::Constant(fixture.scalar(&Value::Number(1), location())),
            Operation::Binary(BinaryOp::Add, 0, 1),
        ],
    }
}
fn budget() -> Budget {
    Budget::new(ExpansionLimits::default(), usize::MAX)
}

#[test]
fn final_filters_use_the_join_workspace() {
    let mut fixture = Fixture::default();
    let literals = [LiteralIr::TupleCompare(
        vec![increment(&mut fixture, 0)],
        Relation::Eq,
        vec![increment(&mut fixture, 0)],
    )];
    fixture.with(location(), |support, computation, counters| {
        let prefix = binding(&[Some(Value::Number(7))], computation, counters, location());
        let limits = FormulaLimits::default();
        let mut budget = budget();
        let mut join = Join::new(
            &literals,
            &prefix,
            1,
            support,
            &mut budget,
            Context::new(computation, &limits, counters, location()),
        )
        .unwrap();
        let result = join
            .next(computation, &limits, &mut budget, counters, location())
            .unwrap()
            .unwrap();
        assert_eq!(
            result
                .read(0, computation.read(), location())
                .unwrap()
                .descriptor(),
            ValueNodeRef::Number(7)
        );
        assert!(empty(&join.evaluation));
        assert!(join.evaluation.scratch.integers.capacity() >= 2);
    });
}
#[test]
fn binding_generators_use_the_join_workspace() {
    let mut fixture = Fixture::default();
    let literals = [
        LiteralIr::Range {
            target: 0,
            lower: number(&mut fixture, 0),
            upper: number(&mut fixture, 2),
            binder: true,
        },
        LiteralIr::Bind {
            target: 1,
            value: increment(&mut fixture, 0),
        },
    ];
    fixture.with(location(), |support, computation, counters| {
        let prefix = binding(&[], computation, counters, location());
        let limits = FormulaLimits::default();
        let mut budget = budget();
        let mut join = Join::new(
            &literals,
            &prefix,
            2,
            support,
            &mut budget,
            Context::new(computation, &limits, counters, location()),
        )
        .unwrap();
        for value in 0..=2 {
            let row = join
                .next(computation, &limits, &mut budget, counters, location())
                .unwrap()
                .unwrap();
            assert_eq!(
                row.read(0, computation.read(), location())
                    .unwrap()
                    .descriptor(),
                ValueNodeRef::Number(value)
            );
            assert_eq!(
                row.read(1, computation.read(), location())
                    .unwrap()
                    .descriptor(),
                ValueNodeRef::Number(value + 1)
            );
            assert!(empty(&join.evaluation));
            assert!(join.evaluation.scratch.integers.capacity() >= 2);
        }
        assert!(
            join.next(computation, &limits, &mut budget, counters, location())
                .unwrap()
                .is_none()
        );
    });
}
#[test]
fn range_endpoints_use_the_join_workspace() {
    let mut fixture = Fixture::default();
    let literals = [LiteralIr::Range {
        target: 1,
        lower: increment(&mut fixture, 0),
        upper: increment(&mut fixture, 0),
        binder: true,
    }];
    fixture.with(location(), |support, computation, counters| {
        let prefix = binding(&[Some(Value::Number(2))], computation, counters, location());
        let limits = FormulaLimits::default();
        let mut budget = budget();
        let mut join = Join::new(
            &literals,
            &prefix,
            2,
            support,
            &mut budget,
            Context::new(computation, &limits, counters, location()),
        )
        .unwrap();
        let row = join
            .next(computation, &limits, &mut budget, counters, location())
            .unwrap()
            .unwrap();
        assert_eq!(
            row.read(1, computation.read(), location())
                .unwrap()
                .descriptor(),
            ValueNodeRef::Number(3)
        );
        assert!(empty(&join.evaluation));
        assert!(join.evaluation.scratch.integers.capacity() >= 2);
    });
}
#[test]
fn false_filters_do_not_hide_later_arithmetic_errors() {
    let mut fixture = Fixture::default();
    let literals = [
        LiteralIr::TupleCompare(
            vec![number(&mut fixture, 0)],
            Relation::Eq,
            vec![number(&mut fixture, 1)],
        ),
        LiteralIr::TupleCompare(
            vec![Expression {
                nodes: vec![
                    Operation::Constant(fixture.scalar(&Value::Number(1), location())),
                    Operation::Constant(fixture.scalar(&Value::Number(0), location())),
                    Operation::Binary(BinaryOp::Div, 0, 1),
                ],
            }],
            Relation::Eq,
            vec![number(&mut fixture, 0)],
        ),
    ];
    fixture.with(location(), |support, computation, counters| {
        let prefix = binding(&[], computation, counters, location());
        let limits = FormulaLimits::default();
        let mut budget = budget();
        let mut join = Join::new(
            &literals,
            &prefix,
            0,
            support,
            &mut budget,
            Context::new(computation, &limits, counters, location()),
        )
        .unwrap();
        assert!(
            join.next(computation, &limits, &mut budget, counters, location())
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            join.take_family().finish(),
            Err(FormulaFailure::Expansion(
                ExpansionFailure::Evaluation { .. }
            ))
        ));
    });
}
#[test]
fn stopped_filters_release_live_workspace_values() {
    let mut fixture = Fixture::default();
    let literals = [LiteralIr::Compare(
        increment(&mut fixture, 0),
        Relation::Eq,
        increment(&mut fixture, 0),
    )];
    fixture.with(location(), |support, computation, counters| {
        let prefix = binding(&[], computation, counters, location());
        let limits = FormulaLimits::default();
        let mut budget = budget();
        let mut join = Join::new(
            &literals,
            &prefix,
            1,
            support,
            &mut budget,
            Context::new(computation, &limits, counters, location()),
        )
        .unwrap();
        let frame = crate::formula_support::rows::Frame::Owned(binding(
            &[Some(Value::Number(3))],
            computation,
            counters,
            location(),
        ));
        let bounded = FormulaLimits {
            max_work: counters.accounting.work + 5,
            ..limits
        };
        assert!(matches!(
            join.filters(
                &frame,
                Comparisons::Deferred,
                computation,
                &bounded,
                counters,
                location()
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
        assert!(empty(&join.evaluation));
        assert!(matches!(
            join.filters(
                &frame,
                Comparisons::Deferred,
                computation,
                &limits,
                counters,
                location()
            )
            .unwrap(),
            crate::formula_support::filters::Selection::Defined(true)
        ));
        assert!(empty(&join.evaluation));
    });
}
#[test]
fn generator_reads_refuse_absent_inputs() {
    let mut fixture = Fixture::default();
    let literals = [
        LiteralIr::Bind {
            target: 0,
            value: increment(&mut fixture, 1),
        },
        LiteralIr::Bind {
            target: 1,
            value: number(&mut fixture, 8),
        },
    ];
    fixture.with(location(), |support, computation, counters| {
        let prefix = binding(&[], computation, counters, location());
        let limits = FormulaLimits::default();
        let mut budget = budget();
        let mut join = Join::new(
            &literals,
            &prefix,
            2,
            support,
            &mut budget,
            Context::new(computation, &limits, counters, location()),
        )
        .unwrap();
        assert!(matches!(
            join.next(computation, &limits, &mut budget, counters, location()),
            Err(FormulaFailure::UnsafeVariable { variable: 1, .. })
        ));
    });
}
#[test]
fn component_rows_preserve_excluded_slots() {
    let mut fixture = Fixture::default();
    let literals = [LiteralIr::Compare(
        number(&mut fixture, 1),
        Relation::Eq,
        number(&mut fixture, 1),
    )];
    fixture.with(location(), |support, computation, counters| {
        let fixed = binding(
            &[Some(Value::Number(0)), None],
            computation,
            counters,
            location(),
        );
        let limits = FormulaLimits::default();
        let mut budget = budget();
        let mut join = Join::component(
            &literals,
            2,
            &[0],
            &fixed,
            support,
            &mut budget,
            Context::new(computation, &limits, counters, location()),
        )
        .unwrap();
        let row = join
            .next(computation, &limits, &mut budget, counters, location())
            .unwrap()
            .unwrap();
        assert_eq!(
            row.read(0, computation.read(), location())
                .unwrap()
                .descriptor(),
            ValueNodeRef::Number(0)
        );
        assert!(!row.is_bound(1, location()).unwrap());
    });
}
