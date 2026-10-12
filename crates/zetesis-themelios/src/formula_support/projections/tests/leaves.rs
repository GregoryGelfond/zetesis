//! The projection consumer reuses the existing canonical leaf comparison.

use super::*;

fn leaves() -> [Expression; 2] {
    [0, 1].map(|slot| Expression {
        nodes: vec![Operation::Variable(slot)],
    })
}

#[test]
fn leaf_dispatch_retains_an_unrelated_projection() {
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
            let identity = projections.values.as_ptr();
            let expressions = leaves();
            let before = counters.accounting.work;
            let expected = evaluation
                .source_comparison(
                    ([&expressions[0], &expressions[1]], Relation::Gt),
                    &values,
                    Context::new(computation, &limits, counters, location()),
                )
                .unwrap();
            let borrowed_work = counters.accounting.work - before;
            evaluation.zero_divisor_failure(location());
            let before = counters.accounting.work;
            let actual = projections
                .compare(
                    1,
                    ([&expressions[0], &expressions[1]], Relation::Gt),
                    &values,
                    evaluation,
                    Context::new(computation, &limits, counters, location()),
                )
                .unwrap();
            assert!(expected && actual);
            // Negative control: the old wrapper performs two additional result-key
            // resolutions despite both operands having no projection descriptor.
            assert_eq!(counters.accounting.work - before, borrowed_work);
            assert!(!evaluation.zero_divisor());
            assert_eq!(projections.values.as_ptr(), identity);
            assert_eq!(projections.values.len(), 1);
            assert_eq!(projections.values[0].results.len(), 1);
            assert_eq!(
                projections.values[0]
                    .results
                    .read(0, computation.read(), location())
                    .unwrap(),
                Value::Number(7)
            );
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
            assert_eq!(projections.values[0].results.len(), 1);
        },
    );
}

#[test]
fn leaf_order_does_not_follow_interning_order() {
    for (left, relation, right) in [
        (Value::Symbol("z".into()), Relation::Gt, Value::Number(9)),
        (
            Value::String("a".into()),
            Relation::Gt,
            Value::Symbol("z".into()),
        ),
        (Value::Supremum, Relation::Gt, Value::Infimum),
        (Value::Number(-7), Relation::Lt, Value::Number(3)),
        (
            Value::String("same canonical payload".into()),
            Relation::Eq,
            Value::String("same canonical payload".into()),
        ),
    ] {
        with_expression(
            |_| vec![Operation::Variable(0)],
            |_, projections, evaluation, computation, counters| {
                // Import left first. Raw coordinate order cannot explain the first
                // three semantic greater-than results or the shared equal identity.
                let values = binding(
                    &[Some(left), Some(right)],
                    computation,
                    counters,
                    location(),
                );
                let expressions = leaves();
                assert!(
                    projections
                        .compare(
                            0,
                            ([&expressions[0], &expressions[1]], relation),
                            &values,
                            evaluation,
                            Context::new(
                                computation,
                                &FormulaLimits::default(),
                                counters,
                                location()
                            )
                        )
                        .unwrap()
                );
                assert!(projections.values.is_empty());
            },
        );
    }
}

#[test]
fn leaf_failures_keep_source_presence_and_owner() {
    let foreign = Fixture::default().with(location(), |_, computation, counters| {
        binding(
            &[Some(Value::Number(2)), Some(Value::Number(3))],
            computation,
            counters,
            location(),
        )
    });
    with_expression(
        |_| vec![Operation::Variable(0)],
        |_, projections, evaluation, computation, counters| {
            let absent = binding(
                &[None, Some(Value::Number(3))],
                computation,
                counters,
                location(),
            );
            let expressions = leaves();
            let limits = FormulaLimits::default();
            let result = projections.compare(
                0,
                ([&expressions[0], &expressions[1]], Relation::Eq),
                &absent,
                evaluation,
                Context::new(computation, &limits, counters, location()),
            );
            assert!(
                matches!(result, Err(FormulaFailure::UnsafeVariable { variable: 0, location: site }) if site == location())
            );
            let result = projections.compare(
                0,
                ([&expressions[0], &expressions[1]], Relation::Eq),
                &foreign,
                evaluation,
                Context::new(computation, &limits, counters, location()),
            );
            assert!(
                matches!(result, Err(FormulaFailure::TermAssignment { error: AssignmentError::Read(ReadError::ForeignCatalog), location: site }) if site == location())
            );
            let local = binding(
                &[Some(Value::Number(3)), Some(Value::Number(3))],
                computation,
                counters,
                location(),
            );
            assert!(
                projections
                    .compare(
                        0,
                        ([&expressions[0], &expressions[1]], Relation::Eq),
                        &local,
                        evaluation,
                        Context::new(computation, &limits, counters, location())
                    )
                    .unwrap()
            );
            assert!(projections.values.is_empty());
        },
    );
}

#[test]
fn cancelled_leaf_dispatch_cannot_return_a_value() {
    with_expression(
        |_| vec![Operation::Variable(0)],
        |_, projections, evaluation, computation, counters| {
            let values = binding(
                &[Some(Value::Number(3)), Some(Value::Number(3))],
                computation,
                counters,
                location(),
            );
            let expressions = leaves();
            let cancellation = zetesis_cpu::Cancellation::default();
            cancellation.cancel();
            counters.cancellation = Some(cancellation);
            assert!(matches!(
                projections.compare(
                    0,
                    ([&expressions[0], &expressions[1]], Relation::Eq),
                    &values,
                    evaluation,
                    Context::new(computation, &FormulaLimits::default(), counters, location())
                ),
                Err(FormulaFailure::Interrupted {
                    reason: zetesis_cpu::Stop::Cancelled,
                    ..
                })
            ));
            assert!(projections.values.is_empty());
        },
    );
}
