//! Leaf reads keep canonical identity without numeric reconstruction.

use super::{empty, location};
use crate::formula_ir::{Expression, Operation};
use crate::formula_support::Evaluation;
use crate::formula_support::testing::{Fixture, binding};
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits};
use std::cmp::Ordering;
use themelios_program::term::EvalError;
use zetesis_core::Value;
use zetesis_core::catalog::{AssignmentError, ReadError};

fn values() -> [Value; 5] {
    [
        Value::Number(i32::MIN),
        Value::Number(0),
        Value::Number(i32::MAX),
        Value::Symbol("symbol".into()),
        Value::String("text".into()),
    ]
}

#[test]
fn numeric_variable_leaves_cost_only_identity_reads() {
    let mut fixture = Fixture::default();
    fixture.with(location(), |_, computation, counters| {
        let values = values().map(Some);
        let source = binding(&values, computation, counters, location());
        let limits = FormulaLimits::default();
        let mut work = Vec::new();
        for slot in 0..values.len() {
            let mut evaluation = Evaluation::default();
            let before = counters.accounting.work;
            let key = evaluation
                .expression(
                    &Expression {
                        nodes: vec![Operation::Variable(slot)],
                    },
                    |slot| source.key(slot, location()),
                    computation,
                    &limits,
                    counters,
                    location(),
                )
                .unwrap();
            work.push(counters.accounting.work - before);
            assert_eq!(source.slots().compare_key(slot, &key), Ok(Ordering::Equal));
            assert!(empty(&evaluation));
        }
        // Numeric keys already carry the result, just as strings and symbols
        // do. Re-entering numeric construction adds charged work and violates
        // this bound even when it finds the existing canonical number.
        let text_work = *work.last().unwrap();
        assert!(work.iter().all(|&units| units == text_work));
    });
}

#[test]
fn numeric_constant_leaves_cost_only_identity_reads() {
    let mut fixture = Fixture::default();
    let scalars = values().map(|value| fixture.scalar(&value, location()));
    fixture.with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let mut work = Vec::new();
        for scalar in scalars {
            let expected = computation
                .static_scalar(scalar, &limits, counters, location())
                .unwrap();
            let mut evaluation = Evaluation::default();
            let before = counters.accounting.work;
            let key = evaluation
                .expression(
                    &Expression {
                        nodes: vec![Operation::Constant(scalar)],
                    },
                    |_| unreachable!("constant expression"),
                    computation,
                    &limits,
                    counters,
                    location(),
                )
                .unwrap();
            work.push(counters.accounting.work - before);
            assert_eq!(computation.read().term(&key).unwrap(), expected);
            assert!(empty(&evaluation));
        }
        let text_work = *work.last().unwrap();
        assert!(work.iter().all(|&units| units == text_work));
    });
}

#[test]
fn variable_leaf_refuses_a_retired_foreign_owner() {
    let source = Fixture::default().with(location(), |_, computation, counters| {
        binding(&[Some(Value::Number(7))], computation, counters, location())
    });
    Fixture::default().with(location(), |_, computation, counters| {
        let mut evaluation = Evaluation::default();
        let result = evaluation.expression(
            &Expression {
                nodes: vec![Operation::Variable(0)],
            },
            |slot| source.key(slot, location()),
            computation,
            &FormulaLimits::default(),
            counters,
            location(),
        );
        assert!(matches!(
            result,
            Err(FormulaFailure::TermAssignment {
                error: AssignmentError::Read(ReadError::ForeignCatalog),
                ..
            })
        ));
        assert!(empty(&evaluation));
    });
}

#[test]
fn unavailable_leaf_needs_no_missing_mask() {
    Fixture::default().with(location(), |_, computation, counters| {
        let mut evaluation = Evaluation::default();
        let result = evaluation.source_partial(
            &Expression {
                nodes: vec![Operation::Variable(0)],
            },
            |_| Ok(None),
            computation,
            &FormulaLimits::default(),
            counters,
            location(),
        );
        assert!(matches!(
            result,
            Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                error: EvalError::Undefined,
                ..
            }))
        ));
        assert!(evaluation.zero_divisor());
        assert!(empty(&evaluation));
        assert_eq!(evaluation.scratch.missing.capacity(), 0);
    });
}

#[test]
fn available_leaf_clears_prior_unavailability() {
    Fixture::default().with(location(), |_, computation, counters| {
        let source = binding(&[Some(Value::Number(7))], computation, counters, location());
        let mut evaluation = Evaluation::default();
        evaluation.zero_divisor_failure(location());
        let result = evaluation
            .source_partial(
                &Expression {
                    nodes: vec![Operation::Variable(0)],
                },
                |slot| source.key(slot, location()).map(Some),
                computation,
                &FormulaLimits::default(),
                counters,
                location(),
            )
            .unwrap();
        assert!(!evaluation.zero_divisor());
        assert_eq!(source.slots().compare_key(0, &result), Ok(Ordering::Equal));
    });
}

#[test]
fn cancelled_leaf_never_reads_its_variable() {
    Fixture::default().with(location(), |_, computation, counters| {
        let cancellation = zetesis_cpu::Cancellation::default();
        cancellation.cancel();
        counters.cancellation = Some(cancellation);
        let mut evaluation = Evaluation::default();
        let result = evaluation.expression(
            &Expression {
                nodes: vec![Operation::Variable(0)],
            },
            |_| panic!("cancellation must precede the variable read"),
            computation,
            &FormulaLimits::default(),
            counters,
            location(),
        );
        assert!(matches!(
            result,
            Err(FormulaFailure::Interrupted {
                reason: zetesis_cpu::Stop::Cancelled,
                ..
            })
        ));
        assert!(empty(&evaluation));
    });
}
