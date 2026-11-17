use super::*;
use crate::formula_support::testing::Fixture;
use crate::test_support::location;
use std::num::NonZeroUsize;

#[test]
fn attached_reservation_preserves_the_full_publication_boundary() {
    let executor = Executor::new(NonZeroUsize::new(3).unwrap()).unwrap();
    Fixture::default().with(location(), |_, computation, counters| {
        counters.observed = Work::worker(true);
        let limits = FormulaLimits {
            max_support_bytes: 1 << 20,
            ..FormulaLimits::default()
        };
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
        let original = computation.query_support().live_bytes();
        let mut completed = run(
            &executor,
            3,
            &mut budget,
            Context::new(&*computation, &limits, counters, location()),
            |_, _, _, _| Ok(()),
        )
        .unwrap();
        let retained = completed.reserved;
        assert!(retained > 0);
        let before = computation.query_support().live_bytes();
        let probe = computation.lease();
        let allowance = computation
            .allowance(&probe, &completed.publication_limits(&limits), location())
            .unwrap();
        assert_eq!(counters.observed.snapshot().support_peak_bytes, None);
        completed
            .attach_reservation(Context::new(&*computation, &limits, counters, location()))
            .unwrap();
        assert_eq!(computation.query_support().live_bytes(), before + retained);
        assert_eq!(
            completed.publication_limits(&limits).max_support_bytes,
            limits.max_support_bytes
        );
        assert_eq!(
            computation.allowance(&probe, &limits, location()).unwrap(),
            allowance
        );
        assert_eq!(completed.results().count(), 3);
        assert_eq!(counters.observed.snapshot().support_peak_bytes, None);
        drop(completed);
        assert_eq!(computation.query_support().live_bytes(), original);
    });
}

#[test]
fn refused_attachment_reports_a_bound_without_an_allocated_peak() {
    let executor = Executor::new(NonZeroUsize::new(1).unwrap()).unwrap();
    Fixture::default().with(location(), |_, computation, counters| {
        counters.observed = Work::worker(true);
        let limits = FormulaLimits { max_support_bytes: 1 << 20, ..FormulaLimits::default() };
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
        let original = computation.query_support().live_bytes();
        let mut completed = run(&executor, 1, &mut budget,
            Context::new(&*computation, &limits, counters, location()),
            |_, _, _, _| Ok(())).unwrap();
        let required = computation.query_support().live_bytes() + completed.reserved;
        let tight = FormulaLimits { max_support_bytes: required - 1, ..limits };
        assert!(matches!(completed.attach_reservation(
            Context::new(&*computation, &tight, counters, location())),
            Err(FormulaFailure::Limit { resource: FormulaResource::SupportBytes,
                observed, limit, .. }) if observed == required as u128 && limit == (required - 1) as u128));
        assert_eq!(counters.observed.snapshot().support_peak_bytes, None);
        drop(completed);
        assert_eq!(computation.query_support().live_bytes(), original);
    });
}
