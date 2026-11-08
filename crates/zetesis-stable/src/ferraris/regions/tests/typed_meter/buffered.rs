//! Atomic replenishment uses the real candidate Meter and shared permit owner.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::*;
use crate::budget::SharedBudget;

fn source_window(budget: &mut Budget<'_, crate::budget::WorkLease<'_>>) -> u64 {
    let window = budget.reserve_up_to(u64::MAX).unwrap();
    budget.refund(window);
    window
}

#[test]
fn an_atomic_chunk_can_exceed_one_source_window() {
    let cancellation = Cancellation::default();
    let limits = SearchLimits::default();
    let shared = SharedBudget::new(limits, SearchStatistics::default());
    let mut budget = Budget {
        quota: shared.lease(&cancellation),
        limits,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let window = source_window(&mut budget);
    let performed = window.checked_sub(2).unwrap();
    let maximum = window.checked_add(1).unwrap();
    let calls = Cell::new(0);
    {
        let mut quota = BudgetQuota {
            budget: &mut budget,
        };
        let mut meter = Meter::reserved(&mut quota);
        for _ in 0..performed {
            meter.tick().unwrap();
        }
        assert_eq!(meter.remaining(), 2);
        assert_eq!(
            meter.bounded(maximum, || {
                calls.set(calls.get() + 1);
                (17, 3)
            }),
            Ok(17)
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(meter.charged(), performed + 3);
        meter.refund();
        assert_eq!(meter.remaining(), 0);
    }
    assert_eq!(budget.statistics.work, performed + 3);
    drop(budget);
    let mut settled = SearchStatistics::default();
    shared.record(&mut settled);
    assert_eq!(settled.work, performed + 3);
    assert_eq!(settled.decisions, 0);
}

#[test]
fn a_partial_global_remainder_cannot_enter_an_atomic_chunk() {
    let cancellation = Cancellation::default();
    let probe = SharedBudget::new(SearchLimits::default(), SearchStatistics::default());
    let mut probe_budget = Budget {
        quota: probe.lease(&cancellation),
        limits: SearchLimits::default(),
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let window = source_window(&mut probe_budget);
    drop(probe_budget);
    let limits = SearchLimits {
        max_work: window + 5,
        ..SearchLimits::default()
    };
    let shared = SharedBudget::new(limits, SearchStatistics::default());
    let mut budget = Budget {
        quota: shared.lease(&cancellation),
        limits,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let performed = window - 2;
    let calls = Cell::new(0);
    {
        let mut quota = BudgetQuota {
            budget: &mut budget,
        };
        let mut meter = Meter::reserved(&mut quota);
        for _ in 0..performed {
            meter.tick().unwrap();
        }
        assert_eq!(
            meter.bounded(8, || {
                calls.set(1);
                ((), 8)
            }),
            Err(zetesis_algebra::Failure::Refused(Incomplete::WorkLimit))
        );
        assert_eq!(calls.get(), 0);
        assert_eq!((meter.charged(), meter.remaining()), (performed, 0));
        meter.refund();
    }
    assert_eq!(budget.statistics.work, performed);
    drop(budget);
    let mut settled = SearchStatistics::default();
    shared.record(&mut settled);
    assert_eq!(settled.work, performed);
    // The final seven permits remain available to a different, smaller task.
    let remaining = shared.lease(&cancellation);
    remaining.take(7).unwrap();
    drop(remaining);
    shared.record(&mut settled);
    assert_eq!(settled.work, limits.max_work);
}

#[test]
fn cancellation_before_replacement_preserves_the_owned_refund() {
    let cancellation = Cancellation::default();
    let limits = SearchLimits {
        max_work: 10,
        ..SearchLimits::default()
    };
    let shared = SharedBudget::new(limits, SearchStatistics::default());
    let mut budget = Budget {
        quota: shared.lease(&cancellation),
        limits,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    {
        let mut quota = BudgetQuota {
            budget: &mut budget,
        };
        let mut meter = Meter::reserved(&mut quota);
        meter.tick().unwrap();
        assert_eq!(meter.remaining(), 9);
        cancellation.cancel();
        let mut calls = 0;
        assert_eq!(
            meter.bounded(10, || {
                calls += 1;
                ((), 10)
            }),
            Err(zetesis_algebra::Failure::Refused(Incomplete::Cancelled))
        );
        assert_eq!(calls, 0);
        assert_eq!((meter.charged(), meter.remaining()), (1, 9));
        meter.refund();
        assert_eq!((meter.charged(), meter.remaining()), (1, 0));
    }
    assert_eq!(budget.statistics.work, 1);
    drop(budget);
    let mut settled = SearchStatistics::default();
    shared.record(&mut settled);
    assert_eq!(settled.work, 1);
}

/// Wait for the production settlement to publish the waiter's one performed
/// operation. The peer still owns eight of ten permits, so the atomic minimum
/// cannot have been granted. The deadline bounds a broken test; elapsed time
/// is never evidence that the operation reached its replacement boundary.
fn released_prefix(shared: &SharedBudget) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let mut receipt = SearchStatistics::default();
        shared.snapshot(&mut receipt);
        if receipt.work == 1 {
            return;
        }
        assert_eq!(receipt.work, 0);
        assert!(
            Instant::now() < deadline,
            "the waiter did not settle its own prefix"
        );
        thread::yield_now();
    }
}

/// Enter a real shared buffered operation after one performed step. Its peer
/// owns eight permits; this worker owns the remaining two until replacement.
fn wait_for_chunk(
    shared: &SharedBudget,
    cancellation: &Cancellation,
) -> (
    Result<(), zetesis_algebra::Failure<Incomplete>>,
    SearchStatistics,
    u64,
) {
    let mut budget = Budget {
        quota: shared.lease(cancellation),
        limits: SearchLimits {
            max_work: 10,
            ..SearchLimits::default()
        },
        cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut calls = 0;
    let result;
    {
        let mut quota = BudgetQuota {
            budget: &mut budget,
        };
        let mut meter = Meter::reserved(&mut quota);
        meter.tick().unwrap();
        assert_eq!((meter.charged(), meter.remaining()), (1, 1));
        result = meter.bounded(4, || {
            calls += 1;
            ((), 2)
        });
        assert_eq!(meter.charged(), 1 + 2 * calls);
        if result.is_err() {
            assert_eq!(meter.remaining(), 0);
        }
        meter.refund();
        assert_eq!(meter.remaining(), 0);
    }
    assert_eq!(budget.statistics.work, 1 + 2 * calls);
    let receipt = budget.statistics;
    drop(budget);
    (result, receipt, calls)
}

#[test]
fn a_replacement_wait_releases_its_own_permits() {
    let limits = SearchLimits {
        max_work: 10,
        ..SearchLimits::default()
    };
    let shared = SharedBudget::new(limits, SearchStatistics::default());
    let owner_control = Cancellation::default();
    let mut owner = shared.lease(&owner_control);
    let reservation = owner.reserve(8).unwrap();
    let cancellation =
        Cancellation::with_deadline(Instant::now() + Duration::from_secs(5)).unwrap();
    thread::scope(|scope| {
        let (done, result) = mpsc::sync_channel(1);
        let shared = &shared;
        let cancellation = &cancellation;
        let worker = scope.spawn(move || {
            done.send(wait_for_chunk(shared, cancellation)).unwrap();
        });
        released_prefix(shared);
        assert_eq!(result.try_recv(), Err(mpsc::TryRecvError::Empty));
        Meter::under_reservation(reservation, |_| Ok::<_, Incomplete>(((), 3))).unwrap();
        let (outcome, receipt, calls) = result.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(outcome, Ok(()));
        assert_eq!(
            receipt,
            SearchStatistics {
                work: 3,
                decisions: 0
            }
        );
        assert_eq!(calls, 1);
        worker.join().unwrap();
    });
    drop(owner);
    let mut settled = SearchStatistics::default();
    shared.record(&mut settled);
    assert_eq!(
        settled,
        SearchStatistics {
            work: 6,
            decisions: 0
        }
    );
}

#[test]
fn cancellation_during_replacement_keeps_only_performed_work() {
    let limits = SearchLimits {
        max_work: 10,
        ..SearchLimits::default()
    };
    let shared = SharedBudget::new(limits, SearchStatistics::default());
    let owner_control = Cancellation::default();
    let mut owner = shared.lease(&owner_control);
    let reservation = owner.reserve(8).unwrap();
    let cancellation =
        Cancellation::with_deadline(Instant::now() + Duration::from_secs(5)).unwrap();
    thread::scope(|scope| {
        let (done, result) = mpsc::sync_channel(1);
        let shared = &shared;
        let cancellation = &cancellation;
        let worker = scope.spawn(move || {
            done.send(wait_for_chunk(shared, cancellation)).unwrap();
        });
        released_prefix(shared);
        cancellation.cancel();
        let (outcome, receipt, calls) = result.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(
            outcome,
            Err(zetesis_algebra::Failure::Refused(Incomplete::Cancelled))
        );
        assert_eq!(
            receipt,
            SearchStatistics {
                work: 1,
                decisions: 0
            }
        );
        assert_eq!(calls, 0);
        worker.join().unwrap();
    });
    Meter::under_reservation(reservation, |_| Ok::<_, Incomplete>(((), 0))).unwrap();
    drop(owner);
    let mut settled = SearchStatistics::default();
    shared.record(&mut settled);
    assert_eq!(
        settled,
        SearchStatistics {
            work: 1,
            decisions: 0
        }
    );
    let peer = shared.lease(&owner_control);
    peer.take(9).unwrap();
    drop(peer);
    shared.record(&mut settled);
    assert_eq!(settled.work, 10);
}

#[test]
fn an_unreceipted_buffered_chunk_keeps_conservative_settlement() {
    let cancellation = Cancellation::default();
    let limits = SearchLimits::default();
    let shared = SharedBudget::new(limits, SearchStatistics::default());
    let mut observed_window = 0;
    let entered = Cell::new(false);
    let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut budget = Budget {
            quota: shared.lease(&cancellation),
            limits,
            cancellation: &cancellation,
            statistics: SearchStatistics::default(),
        };
        let window = source_window(&mut budget);
        observed_window = window;
        let mut quota = BudgetQuota {
            budget: &mut budget,
        };
        let mut meter = Meter::reserved(&mut quota);
        for _ in 0..window - 2 {
            meter.tick().unwrap();
        }
        let _ = meter.bounded(3, || -> ((), u64) {
            entered.set(true);
            panic!("the bounded operation returned no actual-work receipt");
        });
    }));
    assert!(stopped.is_err());
    assert!(entered.get());
    assert!(observed_window > 2);
    let mut settled = SearchStatistics::default();
    shared.record(&mut settled);
    // No refund or reuse follows the unwind. Only the new borrowed prefix is
    // conservatively used; the outer lease returns its unborrowed remainder.
    assert_eq!(
        settled.work,
        observed_window - 2 + zetesis_algebra::POLL_INTERVAL
    );
    assert_eq!(settled.decisions, 0);
    let later = shared.lease(&cancellation);
    later.tick().unwrap();
    drop(later);
    shared.record(&mut settled);
    assert_eq!(
        settled.work,
        observed_window - 1 + zetesis_algebra::POLL_INTERVAL
    );
}
