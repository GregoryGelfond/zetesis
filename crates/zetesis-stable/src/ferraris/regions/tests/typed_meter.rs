//! Original quota payloads survive actual narrowing and receipt publication.

mod buffered;

use std::cell::Cell;

use super::*;

/// Inject a named refusal after a finite prefix, retaining the normal local
/// account for every operation outside per-read reservation.
struct RefuseAfter {
    remaining: Cell<u64>,
    error: Incomplete,
}

impl Quota for RefuseAfter {
    type Reservation<'call> = <LocalQuota as Quota>::Reservation<'call>;

    fn work(&self, spent: u64, ceiling: u64) -> Result<(), Incomplete> {
        LocalQuota.work(spent, ceiling)
    }

    fn decision(&self, spent: u64, ceiling: u64) -> Result<(), Incomplete> {
        LocalQuota.decision(spent, ceiling)
    }

    fn charge(&self, spent: u64, ceiling: u64, amount: u64) -> Result<(), Incomplete> {
        LocalQuota.charge(spent, ceiling, amount)
    }

    fn reserve_up_to(&self, spent: u64, ceiling: u64, wanted: u64) -> Result<u64, Incomplete> {
        if self.remaining.get() == 0 {
            return Err(self.error);
        }
        let granted = LocalQuota.reserve_up_to(spent, ceiling, wanted.min(self.remaining.get()))?;
        self.remaining.set(self.remaining.get() - granted);
        Ok(granted)
    }

    fn reserve_kernel(
        &mut self,
        spent: u64,
        ceiling: u64,
        wanted: u64,
    ) -> Result<Self::Reservation<'_>, Incomplete> {
        LocalQuota.reserve_kernel(spent, ceiling, wanted)
    }

    fn refund(&self, unspent: u64) {
        self.remaining.set(self.remaining.get() + unspent);
    }
}

fn refused_narrowing(permits: u64, error: Incomplete) -> (NarrowingAttempt<Incomplete>, Region) {
    let theory = positive();
    let narrower = Narrower::new(&theory);
    let cancellation = Cancellation::default();
    let mut budget = Budget {
        quota: RefuseAfter {
            remaining: Cell::new(permits),
            error,
        },
        limits: SearchLimits::default(),
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut region = Region::all_open(theory.atom_count());
    let attempt = BudgetQuota::narrow(&mut budget, |cancellation, meter| {
        narrower.narrow_known_reserved(
            OriginalSubject::new(&theory, None),
            &mut region,
            &mut narrower.knowledge(),
            &mut NarrowingScratch::default(),
            cancellation,
            meter,
        )
    });
    assert_eq!(attempt.result, Err(error));
    assert_eq!(attempt.statistics.work, permits);
    assert_eq!(budget.statistics.work, permits);
    assert_eq!(budget.quota.remaining.get(), 0);
    (attempt, region)
}

#[test]
fn narrowing_preserves_the_original_quota_payload() {
    let failures = [
        Incomplete::DecisionLimit,
        Incomplete::ReductStorage {
            required: 4097,
            limit: 4096,
        },
        Incomplete::NativeNarrowingStorage {
            required: 1025,
            limit: 1024,
        },
        Incomplete::Allocation,
        Incomplete::WrongTheory,
        Incomplete::CounterOverflow,
    ];
    for permits in [0, 1, 3] {
        let (baseline, expected_region) = refused_narrowing(permits, Incomplete::WorkLimit);
        for error in failures {
            let (attempt, region) = refused_narrowing(permits, error);
            assert_eq!(attempt.statistics, baseline.statistics);
            assert_eq!(attempt.preferred, baseline.preferred);
            assert_eq!(region, expected_region);
            let mut counts = RegionCounts::default();
            assert_eq!(
                SubjectAttempt::from_narrowing(&attempt).account(&mut counts),
                Err(error)
            );
            assert_eq!(counts.work, permits);
            assert_eq!(counts.propagations, attempt.statistics.propagations);
            assert_eq!(counts.held, attempt.statistics.held);
            assert_eq!(counts.cut, attempt.statistics.cut);
        }
    }
}

#[test]
fn receipt_overflow_precedes_a_typed_narrowing_refusal() {
    let (attempt, _) = refused_narrowing(
        1,
        Incomplete::ReductStorage {
            required: 4097,
            limit: 4096,
        },
    );
    let mut counts = RegionCounts {
        work: u64::MAX,
        ..RegionCounts::default()
    };
    let before = counts;
    assert_eq!(
        SubjectAttempt::from_narrowing(&attempt).account(&mut counts),
        Err(Incomplete::CounterOverflow)
    );
    assert_eq!(counts, before);
}

#[test]
fn buffered_refusal_returns_unused_permits_after_a_lease_refill() {
    // The common Meter can retain an unused prefix while a bounded chunk
    // needs another grant. Neither a failed chunk nor its unused permits is
    // performed work, including after the source tries to renew its grant.
    for (ceiling, performed) in [(2, 0), (5, 3)] {
        let cancellation = Cancellation::default();
        let limits = SearchLimits {
            max_work: ceiling,
            ..SearchLimits::default()
        };
        let shared = crate::budget::SharedBudget::new(limits, SearchStatistics::default());
        let mut budget = Budget {
            quota: shared.lease(&cancellation),
            limits,
            cancellation: &cancellation,
            statistics: SearchStatistics::default(),
        };
        let reached = Cell::new(false);
        {
            let mut quota = BudgetQuota {
                budget: &mut budget,
            };
            let mut meter = Meter::reserved(&mut quota);
            for _ in 0..performed {
                meter.tick().unwrap();
            }
            assert_eq!(
                meter.bounded(3, || {
                    reached.set(true);
                    ((), 3)
                }),
                Err(zetesis_algebra::Failure::Refused(Incomplete::WorkLimit))
            );
            assert!(!reached.get());
            // Replacement returned the old unused borrow before its refusal.
            assert_eq!((meter.charged(), meter.remaining()), (performed, 0));
            meter.refund();
            assert_eq!((meter.charged(), meter.remaining()), (performed, 0));
        }
        assert_eq!(budget.statistics.work, performed);
        drop(budget);
        let mut settled = SearchStatistics::default();
        shared.record(&mut settled);
        assert_eq!(settled.work, performed);
        assert_eq!(settled.decisions, 0);
    }
}

#[test]
fn unused_chunk_grants_return_after_successful_refill() {
    let cancellation = Cancellation::default();
    let limits = SearchLimits {
        max_work: u64::MAX,
        ..SearchLimits::default()
    };
    let shared = crate::budget::SharedBudget::new(limits, SearchStatistics::default());
    let mut budget = Budget {
        quota: shared.lease(&cancellation),
        limits,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    // Observe the real lease size instead of copying its scheduling constant.
    let grant = budget.reserve_up_to(u64::MAX).unwrap();
    budget.refund(grant);
    let performed = grant.checked_sub(2).expect("the lease covers this prefix");
    {
        let mut quota = BudgetQuota {
            budget: &mut budget,
        };
        let mut meter = Meter::reserved(&mut quota);
        for _ in 0..performed {
            meter.tick().unwrap();
        }
        assert_eq!(meter.remaining(), 2);
        // The complete chunk is admitted, but the operation can return an
        // allocation refusal before any of its reserved work is performed.
        assert_eq!(
            meter.bounded(3, || (Err::<(), _>(Incomplete::Allocation), 0)),
            Ok(Err(Incomplete::Allocation))
        );
        assert_eq!(meter.charged(), performed);
        meter.refund();
        assert_eq!((meter.charged(), meter.remaining()), (performed, 0));
    }
    assert_eq!(budget.statistics.work, performed);
    drop(budget);
    let mut settled = SearchStatistics::default();
    shared.record(&mut settled);
    assert_eq!(settled.work, performed);
    assert_eq!(settled.decisions, 0);
}
