use super::*;
use crate::formula_support::testing::{Fixture, binding};
use crate::{ExpansionFailure, FormulaLimits};
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Span},
};
use themelios_program::term::{BinaryOp, EvalError};
use zetesis_core::Value;
use zetesis_core::catalog::{AssignmentError, ReadError};

fn location() -> crate::ProgramSite {
    crate::ProgramSite::source(themelios_base::span::Location {
        source: SourceId::new(37),
        span: Span::empty(ByteOffset::new(12)),
    })
}

fn owned_comparison(
    evaluation: &mut Evaluation,
    comparison: ([&Expression; 2], Relation),
    source: &Binding<'_>,
    context: Context<'_, &mut Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    let Context { computation, work } = context;
    let [left, right] = evaluation.source_values(
        comparison.0,
        |slot| source.key(slot, work.location),
        computation,
        work.limits,
        work.counters,
        work.location,
    )?;
    let read = computation.read();
    compare(
        read.term(&left)
            .map_err(|error| scope(error, work.location))?,
        comparison.1,
        read.term(&right)
            .map_err(|error| scope(error, work.location))?,
        work.limits,
        work.counters,
        work.location,
    )
}

#[test]
fn borrowed_leaf_comparisons_preserve_term_order() {
    let values = [
        Value::Infimum,
        Value::Number(-1),
        Value::Number(4),
        Value::Symbol("a".into()),
        Value::String("z".into()),
        Value::Supremum,
    ];
    for left in &values {
        for right in &values {
            for relation in [
                Relation::Eq,
                Relation::Neq,
                Relation::Lt,
                Relation::Le,
                Relation::Gt,
                Relation::Ge,
            ] {
                for constants in [false, true] {
                    let mut fixture = Fixture::default();
                    let first = if constants {
                        Operation::Constant(fixture.scalar(left, location()))
                    } else {
                        Operation::Variable(0)
                    };
                    let second = if constants {
                        Operation::Constant(fixture.scalar(right, location()))
                    } else {
                        Operation::Variable(1)
                    };
                    let expressions = [
                        Expression { nodes: vec![first] },
                        Expression {
                            nodes: vec![second],
                        },
                    ];
                    assert!(leaves([&expressions[0], &expressions[1]]).is_some());
                    fixture.with(location(), |_, computation, counters| {
                        let source = binding(
                            &[Some(left.clone()), Some(right.clone())],
                            computation,
                            counters,
                            location(),
                        );
                        let limits = FormulaLimits::default();
                        let before = counters.accounting.work;
                        let expected = owned_comparison(
                            &mut Evaluation::default(),
                            ([&expressions[0], &expressions[1]], relation),
                            &source,
                            Context::new(&mut *computation, &limits, counters, location()),
                        )
                        .unwrap();
                        let work = counters.accounting.work - before;
                        let before = counters.accounting.work;
                        let actual = Evaluation::default()
                            .source_comparison(
                                ([&expressions[0], &expressions[1]], relation),
                                &source,
                                Context::new(computation, &limits, counters, location()),
                            )
                            .unwrap();
                        assert_eq!(actual, expected);
                        assert_eq!(counters.accounting.work - before, work);
                    });
                }
            }
        }
    }
}

fn bounded_comparison(borrowed: bool, extra: u64, absent: Option<usize>) -> (String, u64, usize) {
    let mut fixture = Fixture::default();
    let scalar = fixture.scalar(&Value::Number(9), location());
    fixture.with(location(), |_, computation, counters| {
        let source = binding(
            &[
                if absent == Some(0) {
                    None
                } else {
                    Some(Value::Number(4))
                },
                if absent == Some(1) {
                    None
                } else {
                    Some(Value::Number(9))
                },
            ],
            computation,
            counters,
            location(),
        );
        let baseline = counters.accounting.work;
        let limits = FormulaLimits {
            max_work: baseline.saturating_add(extra),
            ..Default::default()
        };
        let expressions = [
            Expression {
                nodes: vec![Operation::Variable(0)],
            },
            Expression {
                nodes: vec![if absent == Some(1) {
                    Operation::Variable(1)
                } else {
                    Operation::Constant(scalar)
                }],
            },
        ];
        let mut evaluation = Evaluation::default();
        evaluation.zero_divisor_failure(location());
        let context = Context::new(computation, &limits, counters, location());
        let result = if borrowed {
            evaluation.source_comparison(
                ([&expressions[0], &expressions[1]], Relation::Lt),
                &source,
                context,
            )
        } else {
            owned_comparison(
                &mut evaluation,
                ([&expressions[0], &expressions[1]], Relation::Lt),
                &source,
                context,
            )
        };
        assert!(!evaluation.zero_divisor());
        assert!(evaluation.scratch.terms().is_empty());
        (
            format!("{result:?}"),
            counters.accounting.work - baseline,
            counters.workspace_bytes(),
        )
    })
}

#[test]
fn borrowed_leaf_cutoffs_match_owned_evaluation() {
    let (_, complete_work, _) = bounded_comparison(false, u64::MAX, None);
    assert!(complete_work > 0);
    for absent in [None, Some(0), Some(1)] {
        for allowance in 0..=complete_work {
            assert_eq!(
                bounded_comparison(true, allowance, absent),
                bounded_comparison(false, allowance, absent)
            );
        }
    }
}

#[test]
fn borrowed_variable_refuses_a_foreign_owner() {
    let source = Fixture::default().with(location(), |_, computation, counters| {
        binding(&[Some(Value::Number(7))], computation, counters, location())
    });
    Fixture::default().with(location(), |_, computation, counters| {
        let expression = Expression {
            nodes: vec![Operation::Variable(0)],
        };
        let limits = FormulaLimits::default();
        let expected = owned_comparison(
            &mut Evaluation::default(),
            ([&expression, &expression], Relation::Eq),
            &source,
            Context::new(&mut *computation, &limits, counters, location()),
        );
        let result = Evaluation::default().source_comparison(
            ([&expression, &expression], Relation::Eq),
            &source,
            Context::new(computation, &limits, counters, location()),
        );
        assert_eq!(format!("{result:?}"), format!("{expected:?}"));
        assert!(matches!(
            result,
            Err(FormulaFailure::TermAssignment {
                error: AssignmentError::Read(ReadError::ForeignCatalog),
                ..
            })
        ));
    });
}

#[test]
fn mutable_expressions_keep_owned_comparison_results() {
    let mut fixture = Fixture::default();
    let one = fixture.scalar(&Value::Number(1), location());
    let two = fixture.scalar(&Value::Number(2), location());
    let expressions = [
        Expression {
            nodes: vec![Operation::Variable(0)],
        },
        Expression {
            nodes: vec![
                Operation::Constant(one),
                Operation::Constant(two),
                Operation::Binary(BinaryOp::Add, 0, 1),
            ],
        },
    ];
    assert!(leaves([&expressions[0], &expressions[1]]).is_none());
    fixture.with(location(), |_, computation, counters| {
        let source = binding(&[Some(Value::Number(3))], computation, counters, location());
        assert!(
            Evaluation::default()
                .source_comparison(
                    ([&expressions[0], &expressions[1]], Relation::Eq),
                    &source,
                    Context::new(computation, &FormulaLimits::default(), counters, location())
                )
                .unwrap()
        );
    });
}

#[test]
fn comparison_fallback_retains_undefined_arithmetic() {
    let mut fixture = Fixture::default();
    let zero = fixture.scalar(&Value::Number(0), location());
    let expressions = [
        Expression {
            nodes: vec![
                Operation::Constant(zero),
                Operation::Binary(BinaryOp::Div, 0, 0),
            ],
        },
        Expression {
            nodes: vec![Operation::Variable(0)],
        },
    ];
    assert!(leaves([&expressions[0], &expressions[1]]).is_none());
    fixture.with(location(), |_, computation, counters| {
        let source = binding(&[Some(Value::Number(3))], computation, counters, location());
        let mut evaluation = Evaluation::default();
        let result = evaluation.source_comparison(
            ([&expressions[0], &expressions[1]], Relation::Eq),
            &source,
            Context::new(computation, &FormulaLimits::default(), counters, location()),
        );
        assert!(matches!(
            result,
            Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                error: EvalError::Undefined,
                ..
            }))
        ));
        assert!(evaluation.zero_divisor());
    });
}

#[test]
fn cancellation_precedes_borrowed_variable_lookup() {
    Fixture::default().with(location(), |_, computation, counters| {
        let source = binding(&[], computation, counters, location());
        let cancellation = zetesis_cpu::Cancellation::default();
        cancellation.cancel();
        counters.cancellation = Some(cancellation);
        let expression = Expression {
            nodes: vec![Operation::Variable(usize::MAX)],
        };
        let result = Evaluation::default().source_comparison(
            ([&expression, &expression], Relation::Eq),
            &source,
            Context::new(computation, &FormulaLimits::default(), counters, location()),
        );
        assert!(matches!(
            result,
            Err(FormulaFailure::Interrupted {
                reason: zetesis_cpu::Stop::Cancelled,
                ..
            })
        ));
    });
}

#[test]
fn comparison_reuses_general_expression_scratch() {
    let mut fixture = Fixture::default();
    let one = fixture.scalar(&Value::Number(1), location());
    fixture.with(location(), |_, computation, counters| {
        let source = binding(&[Some(Value::Number(2))], computation, counters, location());
        let limits = FormulaLimits::default();
        let general = Expression {
            nodes: vec![
                Operation::Constant(one),
                Operation::Variable(0),
                Operation::Binary(BinaryOp::Add, 0, 1),
            ],
        };
        let leaf = Expression {
            nodes: vec![Operation::Variable(0)],
        };
        let mut outcomes = Vec::new();
        for borrowed in [false, true] {
            let mut evaluation = Evaluation::default();
            let key = evaluation
                .source_expression(
                    &general,
                    |slot| source.key(slot, location()),
                    computation,
                    &limits,
                    counters,
                    location(),
                )
                .unwrap();
            assert_eq!(computation.read().term(&key).unwrap(), Value::Number(3));
            drop(key);
            let before = counters.accounting.work;
            let context = Context::new(&mut *computation, &limits, counters, location());
            let result = if borrowed {
                evaluation.source_comparison(([&leaf, &leaf], Relation::Eq), &source, context)
            } else {
                owned_comparison(
                    &mut evaluation,
                    ([&leaf, &leaf], Relation::Eq),
                    &source,
                    context,
                )
            }
            .unwrap();
            assert!(evaluation.scratch.terms().is_empty());
            outcomes.push((
                result,
                counters.accounting.work - before,
                counters.workspace_bytes(),
            ));
        }
        assert_eq!(outcomes[0], outcomes[1]);
    });
}
