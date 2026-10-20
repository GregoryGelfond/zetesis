use super::*;
use crate::FormulaResource;

fn at_probe(
    run: impl FnOnce(&mut Join<'_, '_>, &mut Computation<'_, '_>, &mut Counters, ProgramSite),
) {
    testing::with_completed_source(
        "number(2..30). divisor(3). h(N,D) :- number(N),divisor(D),D<N,N\\D=0.",
        |program, support, computation, counters| {
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
            join.select_numeric_probes(rule, &limits, counters).unwrap();
            let prepared = join.multiples.expect("positive body certificate");
            let Divisor::Variable(slot) = prepared.divisor else {
                panic!("variable divisor")
            };
            let value = computation
                .number(3, &limits, counters, rule.location)
                .unwrap();
            join.values
                .set(slot, &value, &limits, counters, rule.location)
                .unwrap();
            join.depth = prepared.depth;
            run(&mut join, computation, counters, rule.location);
        },
    );
}

#[test]
fn refused_intersection_prefixes_publish_nothing() {
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
    for allowance in 0..required {
        at_probe(|join, computation, counters, location| {
            let limits = FormulaLimits {
                max_work: counters.accounting.work + allowance,
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
            assert_eq!(join.support.live_bytes(), live);
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
fn cancelled_intersections_publish_nothing() {
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
