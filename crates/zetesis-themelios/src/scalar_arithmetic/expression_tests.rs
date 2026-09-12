//! Expression-level order and resource contracts around the pure operators.
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use themelios_program::term::{BinaryOp, EvalError, UnaryOp};
use zetesis_core::Value;

use crate::expansion::Budget;
use crate::formula_ir::{Expression, Operation};
use crate::formula_support::{Counters, expression};
use crate::{
    ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure, FormulaLimits,
    FormulaResource,
};

fn location() -> Location {
    Location {
        source: SourceId::new(17),
        span: Span::empty(ByteOffset::new(23)),
    }
}
fn evaluate(
    nodes: Vec<Operation>,
    work: u64,
    expansion: ExpansionLimits,
) -> (Result<Value, FormulaFailure>, u64) {
    let mut budget = Budget::new(expansion, usize::MAX);
    let mut counters = Counters::default();
    let result = expression(
        &Expression { nodes },
        &crate::formula_binding::Binding::default(),
        &FormulaLimits {
            max_work: work,
            ..Default::default()
        },
        &mut budget,
        &mut counters,
        location(),
    );
    (result, counters.work)
}
fn simple_plan() -> Vec<Operation> {
    vec![
        Operation::Constant(Value::Number(1)),
        Operation::Constant(Value::Number(2)),
        Operation::Binary(BinaryOp::Add, 0, 1),
        Operation::Unary(UnaryOp::Negate, 2),
        Operation::Absolute(3),
    ]
}

#[test]
fn scalar_nodes_keep_their_exact_work_charge() {
    let (result, work) = evaluate(
        simple_plan(),
        5,
        ExpansionLimits {
            max_scalar_bytes: 0,
            ..Default::default()
        },
    );
    assert_eq!(result.unwrap(), Value::Number(3));
    assert_eq!(work, 5);
    let (failure, work) = evaluate(simple_plan(), 4, ExpansionLimits::default());
    assert_eq!(work, 4);
    assert!(
        matches!(failure, Err(FormulaFailure::Limit { resource: FormulaResource::Work, observed: 5, limit: 4, location: found }) if found == location())
    );
}

#[test]
fn the_first_failing_node_remains_authoritative() {
    for (first, second, expected) in [
        ((0, 1), (1, 2), EvalError::Overflow),
        ((1, 2), (0, 1), EvalError::Undefined),
    ] {
        let nodes = vec![
            Operation::Constant(Value::Number(i32::MIN)),
            Operation::Constant(Value::Number(-1)),
            Operation::Constant(Value::Number(0)),
            Operation::Binary(BinaryOp::Div, first.0, first.1),
            Operation::Binary(BinaryOp::Div, second.0, second.1),
            Operation::Binary(BinaryOp::Add, 3, 4),
        ];
        let (result, work) = evaluate(nodes, u64::MAX, ExpansionLimits::default());
        assert_eq!(work, 4);
        assert!(
            matches!(result, Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, location: found })) if error == expected && found == location())
        );
    }
}

#[test]
fn nonnumeric_operands_remain_undefined() {
    for operand in [
        Value::Symbol("a".into()),
        Value::String("1".into()),
        Value::Infimum,
        Value::Supremum,
    ] {
        for operation in [
            Operation::Unary(UnaryOp::Negate, 0),
            Operation::Absolute(0),
            Operation::Binary(BinaryOp::Add, 0, 0),
        ] {
            let (result, work) = evaluate(
                vec![Operation::Constant(operand.clone()), operation],
                u64::MAX,
                ExpansionLimits::default(),
            );
            assert_eq!(work, 2);
            assert!(
                matches!(result, Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation { error: EvalError::Undefined, location: found })) if found == location())
            );
        }
    }
}

#[test]
fn operand_payload_refusal_precedes_arithmetic() {
    let (result, work) = evaluate(
        vec![
            Operation::Constant(Value::Symbol("abc".into())),
            Operation::Unary(UnaryOp::Negate, 0),
        ],
        u64::MAX,
        ExpansionLimits {
            max_scalar_bytes: 2,
            ..Default::default()
        },
    );
    assert_eq!(work, 1);
    assert!(
        matches!(result, Err(FormulaFailure::Expansion(ExpansionFailure::Limit { resource: ExpansionResource::ScalarBytes, observed: 3, limit: 2, location: found })) if found == location())
    );
}

#[test]
fn node_work_refusal_precedes_operand_copy() {
    let (result, work) = evaluate(
        vec![
            Operation::Constant(Value::Symbol("abc".into())),
            Operation::Unary(UnaryOp::Negate, 0),
        ],
        0,
        ExpansionLimits {
            max_scalar_bytes: 0,
            ..Default::default()
        },
    );
    assert_eq!(work, 0);
    assert!(
        matches!(result, Err(FormulaFailure::Limit { resource: FormulaResource::Work, observed: 1, limit: 0, location: found }) if found == location())
    );
}
