//! Expression-level order and resource contracts around the pure operators.
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use themelios_program::term::{BinaryOp, EvalError, UnaryOp};
use zetesis_core::{Value, ValueLimits};

use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, Operation};
use crate::formula_support::{Evaluation, testing::Fixture};
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource};

fn location() -> crate::ProgramSite {
    crate::ProgramSite::source(Location {
        source: SourceId::new(17),
        span: Span::empty(ByteOffset::new(23)),
    })
}
fn evaluate(
    plan: impl FnOnce(&mut Fixture) -> Vec<Operation>,
    work: u64,
    extra_storage: Option<usize>,
) -> (Result<Value, FormulaFailure>, u64) {
    let mut fixture = Fixture::default();
    let nodes = plan(&mut fixture);
    fixture.with(location(), |_, computation, counters| {
        let mut limits = FormulaLimits::default();
        let binding = Binding::new(computation, &limits, counters, location()).unwrap();
        if let Some(extra) = extra_storage {
            let observer = computation.lease();
            let current = limits.max_support_bytes
                - computation
                    .allowance(&observer, &limits, location())
                    .unwrap();
            limits.max_support_bytes = current + extra;
        }
        let before = counters.accounting.work;
        limits.max_work = before.saturating_add(work);
        // Strict mode stops at its first failing source node. Export is only
        // this assertion boundary, after evaluation in the canonical authority.
        let result = Evaluation::default()
            .expression(
                &Expression { nodes },
                |variable| binding.key(variable, location()),
                computation,
                &limits,
                counters,
                location(),
            )
            .map(|key| {
                computation
                    .read()
                    .term(&key)
                    .unwrap()
                    .to_value(ValueLimits::default())
                    .unwrap()
            });
        (result, counters.accounting.work - before)
    })
}
fn simple_plan(fixture: &mut Fixture) -> Vec<Operation> {
    vec![
        Operation::Constant(fixture.scalar(&Value::Number(1), location())),
        Operation::Constant(fixture.scalar(&Value::Number(2), location())),
        Operation::Binary(BinaryOp::Add, 0, 1),
        Operation::Unary(UnaryOp::Negate, 2),
        Operation::Absolute(3),
    ]
}

#[test]
fn every_scalar_work_cutoff_refuses_without_returning_a_value() {
    let (result, needed) = evaluate(simple_plan, u64::MAX, None);
    assert_eq!(result.unwrap(), Value::Number(3));
    assert!(needed > 0);
    for cutoff in 0..needed {
        let (failure, spent) = evaluate(simple_plan, cutoff, None);
        assert!(spent <= cutoff);
        assert!(matches!(failure, Err(FormulaFailure::Limit {
            resource: FormulaResource::Work, location: found, .. }) if found == location()));
    }
}

#[test]
fn the_first_failing_node_remains_authoritative() {
    for (first, second, expected) in [
        ((0, 1), (1, 2), EvalError::Overflow),
        ((1, 2), (0, 1), EvalError::Undefined),
    ] {
        let (result, _) = evaluate(
            |fixture| {
                vec![
                    Operation::Constant(fixture.scalar(&Value::Number(i32::MIN), location())),
                    Operation::Constant(fixture.scalar(&Value::Number(-1), location())),
                    Operation::Constant(fixture.scalar(&Value::Number(0), location())),
                    Operation::Binary(BinaryOp::Div, first.0, first.1),
                    Operation::Binary(BinaryOp::Div, second.0, second.1),
                    Operation::Binary(BinaryOp::Add, 3, 4),
                ]
            },
            u64::MAX,
            None,
        );
        assert!(
            matches!(result, Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation {
            error, location: found })) if error == expected && found == location())
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
            let (result, _) = evaluate(
                |fixture| {
                    vec![
                        Operation::Constant(fixture.scalar(&operand, location())),
                        operation,
                    ]
                },
                u64::MAX,
                None,
            );
            assert!(
                matches!(result, Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                error: EvalError::Undefined, location: found })) if found == location())
            );
        }
    }
}

#[test]
fn operand_storage_refusal_precedes_arithmetic() {
    let (result, _) = evaluate(
        |fixture| {
            vec![
                Operation::Constant(
                    fixture.scalar(&Value::Symbol("payload".repeat(8192)), location()),
                ),
                Operation::Unary(UnaryOp::Negate, 0),
            ]
        },
        u64::MAX,
        // The payload was admitted before the interval. Evaluation still needs
        // its bounded metadata scratch before applying the unary operator.
        Some(0),
    );
    assert!(matches!(result, Err(FormulaFailure::Limit {
        resource: FormulaResource::SupportBytes, location: found, .. }) if found == location()));
}

#[test]
fn zero_work_refuses_before_operand_access() {
    let (result, work) = evaluate(
        |fixture| {
            vec![
                Operation::Constant(fixture.scalar(&Value::Symbol("abc".into()), location())),
                Operation::Unary(UnaryOp::Negate, 0),
            ]
        },
        0,
        None,
    );
    assert_eq!(work, 0);
    assert!(matches!(result, Err(FormulaFailure::Limit {
        resource: FormulaResource::Work, location: found, .. }) if found == location()));
}

#[test]
fn admitted_payload_does_not_expand_evaluation_storage() {
    let (result, _) = evaluate(
        |fixture| {
            vec![
                Operation::Constant(
                    fixture.scalar(&Value::Symbol("payload".repeat(8192)), location()),
                ),
                Operation::Unary(UnaryOp::Negate, 0),
            ]
        },
        u64::MAX,
        // This bounds metadata, and is much smaller than the admitted spelling.
        Some(4096),
    );
    assert!(
        matches!(result, Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation {
        error: EvalError::Undefined, location: found,
    })) if found == location())
    );
}
