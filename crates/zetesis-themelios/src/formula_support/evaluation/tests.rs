//! Storage lifetime is tested independently of scalar-operation correctness.

mod callers;

use std::panic::{AssertUnwindSafe, catch_unwind};

use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use themelios_program::term::{BinaryOp, EvalError};
use zetesis_core::{Sign, Value, ValueLimits, ValueNode};

use super::{Budget, Counters, Evaluation, Expression, Operation, RETAINED_VALUE_CELLS};
use crate::{ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure, FormulaLimits};

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
) -> Result<Value, FormulaFailure> {
    evaluation.expression(
        &Expression { nodes },
        |variable| Ok(&assignment[variable]),
        &FormulaLimits::default(),
        &mut Budget::new(ExpansionLimits::default(), usize::MAX),
        &mut Counters::default(),
        location(),
    )
}

#[test]
fn reuse_starts_from_the_current_binding() {
    let mut evaluation = Evaluation::default();
    for left in -4..4 {
        for right in -4..4 {
            let result = evaluate(
                &mut evaluation,
                vec![
                    Operation::Variable(0),
                    Operation::Constant(Value::Number(2)),
                    Operation::Binary(BinaryOp::Mul, 0, 1),
                    Operation::Variable(1),
                    Operation::Binary(BinaryOp::Sub, 2, 3),
                ],
                &[Value::Number(left), Value::Number(right)],
            )
            .unwrap();
            assert_eq!(result, Value::Number(2 * left - right));
        }
    }
}

#[test]
fn leaf_evaluation_needs_no_scratch_cells() {
    for value in [Value::Number(7), Value::String("result".into())] {
        for operation in [Operation::Constant(value.clone()), Operation::Variable(0)] {
            let mut evaluation = Evaluation::default();
            assert_eq!(
                evaluate(
                    &mut evaluation,
                    vec![operation],
                    std::slice::from_ref(&value)
                )
                .unwrap(),
                value
            );
            assert_eq!(evaluation.values.capacity(), 0);
        }
    }
}

#[test]
fn successful_evaluation_retains_only_empty_cells() {
    let mut evaluation = Evaluation::default();
    let result = evaluate(
        &mut evaluation,
        vec![
            Operation::Constant(Value::Symbol("earlier".into())),
            Operation::Constant(Value::String("result".into())),
        ],
        &[],
    )
    .unwrap();
    assert_eq!(result, Value::String("result".into()));
    assert!(evaluation.values.is_empty());
    assert!((1..=RETAINED_VALUE_CELLS).contains(&evaluation.values.capacity()));
}

#[test]
fn failed_evaluation_clears_its_live_prefix() {
    let mut evaluation = Evaluation::default();
    let result = evaluate(
        &mut evaluation,
        vec![
            Operation::Constant(Value::Symbol("earlier".into())),
            Operation::Constant(Value::Number(0)),
            Operation::Binary(BinaryOp::Div, 1, 1),
        ],
        &[],
    );
    assert!(matches!(result, Err(FormulaFailure::Expansion(
        ExpansionFailure::Evaluation { error: EvalError::Undefined, location: found }
    )) if found == location()));
    assert!(evaluation.values.is_empty());
}

#[test]
fn unwinding_clears_the_live_prefix() {
    let mut evaluation = Evaluation::default();
    let result = catch_unwind(AssertUnwindSafe(|| {
        evaluation.expression(
            &Expression {
                nodes: vec![
                    Operation::Constant(Value::String("earlier".into())),
                    Operation::Variable(0),
                ],
            },
            |_| panic!("injected variable lookup unwind"),
            &FormulaLimits::default(),
            &mut Budget::new(ExpansionLimits::default(), usize::MAX),
            &mut Counters::default(),
            location(),
        )
    }));
    assert!(result.is_err());
    assert!(evaluation.values.is_empty());
}

#[test]
fn large_evaluations_release_their_workspace() {
    let mut evaluation = Evaluation::default();
    let result = evaluate(
        &mut evaluation,
        (0..RETAINED_VALUE_CELLS + 2)
            .map(|_| Operation::Constant(Value::Number(7)))
            .collect(),
        &[],
    )
    .unwrap();
    assert_eq!(result, Value::Number(7));
    assert!(evaluation.values.is_empty());
    assert_eq!(evaluation.values.capacity(), 0);
}

#[test]
fn root_does_not_extend_the_retained_prefix() {
    let mut evaluation = Evaluation::default();
    let result = evaluate(
        &mut evaluation,
        (0..=RETAINED_VALUE_CELLS)
            .map(|_| Operation::Constant(Value::Number(7)))
            .collect(),
        &[],
    )
    .unwrap();
    assert_eq!(result, Value::Number(7));
    assert!(evaluation.values.is_empty());
    assert_eq!(evaluation.values.capacity(), RETAINED_VALUE_CELLS);
}

#[test]
fn stopped_evaluations_release_large_storage() {
    let mut evaluation = Evaluation::default();
    let failure = evaluation.expression(
        &Expression {
            nodes: (0..RETAINED_VALUE_CELLS + 2)
                .map(|_| Operation::Constant(Value::Number(7)))
                .collect(),
        },
        |_| unreachable!("the plan has no variables"),
        &FormulaLimits {
            max_work: u64::try_from(RETAINED_VALUE_CELLS + 1).unwrap(),
            ..Default::default()
        },
        &mut Budget::new(ExpansionLimits::default(), usize::MAX),
        &mut Counters::default(),
        location(),
    );
    assert!(matches!(failure, Err(FormulaFailure::Limit {
        resource: crate::FormulaResource::Work, observed, limit, location: found
    }) if observed == (RETAINED_VALUE_CELLS + 2) as u128
        && limit == (RETAINED_VALUE_CELLS + 1) as u128 && found == location()));
    assert!(evaluation.values.is_empty());
    assert_eq!(evaluation.values.capacity(), 0);
}

#[test]
fn reuse_charges_each_operand_copy() {
    let mut evaluation = Evaluation::default();
    let expression = Expression {
        nodes: vec![Operation::Variable(0)],
    };
    let value = Value::String("abc".into());
    let mut budget = Budget::new(
        ExpansionLimits {
            max_scalar_bytes: 6,
            ..Default::default()
        },
        usize::MAX,
    );
    let mut counters = Counters::default();
    for _ in 0..2 {
        assert_eq!(
            evaluation
                .expression(
                    &expression,
                    |_| Ok(&value),
                    &FormulaLimits::default(),
                    &mut budget,
                    &mut counters,
                    location(),
                )
                .unwrap(),
            value
        );
    }
    let failure = evaluation.expression(
        &expression,
        |_| Ok(&value),
        &FormulaLimits::default(),
        &mut budget,
        &mut counters,
        location(),
    );
    assert!(matches!(failure, Err(FormulaFailure::Expansion(
        ExpansionFailure::Limit {
            resource: ExpansionResource::ScalarBytes, observed: 9, limit: 6, location: found
        }
    )) if found == location()));
    assert_eq!(counters.work, 3);
}

#[test]
fn root_work_refusal_precedes_operand_access() {
    let mut evaluation = Evaluation::default();
    let expression = Expression {
        nodes: vec![
            Operation::Constant(Value::Number(7)),
            Operation::Variable(0),
        ],
    };
    let mut counters = Counters::default();
    let failure = evaluation.expression(
        &expression,
        |_| panic!("the work refusal must precede root operand access"),
        &FormulaLimits {
            max_work: 1,
            ..Default::default()
        },
        &mut Budget::new(ExpansionLimits::default(), usize::MAX),
        &mut counters,
        location(),
    );
    assert!(matches!(failure, Err(FormulaFailure::Limit {
        resource: crate::FormulaResource::Work, observed: 2, limit: 1, location: found
    }) if found == location()));
    assert_eq!(counters.work, 1);
    assert!(evaluation.values.is_empty());
}

#[test]
fn root_copy_refusal_clears_the_live_prefix() {
    let mut evaluation = Evaluation::default();
    let expression = Expression {
        nodes: vec![
            Operation::Constant(Value::String("prefix".into())),
            Operation::Constant(Value::String("root".into())),
        ],
    };
    let failure = evaluation.expression(
        &expression,
        |_| unreachable!("the plan has no variables"),
        &FormulaLimits::default(),
        &mut Budget::new(
            ExpansionLimits {
                max_scalar_bytes: 6,
                ..Default::default()
            },
            usize::MAX,
        ),
        &mut Counters::default(),
        location(),
    );
    assert!(matches!(failure, Err(FormulaFailure::Expansion(
        ExpansionFailure::Limit {
            resource: ExpansionResource::ScalarBytes, observed: 10, limit: 6, location: found
        }
    )) if found == location()));
    assert!(evaluation.values.is_empty());
}

#[test]
fn constructor_root_reads_its_ordered_prefix() {
    let mut evaluation = Evaluation::default();
    let actual = evaluate(
        &mut evaluation,
        vec![
            Operation::Constant(Value::Number(7)),
            Operation::Variable(0),
            Operation::Constructor(Box::new(crate::formula_value::Constructor {
                name: Some("f".into()),
                sign: Sign::Negative,
                arguments: vec![1, 0, 1],
            })),
        ],
        &[Value::String("argument".into())],
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
