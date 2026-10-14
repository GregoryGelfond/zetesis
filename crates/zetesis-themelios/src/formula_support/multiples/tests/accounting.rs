use super::*;
use crate::FormulaResource;

fn at_probe(
    run: impl FnOnce(&mut Join<'_, '_>, &mut Computation<'_, '_>, &mut Counters, ProgramSite),
) {
    testing::with_completed_source(DENSE, |program, support, computation, counters| {
        let rule = rule(program);
        let limits = FormulaLimits::default();
        let mut join = Join::rule(
            rule,
            support,
            computation,
            &limits,
            &mut testing::budget(),
            counters,
        )
        .unwrap();
        join.select_total_body(rule, computation, &limits, counters)
            .unwrap();
        let prepared = join.multiples.expect("positive bound divisor plan");
        let Divisor::Variable(slot) = prepared.divisor else {
            panic!("variable divisor")
        };
        // A source occurrence need not already be in the computation's
        // scalar lookup. Prepare its binding through the ordinary checked door
        // before measuring the target probe or setting its refusal allowance.
        let key = computation
            .number(3, &limits, counters, rule.location)
            .unwrap();
        join.values
            .set(slot, &key, &limits, counters, rule.location)
            .unwrap();
        join.depth = prepared.depth;
        run(&mut join, computation, counters, rule.location);
    });
}

#[test]
fn every_refused_probe_prefix_is_unpublished() {
    let mut required = 0;
    at_probe(|join, computation, counters, location| {
        let start = counters.accounting.work;
        join.prepare_probe(
            join.plan.patterns[join.depth],
            Context::new(computation, &FormulaLimits::default(), counters, location),
        )
        .unwrap();
        assert!(matches!(join.probes[join.depth], Some(Probe::Owned(_))));
        required = counters.accounting.work - start;
    });
    for allowed in 0..required {
        at_probe(|join, computation, counters, location| {
            let limits = FormulaLimits {
                max_work: counters.accounting.work + allowed,
                ..Default::default()
            };
            let live = join.support.live_bytes();
            assert!(matches!(
                join.prepare_probe(
                    join.plan.patterns[join.depth],
                    Context::new(computation, &limits, counters, location)
                ),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ));
            assert!(join.probes[join.depth].is_none());
            assert_eq!(
                join.support.live_bytes(),
                live,
                "refused private buffer was released"
            );
            join.prepare_probe(
                join.plan.patterns[join.depth],
                Context::new(computation, &FormulaLimits::default(), counters, location),
            )
            .unwrap();
            assert!(matches!(join.probes[join.depth], Some(Probe::Owned(_))));
        });
    }
}

#[test]
fn owned_probe_payload_is_released_with_its_cursor() {
    at_probe(|join, computation, counters, location| {
        let live = join.support.live_bytes();
        join.prepare_probe(
            join.plan.patterns[join.depth],
            Context::new(computation, &FormulaLimits::default(), counters, location),
        )
        .unwrap();
        let Some(Probe::Owned(rows)) = &join.probes[join.depth] else {
            panic!("owned selection")
        };
        assert_eq!(rows.len(), 8);
        assert!(join.support.live_bytes() >= live + rows.len() * size_of::<usize>());
        join.probes[join.depth] = None;
        assert_eq!(join.support.live_bytes(), live);
    });
}

#[test]
fn query_capacity_refusal_never_becomes_no_rows() {
    at_probe(|join, computation, counters, location| {
        let limits = FormulaLimits {
            max_support_bytes: join.support.live_bytes(),
            ..Default::default()
        };
        assert!(matches!(
            join.prepare_probe(
                join.plan.patterns[join.depth],
                Context::new(computation, &limits, counters, location)
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                ..
            })
        ));
        assert!(join.probes[join.depth].is_none());
        join.prepare_probe(
            join.plan.patterns[join.depth],
            Context::new(computation, &FormulaLimits::default(), counters, location),
        )
        .unwrap();
        assert!(matches!(join.probes[join.depth], Some(Probe::Owned(_))));
    });
}

#[test]
fn cancelled_queries_publish_no_probe_and_retry() {
    at_probe(|join, computation, counters, location| {
        let cancellation = zetesis_cpu::Cancellation::default();
        cancellation.cancel();
        counters.attach_test_control(&cancellation);
        let live = join.support.live_bytes();
        assert!(matches!(
            join.prepare_probe(
                join.plan.patterns[join.depth],
                Context::new(computation, &FormulaLimits::default(), counters, location)
            ),
            Err(FormulaFailure::Interrupted {
                reason: zetesis_cpu::Stop::Cancelled,
                ..
            })
        ));
        assert!(join.probes[join.depth].is_none());
        assert_eq!(join.support.live_bytes(), live);
        counters.detach_test_control();
        join.prepare_probe(
            join.plan.patterns[join.depth],
            Context::new(computation, &FormulaLimits::default(), counters, location),
        )
        .unwrap();
        assert!(matches!(join.probes[join.depth], Some(Probe::Owned(_))));
    });
}

#[test]
fn a_prepared_target_requires_its_occurrence_owner() {
    at_probe(|join, computation, counters, location| {
        let limits = FormulaLimits::default();
        let foreign = join
            .support
            .resolve(join.plan.patterns[0].atom(), &limits, counters, location)
            .unwrap();
        assert!(foreign.is_some());
        let live = join.support.live_bytes();
        for source in [None, foreign] {
            assert!(matches!(
                join.multiples_probe(
                    source,
                    Context::new(computation, &limits, counters, location)
                ),
                Err(FormulaFailure::SupportRelation {
                    error: zetesis_core::relation::Failure::Owner,
                    ..
                })
            ));
            assert!(join.probes[join.depth].is_none());
            assert_eq!(join.support.live_bytes(), live);
        }
    });
}
