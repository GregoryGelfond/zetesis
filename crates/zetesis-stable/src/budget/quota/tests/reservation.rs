//! Whole-kernel receipt publication retains the local and shared boundaries.

use super::{
    Budget, Cancellation, Incomplete, LocalQuota, Quota, SearchLimits, SearchStatistics,
    SharedBudget,
};

fn native_failure(budget: &mut Budget<'_, impl Quota>, cancellation: &Cancellation) {
    let native = Err::<(), _>(Incomplete::Allocation);
    let mut calls = 0;
    let result = budget.with_reservation(20, |granted| {
        calls += 1;
        assert_eq!(granted, 7);
        cancellation.cancel();
        (native, 5)
    });
    // No exit poll replaces the returned, accounted native error.
    assert_eq!(result, Ok(native));
    assert_eq!(calls, 1);
    assert_eq!(budget.statistics.work, 5);
    assert_eq!(budget.tick(), Err(Incomplete::Cancelled));
    assert_eq!(budget.statistics.work, 5);
}

#[test]
fn local_and_shared_budgets_publish_failed_receipts_before_the_next_poll() {
    let limits = SearchLimits {
        max_work: 7,
        max_decisions: 0,
    };
    let local_control = Cancellation::default();
    let mut local = Budget {
        quota: LocalQuota,
        limits,
        cancellation: &local_control,
        statistics: SearchStatistics::default(),
    };
    native_failure(&mut local, &local_control);

    let shared_control = Cancellation::default();
    let shared = SharedBudget::new(limits, SearchStatistics::default());
    let mut worker = Budget {
        quota: shared.lease(&shared_control),
        limits,
        cancellation: &shared_control,
        statistics: SearchStatistics::default(),
    };
    native_failure(&mut worker, &shared_control);
    let mut joined = SearchStatistics::default();
    shared.record(&mut joined);
    assert_eq!(joined, local.statistics);
    assert_eq!(joined, worker.statistics);
}

fn refuse_zero(budget: &mut Budget<'_, impl Quota>, wanted: u64) {
    let before = budget.statistics;
    let mut calls = 0;
    let result = budget.with_reservation(wanted, |_| {
        calls += 1;
        ((), 0)
    });
    assert_eq!(result, Err(Incomplete::WorkLimit));
    assert_eq!(calls, 0);
    assert_eq!(budget.statistics, before);
}

#[test]
fn a_budget_refuses_zero_without_entering_its_kernel() {
    for (ceiling, wanted) in [(0, 7), (7, 0)] {
        let limits = SearchLimits {
            max_work: ceiling,
            max_decisions: 0,
        };
        let cancellation = Cancellation::default();
        let mut local = Budget {
            quota: LocalQuota,
            limits,
            cancellation: &cancellation,
            statistics: SearchStatistics::default(),
        };
        let shared = SharedBudget::new(limits, SearchStatistics::default());
        let mut worker = Budget {
            quota: shared.lease(&cancellation),
            limits,
            cancellation: &cancellation,
            statistics: SearchStatistics::default(),
        };
        if ceiling != 0 {
            local.tick().unwrap();
            worker.tick().unwrap();
        }
        refuse_zero(&mut local, wanted);
        refuse_zero(&mut worker, wanted);
        // A shared zero reservation still settles the prior incremental lease.
        let mut joined = SearchStatistics::default();
        shared.record(&mut joined);
        assert_eq!(joined, worker.statistics);
        assert_eq!(joined, local.statistics);
    }
}

#[test]
fn an_invalid_receipt_preserves_local_work_but_consumes_the_shared_grant() {
    let limits = SearchLimits {
        max_work: 9,
        max_decisions: 0,
    };
    let spent = SearchStatistics {
        work: 2,
        decisions: 0,
    };
    let cancellation = Cancellation::default();
    let mut local = Budget {
        quota: LocalQuota,
        limits,
        cancellation: &cancellation,
        statistics: spent,
    };
    let shared = SharedBudget::new(limits, spent);
    let mut worker = Budget {
        quota: shared.lease(&cancellation),
        limits,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    assert_eq!(
        local.with_reservation(20, |granted| {
            assert_eq!(granted, 7);
            ((), 8)
        }),
        Err(Incomplete::InvalidWitness)
    );
    assert_eq!(
        worker.with_reservation(20, |granted| {
            assert_eq!(granted, 7);
            ((), 8)
        }),
        Err(Incomplete::InvalidWitness)
    );
    assert_eq!(local.statistics, spent);
    assert_eq!(worker.statistics, SearchStatistics::default());
    let mut joined = SearchStatistics::default();
    shared.record(&mut joined);
    assert_eq!(joined.work, 9);
    assert_eq!(joined.decisions, 0);
}

#[test]
fn the_shared_receipt_settles_before_cumulative_counter_overflow() {
    let limits = SearchLimits {
        max_work: 10,
        max_decisions: 0,
    };
    let cancellation = Cancellation::default();
    let shared = SharedBudget::new(limits, SearchStatistics::default());
    let mut worker = Budget {
        quota: shared.lease(&cancellation),
        limits,
        cancellation: &cancellation,
        // Inject the receipt boundary independently of the shared authority:
        // shared grants ignore this worker-local count.
        statistics: SearchStatistics {
            work: u64::MAX,
            decisions: 0,
        },
    };
    assert_eq!(
        worker.with_reservation(7, |granted| {
            assert_eq!(granted, 7);
            ((), 3)
        }),
        Err(Incomplete::CounterOverflow)
    );
    assert_eq!(worker.statistics.work, u64::MAX);
    let mut joined = SearchStatistics::default();
    shared.record(&mut joined);
    assert_eq!(joined.work, 3);
    // Settlement returned the four unused permits before publishing failed.
    assert_eq!(worker.quota.take(7), Ok(()));
}

#[test]
fn cancellation_precedes_whole_kernel_acquisition() {
    let limits = SearchLimits {
        max_work: 0,
        max_decisions: 0,
    };
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut local = Budget {
        quota: LocalQuota,
        limits,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let shared = SharedBudget::new(limits, SearchStatistics::default());
    let mut worker = Budget {
        quota: shared.lease(&cancellation),
        limits,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    assert_eq!(
        local.with_reservation(7, |_| panic!("cancelled local kernel entered")),
        Err::<(), _>(Incomplete::Cancelled)
    );
    assert_eq!(
        worker.with_reservation(7, |_| panic!("cancelled shared kernel entered")),
        Err::<(), _>(Incomplete::Cancelled)
    );
    assert_eq!(local.statistics, SearchStatistics::default());
    assert_eq!(worker.statistics, local.statistics);
}
