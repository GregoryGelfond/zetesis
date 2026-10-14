//! Refused snapshot preparation cannot publish an unauthenticated descriptor.

use super::*;
use crate::FormulaResource;

fn at_delta(
    run: impl for<'rule> FnOnce(
        &'rule RuleIr,
        &mut Join<'rule, '_>,
        &mut Computation<'_, '_>,
        &mut Counters,
    ),
) {
    let mut snapshot = Snapshot::new(FIRST);
    snapshot.publish(population(3, 0));
    snapshot.publish(population(4, 1));
    snapshot.with(|rule, support, computation, counters| {
        let pivot = occurrence(rule, computation, counters, true);
        let mut join = Join::variant_rule(
            rule,
            Variant::Delta(pivot),
            support,
            None,
            &mut testing::budget(),
            Context::new(
                computation,
                &FormulaLimits::default(),
                counters,
                rule.location,
            ),
        )
        .unwrap();
        run(rule, &mut join, computation, counters);
    });
}

#[test]
fn refused_round_preparation_publishes_no_selection() {
    let mut required = 0;
    at_delta(|rule, join, computation, counters| {
        let start = counters.accounting.work;
        assert!(
            join.select_round_multiples(rule, computation, &FormulaLimits::default(), counters)
                .unwrap()
        );
        required = counters.accounting.work - start;
    });
    assert!(required > 1);
    for allowed in 0..required {
        at_delta(|rule, join, computation, counters| {
            let limits = FormulaLimits {
                max_work: counters.accounting.work + allowed,
                ..Default::default()
            };
            assert!(matches!(
                join.select_round_multiples(rule, computation, &limits, counters),
                Err(FormulaFailure::Limit { resource: FormulaResource::Work, location, .. }) if location == rule.location
            ));
            assert!(join.multiples.is_none());
            assert!(!join.certified_total);
            assert!(
                join.select_round_multiples(rule, computation, &FormulaLimits::default(), counters)
                    .unwrap()
            );
            assert!(collect(join, computation, counters, rule.location).owned);
        });
    }
}

#[test]
fn cancelled_round_preparation_can_retry() {
    at_delta(|rule, join, computation, counters| {
        let cancellation = zetesis_cpu::Cancellation::default();
        cancellation.cancel();
        counters.attach_test_control(&cancellation);
        assert!(matches!(
            join.select_round_multiples(rule, computation, &FormulaLimits::default(), counters),
            Err(FormulaFailure::Interrupted { reason: zetesis_cpu::Stop::Cancelled, location }) if location == rule.location
        ));
        assert!(join.multiples.is_none());
        assert!(!join.certified_total);
        counters.detach_test_control();
        assert!(
            join.select_round_multiples(rule, computation, &FormulaLimits::default(), counters)
                .unwrap()
        );
        join.evidence();
        assert!(join.multiples.is_none());
        assert!(!collect(join, computation, counters, rule.location).owned);
    });
}
