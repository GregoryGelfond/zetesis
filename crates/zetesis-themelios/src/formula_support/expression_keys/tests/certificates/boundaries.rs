//! Direct common-selector prefix checks; no consumer or query is needed.
use super::*;

fn certificate_work() -> u64 {
    let mut work = 0;
    with_local(|join, computation, counters, limits, location, _| {
        let before = counters.accounting.work;
        join.select_numeric_local(Context::new(&*computation, limits, counters, location))
            .unwrap();
        assert!(join.expression_keys.is_some());
        work = counters.accounting.work - before;
    });
    assert!(work > 0);
    work
}

#[test]
fn every_local_certificate_work_prefix_preserves_atomic_publication() {
    let work = certificate_work();
    for allowed in 0..=work {
        with_local(|join, computation, counters, limits, location, _| {
            assert!(join.expression_keys.is_none());
            let before = counters.accounting.work;
            let patterns = join
                .plan
                .patterns
                .iter()
                .map(|p| p.source)
                .collect::<Vec<_>>();
            let bounded = FormulaLimits {
                max_work: before + allowed,
                ..*limits
            };
            let result = join.select_numeric_local(Context::new(
                &*computation,
                &bounded,
                counters,
                location,
            ));
            assert_eq!(counters.accounting.work, before + allowed);
            if allowed == work {
                result.unwrap();
                assert!(join.expression_keys.is_some());
            } else {
                assert!(matches!(result, Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work, observed, limit, location: site,
                }) if observed == u128::from(before + allowed + 1)
                    && limit == u128::from(before + allowed) && site == location));
                assert!(join.expression_keys.is_none());
            }
            assert_eq!(
                join.plan
                    .patterns
                    .iter()
                    .map(|p| p.source)
                    .collect::<Vec<_>>(),
                patterns
            );
        });
    }
}

#[test]
fn pending_stop_at_each_certificate_prefix_uses_the_production_cadence() {
    let work = certificate_work();
    assert!(work < 256, "the small fixture fits one polling interval");
    for allowed in 0..work {
        with_local(|join, computation, counters, limits, location, _| {
            assert!(join.expression_keys.is_none());
            let patterns = join
                .plan
                .patterns
                .iter()
                .map(|p| p.source)
                .collect::<Vec<_>>();
            let mut accounting = std::mem::take(&mut counters.accounting);
            // A valid prior charge history places the next production poll
            // after exactly `allowed` unit charges in this direct operation.
            let boundary = (accounting.work + work).div_ceil(256) * 256;
            accounting.work = boundary - allowed;
            let before = accounting.work;
            let cancellation = zetesis_algebra::Cancellation::default();
            let result = accounting.controlled(
                Some(&cancellation),
                crate::grounding_observer::Work::default(),
                location,
                |active| {
                    // Entry succeeded. The stop is already pending when the
                    // selector starts; allowed=0 is its direct pre-cancel case.
                    cancellation.cancel();
                    join.select_numeric_local(Context::new(&*computation, limits, active, location))
                },
            );
            assert!(matches!(result, Err(FormulaFailure::Interrupted {
                reason: zetesis_algebra::Stop::Cancelled, location: site,
            }) if site == location));
            assert_eq!(accounting.work, before + allowed);
            assert!(join.expression_keys.is_none());
            assert_eq!(
                join.plan
                    .patterns
                    .iter()
                    .map(|p| p.source)
                    .collect::<Vec<_>>(),
                patterns
            );
            counters.accounting = accounting;
        });
    }
}
