//! Optional proof refusal leaves no descriptor or numeric scratch behind.

use super::*;
use zetesis_cpu::{Cancellation, Stop};

fn attempt(
    run: impl for<'a, 'source> FnOnce(
        &mut Join<'a, 'source>,
        &'a RuleIr,
        &mut Computation<'_, '_>,
        &mut Counters,
    ),
) {
    testing::with_completed_source(CLOSED, |program, support, computation, counters| {
        let rule = range_rule(program);
        let mut join = Join::rule(
            rule,
            support,
            computation,
            &FormulaLimits::default(),
            &mut testing::budget(),
            counters,
        )
        .unwrap();
        run(&mut join, rule, computation, counters);
    });
}

#[test]
fn refused_preparation_prefixes_publish_no_selection() {
    let mut required = 0;
    attempt(|join, rule, computation, counters| {
        let start = counters.accounting.work;
        assert!(
            join.select_range_multiples(rule, computation, &FormulaLimits::default(), counters)
                .unwrap()
        );
        required = counters.accounting.work - start;
    });
    assert!(required > 0);
    for allowed in 0..required {
        attempt(|join, rule, computation, counters| {
            let live = join.support.live_bytes();
            let limits = FormulaLimits {
                max_work: counters.accounting.work + allowed,
                ..Default::default()
            };
            assert!(
                matches!(join.select_range_multiples(rule, computation, &limits, counters),
                Err(FormulaFailure::Limit { resource: FormulaResource::Work, location, .. }) if location == rule.location)
            );
            assert!(join.range_selection.is_none());
            assert_eq!(join.support.live_bytes(), live);
            assert!(
                join.select_range_multiples(rule, computation, &FormulaLimits::default(), counters)
                    .unwrap()
            );
            assert_eq!(
                join.support.live_bytes(),
                live,
                "numeric workspace is temporary on success too"
            );
        });
    }
}

#[test]
fn cancelled_preparation_keeps_the_ordinary_owner() {
    attempt(|join, rule, computation, counters| {
        let cancellation = Cancellation::default();
        cancellation.cancel();
        counters.attach_test_control(&cancellation);
        let live = join.support.live_bytes();
        assert!(matches!(
            join.select_range_multiples(rule, computation, &FormulaLimits::default(), counters),
            Err(FormulaFailure::Interrupted {
                reason: Stop::Cancelled,
                ..
            })
        ));
        assert!(join.range_selection.is_none());
        assert_eq!(join.support.live_bytes(), live);
        counters.detach_test_control();
        assert!(
            join.select_range_multiples(rule, computation, &FormulaLimits::default(), counters)
                .unwrap()
        );
    });
}

#[test]
fn numeric_scratch_refusal_is_not_an_empty_range() {
    attempt(|join, rule, computation, counters| {
        let live = join.support.live_bytes();
        let limits = FormulaLimits {
            max_support_bytes: live,
            ..Default::default()
        };
        assert!(matches!(
            join.select_range_multiples(rule, computation, &limits, counters),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                ..
            })
        ));
        assert!(join.range_selection.is_none());
        assert_eq!(join.support.live_bytes(), live);
    });
}

#[test]
fn distinct_rule_bodies_cannot_share_selection() {
    testing::with_completed_source(
        "d(2). p(X) :- d(D),X=1..6,X\\D=0. q(X) :- d(D),X=1..8,X\\D=0.",
        |program, support, computation, counters| {
            let rules: Vec<_> = program
                .rules_without_facts()
                .iter()
                .filter(|rule| {
                    rule.body
                        .iter()
                        .any(|literal| matches!(literal, LiteralIr::Range { binder: true, .. }))
                })
                .collect();
            assert_eq!(rules.len(), 2);
            let mut join = Join::rule(
                rules[0],
                support,
                computation,
                &FormulaLimits::default(),
                &mut testing::budget(),
                counters,
            )
            .unwrap();
            assert!(matches!(
                join.select_range_multiples(
                    rules[1],
                    computation,
                    &FormulaLimits::default(),
                    counters
                ),
                Err(FormulaFailure::SupportRelation {
                    error: zetesis_core::relation::Failure::Owner,
                    ..
                })
            ));
            assert!(join.range_selection.is_none());
        },
    );
}

#[test]
fn bounded_factors_match_checked_remainders() {
    for low in 1..=8 {
        for high in low..=12 {
            for divisor in 1..=14 {
                let expected: Vec<_> = (low..=high).filter(|value| value % divisor == 0).collect();
                let actual: Vec<_> = positive_factors((low, high), divisor)
                    .map(|factor| i32::try_from(factor * i64::from(divisor)).unwrap())
                    .collect();
                assert_eq!(actual, expected);
            }
        }
    }
}
