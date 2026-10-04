use super::*;
use crate::ProgramSite;
use crate::formula_ir::Operation;
use crate::formula_support::Counters;
use crate::formula_support::testing::{Fixture, binding};
use crate::{ExpansionFailure, FormulaLimits, FormulaResource};
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Span},
};
use themelios_program::term::{BinaryOp, EvalError};
use zetesis_core::Value;
use zetesis_core::catalog::{AssignmentError, ReadError};

fn location() -> ProgramSite {
    ProgramSite::source(themelios_base::span::Location {
        source: SourceId::new(81),
        span: Span::empty(ByteOffset::new(0)),
    })
}

fn with_expression(
    nodes: impl FnOnce(&mut Fixture) -> Vec<Operation>,
    run: impl FnOnce(
        &[LiteralIr],
        &mut Projections<'_>,
        &mut Evaluation,
        &mut Computation<'_, '_>,
        &mut Counters,
    ),
) {
    let mut fixture = Fixture::default();
    let literals = [LiteralIr::Compare(
        Expression {
            nodes: nodes(&mut fixture),
        },
        Relation::Eq,
        Expression {
            nodes: vec![Operation::Variable(1)],
        },
    )];
    fixture.with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let mut projections =
            Projections::new(&Context::new(&*computation, &limits, counters, location())).unwrap();
        projections
            .prepare(
                &literals,
                Context::new(&*computation, &limits, counters, location()),
            )
            .unwrap();
        run(
            &literals,
            &mut projections,
            &mut Evaluation::default(),
            computation,
            counters,
        );
    });
}

fn quotient(fixture: &mut Fixture) -> Vec<Operation> {
    vec![
        Operation::Variable(0),
        Operation::Constant(fixture.scalar(&Value::Number(2), location())),
        Operation::Binary(BinaryOp::Div, 0, 1),
    ]
}

fn evaluate(
    projections: &mut Projections<'_>,
    literals: &[LiteralIr],
    values: &Binding<'_>,
    evaluation: &mut Evaluation,
    context: Context<'_, &mut Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    let (left, relation, right) = comparison(&literals[0]).unwrap();
    projections.compare(0, ([left, right], relation), values, evaluation, context)
}

#[test]
fn equal_inputs_reuse_successful_results() {
    with_expression(
        quotient,
        |literals, projections, evaluation, computation, counters| {
            let values = binding(
                &[Some(Value::Number(14)), Some(Value::Number(7))],
                computation,
                counters,
                location(),
            );
            let limits = FormulaLimits::default();
            let mut work = Vec::new();
            for _ in 0..2 {
                let before = counters.accounting.work;
                assert!(
                    evaluate(
                        projections,
                        literals,
                        &values,
                        evaluation,
                        Context::new(computation, &limits, counters, location())
                    )
                    .unwrap()
                );
                work.push(counters.accounting.work - before);
            }
            assert_eq!(projections.values[0].results.len(), 1);
            assert!(work[1] < work[0]);
        },
    );
}

#[test]
fn changed_inputs_keep_separate_results() {
    with_expression(
        quotient,
        |literals, projections, evaluation, computation, counters| {
            let limits = FormulaLimits::default();
            for (input, expected) in [(14, true), (16, false), (14, true)] {
                let values = binding(
                    &[Some(Value::Number(input)), Some(Value::Number(7))],
                    computation,
                    counters,
                    location(),
                );
                assert_eq!(
                    evaluate(
                        projections,
                        literals,
                        &values,
                        evaluation,
                        Context::new(computation, &limits, counters, location())
                    )
                    .unwrap(),
                    expected
                );
            }
            assert_eq!(projections.values[0].results.len(), 2);
        },
    );
}

#[test]
fn multiple_inputs_use_ordinary_evaluation() {
    with_expression(
        |_| {
            vec![
                Operation::Variable(0),
                Operation::Variable(1),
                Operation::Binary(BinaryOp::Add, 0, 1),
            ]
        },
        |literals, projections, evaluation, computation, counters| {
            assert!(projections.values.is_empty());
            let values = binding(
                &[Some(Value::Number(0)), Some(Value::Number(7))],
                computation,
                counters,
                location(),
            );
            assert!(
                evaluate(
                    projections,
                    literals,
                    &values,
                    evaluation,
                    Context::new(computation, &FormulaLimits::default(), counters, location())
                )
                .unwrap()
            );
        },
    );
}

#[test]
fn repeated_variable_reads_share_one_projection() {
    with_expression(
        |_| {
            vec![
                Operation::Variable(0),
                Operation::Variable(0),
                Operation::Binary(BinaryOp::Add, 0, 1),
            ]
        },
        |literals, projections, evaluation, computation, counters| {
            assert_eq!(projections.values.len(), 1);
            let values = binding(
                &[Some(Value::Number(2)), Some(Value::Number(4))],
                computation,
                counters,
                location(),
            );
            for _ in 0..2 {
                assert!(
                    evaluate(
                        projections,
                        literals,
                        &values,
                        evaluation,
                        Context::new(computation, &FormulaLimits::default(), counters, location())
                    )
                    .unwrap()
                );
            }
            assert_eq!(projections.values[0].results.len(), 1);
        },
    );
}

fn uncached_failure(
    nodes: impl FnOnce(&mut Fixture) -> Vec<Operation>,
    input: Value,
    expected: &EvalError,
    zero: bool,
) {
    with_expression(
        nodes,
        |literals, projections, evaluation, computation, counters| {
            let values = binding(
                &[Some(input), Some(Value::Number(0))],
                computation,
                counters,
                location(),
            );
            for _ in 0..2 {
                let result = evaluate(
                    projections,
                    literals,
                    &values,
                    evaluation,
                    Context::new(computation, &FormulaLimits::default(), counters, location()),
                );
                assert!(
                    matches!(result, Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, .. })) if &error == expected)
                );
                assert_eq!(evaluation.zero_divisor(), zero);
                assert_eq!(projections.values[0].results.len(), 0);
            }
        },
    );
}

#[test]
fn zero_divisor_failures_are_not_retained() {
    uncached_failure(
        |fixture| {
            vec![
                Operation::Constant(fixture.scalar(&Value::Number(1), location())),
                Operation::Variable(0),
                Operation::Binary(BinaryOp::Div, 0, 1),
            ]
        },
        Value::Number(0),
        &EvalError::Undefined,
        true,
    );
}

#[test]
fn nonnumeric_failures_are_not_retained() {
    uncached_failure(
        quotient,
        Value::Symbol("x".into()),
        &EvalError::Undefined,
        false,
    );
}

#[test]
fn overflowing_failures_are_not_retained() {
    uncached_failure(
        |fixture| {
            vec![
                Operation::Variable(0),
                Operation::Constant(fixture.scalar(&Value::Number(-1), location())),
                Operation::Binary(BinaryOp::Div, 0, 1),
            ]
        },
        Value::Number(i32::MIN),
        &EvalError::Overflow,
        false,
    );
}

#[test]
fn successful_hits_clear_prior_zero_evidence() {
    with_expression(
        quotient,
        |literals, projections, evaluation, computation, counters| {
            let values = binding(
                &[Some(Value::Number(14)), Some(Value::Number(7))],
                computation,
                counters,
                location(),
            );
            let limits = FormulaLimits::default();
            assert!(
                evaluate(
                    projections,
                    literals,
                    &values,
                    evaluation,
                    Context::new(computation, &limits, counters, location())
                )
                .unwrap()
            );
            evaluation.zero_divisor_failure(location());
            assert!(
                evaluate(
                    projections,
                    literals,
                    &values,
                    evaluation,
                    Context::new(computation, &limits, counters, location())
                )
                .unwrap()
            );
            assert!(!evaluation.zero_divisor());
        },
    );
}

#[test]
fn foreign_inputs_never_reuse_local_results() {
    let foreign = Fixture::default().with(location(), |_, computation, counters| {
        binding(
            &[Some(Value::Number(14)), Some(Value::Number(7))],
            computation,
            counters,
            location(),
        )
    });
    with_expression(
        quotient,
        |literals, projections, evaluation, computation, counters| {
            let local = binding(
                &[Some(Value::Number(14)), Some(Value::Number(7))],
                computation,
                counters,
                location(),
            );
            let limits = FormulaLimits::default();
            assert!(
                evaluate(
                    projections,
                    literals,
                    &local,
                    evaluation,
                    Context::new(computation, &limits, counters, location())
                )
                .unwrap()
            );
            assert!(matches!(
                evaluate(
                    projections,
                    literals,
                    &foreign,
                    evaluation,
                    Context::new(computation, &limits, counters, location())
                ),
                Err(FormulaFailure::TermAssignment {
                    error: AssignmentError::Read(ReadError::ForeignCatalog),
                    ..
                })
            ));
            assert_eq!(projections.values[0].results.len(), 1);
        },
    );
}

#[test]
fn cache_hits_obey_inclusive_work_limits() {
    with_expression(
        quotient,
        |literals, projections, evaluation, computation, counters| {
            let values = binding(
                &[Some(Value::Number(14)), Some(Value::Number(7))],
                computation,
                counters,
                location(),
            );
            let limits = FormulaLimits::default();
            evaluate(
                projections,
                literals,
                &values,
                evaluation,
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap();
            let before = counters.accounting.work;
            evaluate(
                projections,
                literals,
                &values,
                evaluation,
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap();
            let needed = counters.accounting.work - before;
            assert!(needed > 0);
            for cutoff in 0..=needed {
                let bounded = FormulaLimits {
                    max_work: counters.accounting.work + cutoff,
                    ..limits
                };
                let result = evaluate(
                    projections,
                    literals,
                    &values,
                    evaluation,
                    Context::new(computation, &bounded, counters, location()),
                );
                if cutoff == needed {
                    assert!(result.unwrap());
                } else {
                    assert!(matches!(
                        result,
                        Err(FormulaFailure::Limit {
                            resource: FormulaResource::Work,
                            ..
                        })
                    ));
                }
                assert!(counters.accounting.work <= bounded.max_work);
                assert_eq!(projections.values[0].results.len(), 1);
            }
        },
    );
}

#[test]
fn cache_hits_obey_current_storage_limits() {
    with_expression(
        quotient,
        |literals, projections, evaluation, computation, counters| {
            let values = binding(
                &[Some(Value::Number(14)), Some(Value::Number(7))],
                computation,
                counters,
                location(),
            );
            evaluate(
                projections,
                literals,
                &values,
                evaluation,
                Context::new(computation, &FormulaLimits::default(), counters, location()),
            )
            .unwrap();
            let limits = FormulaLimits {
                max_support_bytes: 0,
                ..FormulaLimits::default()
            };
            assert!(matches!(
                evaluate(
                    projections,
                    literals,
                    &values,
                    evaluation,
                    Context::new(computation, &limits, counters, location())
                ),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::SupportBytes,
                    ..
                })
            ));
        },
    );
}

#[test]
fn cancelled_hits_never_return_a_value() {
    with_expression(
        quotient,
        |literals, projections, evaluation, computation, counters| {
            let values = binding(
                &[Some(Value::Number(14)), Some(Value::Number(7))],
                computation,
                counters,
                location(),
            );
            let limits = FormulaLimits::default();
            evaluate(
                projections,
                literals,
                &values,
                evaluation,
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap();
            let cancellation = zetesis_cpu::Cancellation::default();
            cancellation.cancel();
            counters.cancellation = Some(cancellation);
            assert!(matches!(
                evaluate(
                    projections,
                    literals,
                    &values,
                    evaluation,
                    Context::new(computation, &limits, counters, location())
                ),
                Err(FormulaFailure::Interrupted {
                    reason: zetesis_cpu::Stop::Cancelled,
                    ..
                })
            ));
        },
    );
}

#[test]
fn descriptor_growth_releases_nested_leases() {
    let mut fixture = Fixture::default();
    let literals: Vec<_> = (0..6)
        .map(|_| {
            LiteralIr::Compare(
                Expression {
                    nodes: quotient(&mut fixture),
                },
                Relation::Eq,
                Expression {
                    nodes: vec![Operation::Variable(1)],
                },
            )
        })
        .collect();
    fixture.with(location(), |_, computation, counters| {
        let before = counters.accounting.workspace.bytes();
        let limits = FormulaLimits::default();
        let mut projections =
            Projections::new(&Context::new(&*computation, &limits, counters, location())).unwrap();
        projections
            .prepare(
                &literals,
                Context::new(&*computation, &limits, counters, location()),
            )
            .unwrap();
        assert_eq!(projections.values.len(), 6);
        assert!(counters.accounting.workspace.bytes() > before);
        drop(projections);
        assert_eq!(counters.accounting.workspace.bytes(), before);
    });
}

#[test]
fn borrowed_maps_do_not_retain_uncovered_inputs() {
    with_expression(
        quotient,
        |literals, projections, evaluation, computation, counters| {
            let limits = FormulaLimits::default();
            let covered = binding(
                &[Some(Value::Number(14)), Some(Value::Number(7))],
                computation,
                counters,
                location(),
            );
            assert!(
                evaluate(
                    projections,
                    literals,
                    &covered,
                    evaluation,
                    Context::new(computation, &limits, counters, location())
                )
                .unwrap()
            );
            let retained = projections.values[0].results.len();
            let mut borrowed = Projections::Prepared(projections);
            for input in [16, 16] {
                let values = binding(
                    &[Some(Value::Number(input)), Some(Value::Number(7))],
                    computation,
                    counters,
                    location(),
                );
                assert!(
                    !evaluate(
                        &mut borrowed,
                        literals,
                        &values,
                        evaluation,
                        Context::new(computation, &limits, counters, location())
                    )
                    .unwrap()
                );
            }
            assert_eq!(borrowed.values[0].results.len(), retained);
        },
    );
}

#[test]
fn borrowed_map_misses_preserve_arithmetic_errors() {
    with_expression(
        quotient,
        |literals, projections, evaluation, computation, counters| {
            let values = binding(
                &[Some(Value::Symbol("symbol".into())), Some(Value::Number(7))],
                computation,
                counters,
                location(),
            );
            let mut borrowed = Projections::Prepared(projections);
            assert!(matches!(
                evaluate(
                    &mut borrowed,
                    literals,
                    &values,
                    evaluation,
                    Context::new(computation, &FormulaLimits::default(), counters, location())
                ),
                Err(FormulaFailure::Expansion(
                    ExpansionFailure::Evaluation { .. }
                ))
            ));
            assert_eq!(borrowed.values[0].results.len(), 0);
        },
    );
}
