//! Differential contracts against the unchanged generic operation evaluator.

use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};

use proptest::prelude::*;
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use themelios_program::term::{BinaryOp, EvalError, UnaryOp};
use zetesis_core::{Sign, Value};

use super::{
    Budget, Counters, Evaluation, Execution, Expression, Frame, NUMERIC_PREFIX_CELLS, Operation,
};
use crate::formula_value::Constructor;
use crate::grounding_observer::Event;
use crate::{
    ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure, FormulaLimits,
    FormulaResource,
};

const OPERATORS: [BinaryOp; 9] = [
    BinaryOp::Add,
    BinaryOp::Sub,
    BinaryOp::Mul,
    BinaryOp::Div,
    BinaryOp::Mod,
    BinaryOp::Pow,
    BinaryOp::BitAnd,
    BinaryOp::BitOr,
    BinaryOp::BitXor,
];

fn location() -> Location {
    Location {
        source: SourceId::new(31),
        span: Span::empty(ByteOffset::new(47)),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Value(Value),
    Arithmetic(EvalError, Location),
    Work(FormulaResource, u128, u128, Location),
    Expansion(ExpansionResource, u128, u128, Location),
}

fn outcome(result: Result<Value, FormulaFailure>) -> Outcome {
    match result {
        Ok(value) => Outcome::Value(value),
        Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, location })) => {
            Outcome::Arithmetic(error, location)
        }
        Err(FormulaFailure::Limit {
            resource,
            observed,
            limit,
            location,
        }) => Outcome::Work(resource, observed, limit, location),
        Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource,
            observed,
            limit,
            location,
        })) => Outcome::Expansion(resource, observed, limit, location),
        Err(error) => panic!("unexpected fixture failure: {error:?}"),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Observation {
    outcome: Outcome,
    work: u64,
    lookups: Vec<usize>,
    copied_payload: u128,
}

#[derive(Clone, Copy)]
enum Route {
    Numeric,
    Generic,
}

fn observe(
    expression: &Expression,
    assignment: &[Value],
    maximum_work: u64,
    maximum_payload: usize,
    route: Route,
) -> Observation {
    let limits = FormulaLimits {
        max_work: maximum_work,
        ..FormulaLimits::default()
    };
    let mut budget = Budget::new(
        ExpansionLimits {
            max_scalar_bytes: maximum_payload,
            ..ExpansionLimits::default()
        },
        usize::MAX,
    );
    let mut counters = Counters::default();
    let lookups = RefCell::new(Vec::new());
    let variable = |index| {
        lookups.borrow_mut().push(index);
        &assignment[index]
    };
    let mut evaluation = Evaluation::default();
    let result = match route {
        Route::Numeric => evaluation.expression(
            expression,
            variable,
            limits,
            &mut budget,
            &mut counters,
            location(),
        ),
        Route::Generic => {
            counters.record(Event::ExpressionEvaluation);
            let frame = Frame {
                values: &mut evaluation.values,
            };
            Execution {
                limits,
                budget: &mut budget,
                counters: &mut counters,
                location: location(),
            }
            .value_suffix(&expression.nodes, &variable, frame.values)
            .map(|()| frame.values.pop().expect("fixture has root"))
        }
    };
    // A guaranteed refused charge exposes the already consumed payload without
    // changing the budget or adding a production statistics accessor for tests.
    let probe = usize::MAX as u128 + 1;
    let Err(ExpansionFailure::Limit { observed, .. }) =
        budget.charge(ExpansionResource::ScalarBytes, probe, location())
    else {
        panic!("probe exceeds every usize payload ceiling");
    };
    assert!(evaluation.values.is_empty());
    Observation {
        outcome: outcome(result),
        work: counters.work,
        lookups: lookups.into_inner(),
        copied_payload: observed - probe,
    }
}

fn compare(expression: &Expression, assignment: &[Value], work: u64, payload: usize) {
    assert_eq!(
        observe(expression, assignment, work, payload, Route::Numeric),
        observe(expression, assignment, work, payload, Route::Generic),
    );
}

#[test]
fn numeric_operators_preserve_generic_failure_boundaries() {
    for operator in OPERATORS {
        let expression = Expression {
            nodes: vec![
                Operation::Variable(0),
                Operation::Variable(1),
                Operation::Binary(operator, 0, 1),
            ],
        };
        for left in [i32::MIN, -32, -1, 0, 1, i32::MAX] {
            for right in [i32::MIN, -1, 0, 1, 32, i32::MAX] {
                for work in 0..=3 {
                    compare(
                        &expression,
                        &[Value::Number(left), Value::Number(right)],
                        work,
                        0,
                    );
                }
            }
        }
    }
}

proptest! {
    #[test]
    fn mixed_numeric_plans_preserve_generic_outcomes(
        left in any::<i32>(), right in any::<i32>(),
        operator in 0_usize..OPERATORS.len(), work in 0_u64..8,
    ) {
        let expression = Expression {
            nodes: vec![
                Operation::Variable(0),
                Operation::Unary(UnaryOp::BitwiseNot, 0),
                Operation::Variable(1),
                Operation::Binary(OPERATORS[operator], 1, 2),
                Operation::Unary(UnaryOp::Negate, 3),
                Operation::Absolute(4),
            ],
        };
        compare(&expression, &[Value::Number(left), Value::Number(right)], work, 0);
    }
}

fn numeric_nodes(count: usize) -> Vec<Operation> {
    (0..count)
        .map(|_| Operation::Constant(Value::Number(7)))
        .collect()
}

#[test]
fn numeric_storage_threshold_preserves_generic_results() {
    for count in [
        NUMERIC_PREFIX_CELLS - 1,
        NUMERIC_PREFIX_CELLS,
        NUMERIC_PREFIX_CELLS + 1,
        NUMERIC_PREFIX_CELLS * 2,
    ] {
        let mut nodes = numeric_nodes(count);
        nodes.push(Operation::Binary(BinaryOp::Add, 0, count - 1));
        let expression = Expression { nodes };
        for work in 0..=u64::try_from(expression.nodes.len()).unwrap() {
            compare(&expression, &[], work, 0);
        }
    }
}

#[test]
fn numeric_prefix_avoids_value_workspace_allocation() {
    let mut evaluation = Evaluation::default();
    let expression = Expression {
        nodes: numeric_nodes(NUMERIC_PREFIX_CELLS),
    };
    let result = evaluation
        .expression(
            &expression,
            |_| unreachable!("constant fixture"),
            FormulaLimits::default(),
            &mut Budget::new(ExpansionLimits::default(), usize::MAX),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    assert_eq!(result, Value::Number(7));
    assert_eq!(evaluation.values.capacity(), 0);
}

#[test]
fn nonnumeric_transition_does_not_replay_bindings() {
    let expression = Expression {
        nodes: vec![
            Operation::Variable(0),
            Operation::Constant(Value::Number(2)),
            Operation::Binary(BinaryOp::Mul, 0, 1),
            Operation::Variable(1),
            Operation::Variable(0),
            Operation::Binary(BinaryOp::Add, 2, 4),
        ],
    };
    let assignment = [Value::Number(3), Value::String("tag".into())];
    for work in 0..=6 {
        for payload in 0..=3 {
            compare(&expression, &assignment, work, payload);
        }
    }
    let complete = observe(&expression, &assignment, 6, 3, Route::Numeric);
    assert_eq!(complete.lookups, [0, 1, 0]);
    assert_eq!(complete.outcome, Outcome::Value(Value::Number(9)));
}

#[test]
fn structured_transition_preserves_payload_work() {
    let structure = Value::from_nodes(
        vec![
            zetesis_core::ValueNode::Tuple { arity: 1 },
            zetesis_core::ValueNode::Number(7),
        ],
        zetesis_core::ValueLimits::default(),
    )
    .unwrap();
    let expression = Expression {
        nodes: vec![
            Operation::Constant(Value::Number(2)),
            Operation::Variable(0),
        ],
    };
    let assignment = [structure];
    let baseline = observe(
        &expression,
        &assignment,
        u64::MAX,
        usize::MAX,
        Route::Generic,
    );
    for work in [0, 1, baseline.work - 1, baseline.work] {
        for payload in [0, usize::try_from(baseline.copied_payload).unwrap()] {
            compare(&expression, &assignment, work, payload);
        }
    }
}

#[test]
fn constructors_receive_the_complete_numeric_prefix() {
    for count in [1, NUMERIC_PREFIX_CELLS - 1, NUMERIC_PREFIX_CELLS + 1] {
        let mut nodes = numeric_nodes(count);
        nodes.push(Operation::Constructor(Box::new(Constructor {
            name: Some("f".into()),
            sign: Sign::Positive,
            arguments: vec![0, count - 1],
        })));
        let expression = Expression { nodes };
        let full = observe(&expression, &[], u64::MAX, usize::MAX, Route::Generic);
        for work in [0, u64::try_from(count).unwrap(), full.work - 1, full.work] {
            for payload in [0, usize::try_from(full.copied_payload).unwrap()] {
                compare(&expression, &[], work, payload);
            }
        }
    }
}

#[test]
fn numeric_operand_access_requires_an_evaluated_prefix() {
    let expression = Expression {
        nodes: vec![Operation::Absolute(0)],
    };
    for route in [Route::Numeric, Route::Generic] {
        let result = catch_unwind(AssertUnwindSafe(|| {
            observe(&expression, &[], u64::MAX, usize::MAX, route)
        }));
        assert!(result.is_err());
    }
}

#[test]
fn lookup_unwind_leaves_the_numeric_workspace_empty() {
    let mut evaluation = Evaluation::default();
    let expression = Expression {
        nodes: vec![
            Operation::Constant(Value::Number(7)),
            Operation::Variable(0),
        ],
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        evaluation.expression(
            &expression,
            |_| panic!("injected numeric lookup unwind"),
            FormulaLimits::default(),
            &mut Budget::new(ExpansionLimits::default(), usize::MAX),
            &mut Counters::default(),
            location(),
        )
    }));
    assert!(result.is_err());
    assert_eq!(evaluation.values.capacity(), 0);
}
