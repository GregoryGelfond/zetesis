//! Observe live scoped scratch and its cleanup without exposing a test API.
#[path = "tests/callers.rs"]
mod callers;
use super::super::{Evaluation, Expression, Operation};
use super::RETAINED_CELLS;
use crate::formula_support::testing::{Fixture, binding};
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource};
use std::panic::{AssertUnwindSafe, catch_unwind};
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Location, Span},
};
use themelios_program::term::{BinaryOp, EvalError};
use zetesis_core::catalog::TermAssignment;
use zetesis_core::{Sign, Value, ValueLimits, ValueNode, ValueNodeRef};

fn location() -> Location {
    Location {
        source: SourceId::new(17),
        span: Span::empty(ByteOffset::new(23)),
    }
}
fn evaluate(
    evaluation: &mut Evaluation,
    nodes: Vec<Operation>,
    assignment: &[Value],
    fixture: &mut Fixture,
) -> Result<Value, FormulaFailure> {
    fixture.with(location(), |_, computation, counters| {
        let source: Vec<_> = assignment.iter().cloned().map(Some).collect();
        let source = binding(&source, computation, counters, location());
        let key = evaluation.expression(
            &Expression { nodes },
            |slot| source.key(slot, location()),
            computation,
            &FormulaLimits::default(),
            counters,
            location(),
        )?;
        // Export is solely the test assertion boundary, after DAG execution.
        Ok(computation
            .read()
            .term(&key)
            .unwrap()
            .to_value(ValueLimits::default())
            .unwrap())
    })
}
fn empty(evaluation: &Evaluation) -> bool {
    evaluation
        .scratch
        .terms
        .as_ref()
        .is_none_or(TermAssignment::is_empty)
        && evaluation.scratch.integers.is_empty()
        && evaluation.scratch.missing.is_empty()
}
fn term_capacity(evaluation: &Evaluation) -> usize {
    evaluation
        .scratch
        .terms
        .as_ref()
        .map_or(0, TermAssignment::capacity)
}
#[test]
fn reuse_starts_from_the_current_binding() {
    let mut fixture = Fixture::default();
    let mut evaluation = Evaluation::default();
    for left in -4..4 {
        for right in -4..4 {
            let actual = evaluate(
                &mut evaluation,
                vec![
                    Operation::Variable(0),
                    Operation::Constant(fixture.scalar(&Value::Number(2), location())),
                    Operation::Binary(BinaryOp::Mul, 0, 1),
                    Operation::Variable(1),
                    Operation::Binary(BinaryOp::Sub, 2, 3),
                ],
                &[Value::Number(left), Value::Number(right)],
                &mut fixture,
            )
            .unwrap();
            assert_eq!(actual, Value::Number(2 * left - right));
        }
    }
}
#[test]
fn leaf_evaluation_needs_no_term_cells() {
    let mut fixture = Fixture::default();
    for value in [Value::Number(7), Value::String("result".into())] {
        for operation in [
            Operation::Constant(fixture.scalar(&value, location())),
            Operation::Variable(0),
        ] {
            let mut evaluation = Evaluation::default();
            assert_eq!(
                evaluate(
                    &mut evaluation,
                    vec![operation],
                    std::slice::from_ref(&value),
                    &mut fixture
                )
                .unwrap(),
                value
            );
            assert_eq!(term_capacity(&evaluation), 0);
        }
    }
}
#[test]
fn successful_evaluation_retains_only_empty_cells() {
    let mut fixture = Fixture::default();
    let mut evaluation = Evaluation::default();
    let result = evaluate(
        &mut evaluation,
        vec![
            Operation::Constant(fixture.scalar(&Value::Symbol("earlier".into()), location())),
            Operation::Constant(fixture.scalar(&Value::String("result".into()), location())),
        ],
        &[],
        &mut fixture,
    )
    .unwrap();
    assert_eq!(result, Value::String("result".into()));
    assert!(empty(&evaluation));
    assert!((1..=RETAINED_CELLS).contains(&term_capacity(&evaluation)));
}
#[test]
fn failed_evaluation_clears_its_live_prefix() {
    let mut fixture = Fixture::default();
    let mut evaluation = Evaluation::default();
    let result = evaluate(
        &mut evaluation,
        vec![
            Operation::Constant(fixture.scalar(&Value::Symbol("earlier".into()), location())),
            Operation::Constant(fixture.scalar(&Value::Number(0), location())),
            Operation::Binary(BinaryOp::Div, 1, 1),
        ],
        &[],
        &mut fixture,
    );
    assert!(matches!(
        result,
        Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation {
            error: EvalError::Undefined,
            ..
        }))
    ));
    assert!(empty(&evaluation));
}
#[test]
fn unwinding_clears_the_live_prefix() {
    let mut fixture = Fixture::default();
    let mut evaluation = Evaluation::default();
    let earlier = fixture.scalar(&Value::String("earlier".into()), location());
    fixture.with(location(), |_, computation, counters| {
        let result = catch_unwind(AssertUnwindSafe(|| {
            evaluation.expression(
                &Expression {
                    nodes: vec![Operation::Constant(earlier), Operation::Variable(0)],
                },
                |_| panic!("injected variable lookup unwind"),
                computation,
                &FormulaLimits::default(),
                counters,
                location(),
            )
        }));
        assert!(result.is_err());
        assert!(empty(&evaluation));
    });
}
#[test]
fn large_evaluations_release_term_capacity() {
    let mut fixture = Fixture::default();
    let mut evaluation = Evaluation::default();
    evaluate(
        &mut evaluation,
        (0..RETAINED_CELLS + 2)
            .map(|_| {
                Operation::Constant(fixture.scalar(&Value::Symbol("seven".into()), location()))
            })
            .collect(),
        &[],
        &mut fixture,
    )
    .unwrap();
    assert!(empty(&evaluation));
    assert_eq!(term_capacity(&evaluation), 0);
}
#[test]
fn root_does_not_extend_the_retained_prefix() {
    let mut fixture = Fixture::default();
    let mut evaluation = Evaluation::default();
    evaluate(
        &mut evaluation,
        (0..=RETAINED_CELLS)
            .map(|_| {
                Operation::Constant(fixture.scalar(&Value::Symbol("seven".into()), location()))
            })
            .collect(),
        &[],
        &mut fixture,
    )
    .unwrap();
    assert!(empty(&evaluation));
    assert_eq!(term_capacity(&evaluation), RETAINED_CELLS);
}
#[test]
fn numeric_plans_hold_no_term_cells() {
    let mut fixture = Fixture::default();
    let mut evaluation = Evaluation::default();
    let mut nodes: Vec<_> = (0..=RETAINED_CELLS)
        .map(|index| {
            Operation::Constant(
                fixture.scalar(&Value::Number(i32::try_from(index).unwrap()), location()),
            )
        })
        .collect();
    nodes.push(Operation::Binary(BinaryOp::Add, 3, RETAINED_CELLS));
    assert_eq!(
        evaluate(&mut evaluation, nodes, &[], &mut fixture).unwrap(),
        Value::Number(3 + i32::try_from(RETAINED_CELLS).unwrap())
    );
    assert_eq!(term_capacity(&evaluation), 0);
    assert_eq!(evaluation.scratch.integers.capacity(), 0);
}
#[test]
fn a_nonnumeric_value_promotes_the_integer_prefix() {
    let mut fixture = Fixture::default();
    let mut evaluation = Evaluation::default();
    assert_eq!(
        evaluate(
            &mut evaluation,
            vec![
                Operation::Variable(0),
                Operation::Constant(fixture.scalar(&Value::Number(2), location())),
                Operation::Variable(1),
                Operation::Binary(BinaryOp::Add, 0, 1)
            ],
            &[Value::Number(5), Value::String("marker".into())],
            &mut fixture
        )
        .unwrap(),
        Value::Number(7)
    );
    assert!(empty(&evaluation));
    assert!((1..=RETAINED_CELLS).contains(&term_capacity(&evaluation)));
}
#[test]
fn undefined_numeric_arithmetic_clears_integer_cells() {
    let mut fixture = Fixture::default();
    let mut evaluation = Evaluation::default();
    assert!(
        evaluate(
            &mut evaluation,
            vec![
                Operation::Constant(fixture.scalar(&Value::Number(1), location())),
                Operation::Constant(fixture.scalar(&Value::Number(0), location())),
                Operation::Binary(BinaryOp::Div, 0, 1)
            ],
            &[],
            &mut fixture
        )
        .is_err()
    );
    assert!(empty(&evaluation));
    assert_eq!(term_capacity(&evaluation), 0);
}
#[test]
fn repeated_reads_reuse_the_same_canonical_payload() {
    let mut fixture = Fixture::default();
    fixture.with(location(), |_, computation, counters| {
        let source = binding(
            &[Some(Value::String("shared text".into()))],
            computation,
            counters,
            location(),
        );
        let mut evaluation = Evaluation::default();
        for _ in 0..3 {
            let key = evaluation
                .expression(
                    &Expression {
                        nodes: vec![Operation::Variable(0)],
                    },
                    |slot| source.key(slot, location()),
                    computation,
                    &FormulaLimits::default(),
                    counters,
                    location(),
                )
                .unwrap();
            let result = computation.read().term(&key).unwrap();
            let expected = source.read(0, computation.read(), location()).unwrap();
            let (
                zetesis_core::ValueNodeRef::String(left),
                zetesis_core::ValueNodeRef::String(right),
            ) = (result.descriptor(), expected.descriptor())
            else {
                panic!("string leaves")
            };
            assert!(std::ptr::eq(left.as_ptr(), right.as_ptr()));
        }
    });
}
#[test]
fn work_refusal_precedes_variable_access() {
    let mut fixture = Fixture::default();
    fixture.with(location(), |_, computation, counters| {
        let mut evaluation = Evaluation::default();
        let bounded = FormulaLimits {
            max_work: counters.accounting.work + 1,
            ..Default::default()
        };
        let failure = evaluation.expression(
            &Expression {
                nodes: vec![Operation::Variable(0)],
            },
            |_| panic!("work must refuse before variable access"),
            computation,
            &bounded,
            counters,
            location(),
        );
        assert!(matches!(
            failure,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
        assert!(empty(&evaluation));
    });
}
fn retry_expression(fixture: &mut Fixture) -> Expression {
    Expression {
        nodes: vec![
            Operation::Constant(fixture.scalar(&Value::String("prefix".into()), location())),
            Operation::Constant(fixture.scalar(&Value::Number(7), location())),
            Operation::Constructor(Box::new(crate::formula_value::Constructor {
                shape: fixture.constructor(
                    ValueNodeRef::Function {
                        name: "f",
                        sign: Sign::Negative,
                        arity: 3,
                    },
                    location(),
                ),
                arguments: vec![1, 0, 1],
            })),
        ],
    }
}
#[test]
fn every_work_stop_leaves_scratch_ready_for_retry() {
    let mut fixture = Fixture::default();
    let expression = retry_expression(&mut fixture);
    let needed = fixture.with(location(), |_, computation, counters| {
        let before = counters.accounting.work;
        Evaluation::default()
            .expression(
                &expression,
                |_| unreachable!(),
                computation,
                &FormulaLimits::default(),
                counters,
                location(),
            )
            .unwrap();
        counters.accounting.work - before
    });
    for cutoff in 0..needed {
        let mut fixture = Fixture::default();
        let expression = retry_expression(&mut fixture);
        fixture.with(location(), |_, computation, counters| {
            let mut evaluation = Evaluation::default();
            let bounded = FormulaLimits {
                max_work: counters.accounting.work + cutoff,
                ..Default::default()
            };
            assert!(
                evaluation
                    .expression(
                        &expression,
                        |_| unreachable!(),
                        computation,
                        &bounded,
                        counters,
                        location()
                    )
                    .is_err()
            );
            assert!(empty(&evaluation));
            let key = evaluation
                .expression(
                    &expression,
                    |_| unreachable!(),
                    computation,
                    &FormulaLimits::default(),
                    counters,
                    location(),
                )
                .unwrap();
            assert_eq!(
                computation
                    .read()
                    .term(&key)
                    .unwrap()
                    .child(0)
                    .unwrap()
                    .descriptor(),
                zetesis_core::ValueNodeRef::Number(7)
            );
        });
    }
}
#[test]
fn constructor_root_reads_its_ordered_prefix() {
    let mut fixture = Fixture::default();
    let mut evaluation = Evaluation::default();
    let actual = evaluate(
        &mut evaluation,
        vec![
            Operation::Constant(fixture.scalar(&Value::Number(7), location())),
            Operation::Variable(0),
            Operation::Constructor(Box::new(crate::formula_value::Constructor {
                shape: fixture.constructor(
                    ValueNodeRef::Function {
                        name: "f",
                        sign: Sign::Negative,
                        arity: 3,
                    },
                    location(),
                ),
                arguments: vec![1, 0, 1],
            })),
        ],
        &[Value::String("argument".into())],
        &mut fixture,
    )
    .unwrap();
    let expected = Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Negative,
                arity: 3,
            },
            ValueNode::String("argument".into()),
            ValueNode::Number(7),
            ValueNode::String("argument".into()),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn constant_reads_reuse_admitted_payload() {
    let mut fixture = Fixture::default();
    let scalar = fixture.scalar(&Value::String("shared constant".into()), location());
    fixture.with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let expected = computation
            .static_scalar(scalar, &limits, counters, location())
            .unwrap();
        let ValueNodeRef::String(expected) = expected.descriptor() else {
            unreachable!()
        };
        let mut evaluation = Evaluation::default();
        for _ in 0..3 {
            let key = evaluation
                .expression(
                    &Expression {
                        nodes: vec![Operation::Constant(scalar)],
                    },
                    |_| unreachable!(),
                    computation,
                    &limits,
                    counters,
                    location(),
                )
                .unwrap();
            let value = computation.read().term(&key).unwrap();
            let ValueNodeRef::String(actual) = value.descriptor() else {
                unreachable!()
            };
            assert!(std::ptr::eq(expected.as_ptr(), actual.as_ptr()));
        }
    });
}
