//! Permit conservation, exact exhaustion and cleanup use the production owner.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::*;

fn budget(work: u64) -> SharedBudget {
    SharedBudget::new(
        SearchLimits {
            max_work: work,
            max_decisions: 0,
        },
        SearchStatistics::default(),
    )
}

fn state(shared: &SharedBudget, expected: (u64, u64, u64)) {
    let work = shared.lock();
    assert_eq!((work.spent, work.available, work.outstanding), expected);
    assert_eq!(
        work.spent + work.available + work.outstanding,
        shared.limits.max_work
    );
}

#[test]
fn exhausted_available_permits_wait_for_outstanding_grants() {
    let mut work = Work {
        spent: 3,
        available: 1,
        outstanding: 0,
    };
    assert_eq!(work.grant(), Ok(Some(1)));
    assert_eq!(work.grant(), Ok(None));
    assert_eq!((work.spent, work.available, work.outstanding), (3, 0, 1));
    work.outstanding = 0;
    work.spent = 4;
    assert_eq!(work.grant(), Err(Incomplete::WorkLimit));
}

#[test]
fn a_charge_commits_available_permits_and_is_refused_beyond_them() {
    let shared = budget(WORK_QUANTUM + 3);
    let cancellation = Cancellation::default();
    let lease = shared.lease(&cancellation);
    lease.tick().unwrap();
    state(&shared, (0, 3, WORK_QUANTUM));
    shared.charge(2).unwrap();
    state(&shared, (2, 1, WORK_QUANTUM));
    // The lease's unused permits could cover the charge, but they are not
    // waited for.
    assert_eq!(shared.charge(2), Err(Incomplete::WorkLimit));
    state(&shared, (2, 1, WORK_QUANTUM));
    drop(lease);
    state(&shared, (3, WORK_QUANTUM, 0));
}

#[test]
fn each_grant_conserves_used_available_and_outstanding_permits() {
    let shared = budget(2 * WORK_QUANTUM + 3);
    let cancellation = Cancellation::default();
    let first = shared.lease(&cancellation);
    let second = shared.lease(&cancellation);
    first.tick().unwrap();
    state(&shared, (0, WORK_QUANTUM + 3, WORK_QUANTUM));
    second.tick().unwrap();
    state(&shared, (0, 3, 2 * WORK_QUANTUM));
    for _ in 1..WORK_QUANTUM {
        first.tick().unwrap();
    }
    state(&shared, (0, 3, 2 * WORK_QUANTUM));
    first.tick().unwrap();
    state(&shared, (WORK_QUANTUM, 0, WORK_QUANTUM + 3));
    drop(second);
    state(&shared, (WORK_QUANTUM + 1, WORK_QUANTUM - 1, 3));
    drop(first);
    state(&shared, (WORK_QUANTUM + 2, WORK_QUANTUM + 1, 0));
}

#[test]
fn cancelled_renewal_commits_its_consumed_grant() {
    let shared = budget(2 * WORK_QUANTUM);
    let cancellation = Cancellation::default();
    let lease = shared.lease(&cancellation);
    lease.take(WORK_QUANTUM).unwrap();
    cancellation.cancel();
    assert_eq!(lease.tick(), Err(Incomplete::Cancelled));
    // Inspect before Drop: the failed renewal itself must publish the used
    // prefix, and cannot retain a replacement or commit the old grant twice.
    state(&shared, (WORK_QUANTUM, WORK_QUANTUM, 0));
    drop(lease);
    state(&shared, (WORK_QUANTUM, WORK_QUANTUM, 0));
}

#[test]
fn final_consumed_grant_wakes_exhaustion_waiters() {
    let shared = budget(WORK_QUANTUM);
    let cancellation =
        Cancellation::with_deadline(Instant::now() + Duration::from_secs(5)).unwrap();
    let lease = shared.lease(&cancellation);
    lease.take(WORK_QUANTUM).unwrap();
    thread::scope(|scope| {
        let (started, starting) = mpsc::sync_channel(1);
        let shared = &shared;
        let waiter = scope.spawn(move || {
            let mut work = shared.lock();
            started.send(()).unwrap();
            // The handshake holds the ledger lock until this wait releases it.
            // Use a long wait rather than the production polling interval so
            // polling cannot hide a missing final-settlement notification.
            let deadline = Instant::now() + Duration::from_secs(2);
            while work.outstanding != 0 {
                let remaining = deadline.saturating_duration_since(Instant::now());
                let (returned, timeout) = shared.returned.wait_timeout(work, remaining).unwrap();
                work = returned;
                // Check the actual wait result before its predicate: the
                // ledger may become exhausted without anyone notifying us.
                assert!(!timeout.timed_out(), "exhaustion was not notified");
            }
            assert_eq!(work.grant(), Err(Incomplete::WorkLimit));
        });
        starting.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(lease.tick(), Err(Incomplete::WorkLimit));
        waiter.join().unwrap();
    });
    state(&shared, (WORK_QUANTUM, 0, 0));
}

#[test]
fn a_settled_lease_refills_on_its_next_use() {
    // A region walker keeps one lease across its regions and settles it before
    // waiting: settling commits the consumed part and returns the rest, and the
    // same lease takes a fresh grant when it is next used.
    let shared = budget(2 * WORK_QUANTUM);
    let cancellation = Cancellation::default();
    let lease = shared.lease(&cancellation);
    lease.take(5).unwrap();
    lease.settle();
    state(&shared, (5, 2 * WORK_QUANTUM - 5, 0));
    lease.tick().unwrap();
    state(&shared, (5, WORK_QUANTUM - 5, WORK_QUANTUM));
    drop(lease);
    state(&shared, (6, 2 * WORK_QUANTUM - 6, 0));
}

#[test]
fn bulk_consumption_preserves_its_unused_remainder() {
    let shared = budget(WORK_QUANTUM + 7);
    let cancellation = Cancellation::default();
    let lease = shared.lease(&cancellation);
    lease.take(WORK_QUANTUM + 3).unwrap();
    state(&shared, (WORK_QUANTUM, 0, 7));
    drop(lease);
    state(&shared, (WORK_QUANTUM + 3, 4, 0));
}

#[test]
fn failed_bulk_consumption_commits_its_admitted_prefix() {
    let shared = budget(WORK_QUANTUM + 3);
    let cancellation =
        Cancellation::with_deadline(Instant::now() + Duration::from_secs(5)).unwrap();
    let lease = shared.lease(&cancellation);
    assert_eq!(lease.take(WORK_QUANTUM + 4), Err(Incomplete::WorkLimit));
    state(&shared, (WORK_QUANTUM + 3, 0, 0));
    drop(lease);
    state(&shared, (WORK_QUANTUM + 3, 0, 0));
}

#[test]
fn joined_work_is_exact_at_every_small_inclusive_ceiling() {
    // Keep the exhaustive population independent of the production grant size.
    // Grant-size boundaries have their own cases below.
    const SMALL_CEILING: u64 = 129;
    for ceiling in 0..=SMALL_CEILING {
        assert_joined_work(ceiling);
    }
}

#[test]
fn joined_work_is_exact_at_grant_boundaries() {
    for ceiling in [
        WORK_QUANTUM - 1,
        WORK_QUANTUM,
        WORK_QUANTUM + 1,
        2 * WORK_QUANTUM - 1,
        2 * WORK_QUANTUM,
        2 * WORK_QUANTUM + 1,
    ] {
        assert_joined_work(ceiling);
    }
}

fn assert_joined_work(ceiling: u64) {
    let shared = budget(ceiling);
    let cancellation = Cancellation::default();
    thread::scope(|scope| {
        let mut jobs = Vec::new();
        for _ in 0..4 {
            let shared = &shared;
            let cancellation = &cancellation;
            jobs.push(scope.spawn(move || {
                let lease = shared.lease(cancellation);
                let mut consumed = 0;
                while lease.tick().is_ok() {
                    consumed += 1;
                }
                consumed
            }));
        }
        assert_eq!(
            jobs.into_iter().map(|job| job.join().unwrap()).sum::<u64>(),
            ceiling
        );
    });
    let mut recorded = SearchStatistics::default();
    shared.record(&mut recorded);
    assert_eq!(recorded.work, ceiling);
    assert_eq!(recorded.decisions, 0);
    state(&shared, (ceiling, 0, 0));
}

#[test]
fn returned_unused_permits_allow_a_waiting_query_to_continue() {
    let shared = budget(2);
    let cancellation = Cancellation::default();
    let first = shared.lease(&cancellation);
    first.tick().unwrap();
    state(&shared, (0, 0, 2));
    thread::scope(|scope| {
        let (started, starting) = mpsc::sync_channel(1);
        let (done, result) = mpsc::sync_channel(1);
        let shared = &shared;
        let cancellation = &cancellation;
        scope.spawn(move || {
            let next = shared.lease(cancellation);
            started.send(()).unwrap();
            done.send(next.tick()).unwrap();
        });
        starting.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(result.try_recv(), Err(mpsc::TryRecvError::Empty));
        drop(first);
        assert_eq!(result.recv_timeout(Duration::from_secs(2)).unwrap(), Ok(()));
    });
    state(&shared, (2, 0, 0));
}

#[test]
fn waiting_for_permits_observes_cancellation_and_deadline() {
    for deadline in [false, true] {
        let shared = budget(2);
        let owner_control = Cancellation::default();
        let owner = shared.lease(&owner_control);
        owner.tick().unwrap();
        let cancellation = if deadline {
            Cancellation::with_deadline(Instant::now() + Duration::from_millis(20)).unwrap()
        } else {
            Cancellation::default()
        };
        thread::scope(|scope| {
            let (done, result) = mpsc::sync_channel(1);
            let shared = &shared;
            let waiter_control = &cancellation;
            scope.spawn(move || {
                let waiter = shared.lease(waiter_control);
                done.send(waiter.tick()).unwrap();
            });
            if !deadline {
                cancellation.cancel();
            }
            assert_eq!(
                result.recv_timeout(Duration::from_secs(2)).unwrap(),
                Err(if deadline {
                    Incomplete::Deadline
                } else {
                    Incomplete::Cancelled
                }),
            );
        });
        drop(owner);
        state(&shared, (1, 1, 0));
    }
}

#[test]
fn every_typed_query_error_returns_unspent_grants() {
    let shared = budget(WORK_QUANTUM);
    let cancellation = Cancellation::default();
    for error in [
        Incomplete::Allocation,
        Incomplete::InvalidWitness,
        Incomplete::DecisionLimit,
        Incomplete::Cancelled,
        Incomplete::Deadline,
    ] {
        let result = (|| {
            let lease = shared.lease(&cancellation);
            lease.tick()?;
            Err::<(), _>(error)
        })();
        assert_eq!(result, Err(error));
    }
    state(&shared, (5, WORK_QUANTUM - 5, 0));
    let mut recorded = SearchStatistics::default();
    shared.record(&mut recorded);
    assert_eq!(recorded.work, 5);
}

#[test]
fn allocation_failure_and_unwind_return_unused_permits() {
    let shared = budget(WORK_QUANTUM);
    let cancellation = Cancellation::default();
    let result = (|| {
        let lease = shared.lease(&cancellation);
        lease.tick()?;
        Vec::<u64>::new()
            .try_reserve_exact(usize::MAX)
            .map_err(|_| Incomplete::Allocation)
    })();
    assert_eq!(result, Err(Incomplete::Allocation));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let lease = shared.lease(&cancellation);
            lease.tick().unwrap();
            panic!("query failed outside the permit lock");
        }))
        .is_err()
    );
    state(&shared, (2, WORK_QUANTUM - 2, 0));
    assert!(!shared.work.is_poisoned());
}

#[test]
fn last_representable_reservation_never_wraps_or_spends_a_different_quota() {
    let mut spent = SearchStatistics {
        work: u64::MAX - 1,
        decisions: 7,
        ..SearchStatistics::default()
    };
    let shared = SharedBudget::new(
        SearchLimits {
            max_work: u64::MAX,
            max_decisions: 7,
        },
        spent,
    );
    let cancellation = Cancellation::default();
    let lease = shared.lease(&cancellation);
    assert_eq!(lease.tick(), Ok(()));
    assert_eq!(lease.tick(), Err(Incomplete::WorkLimit));
    assert_eq!(lease.decide(), Err(Incomplete::DecisionLimit));
    drop(lease);
    shared.record(&mut spent);
    assert_eq!(spent.work, u64::MAX);
    assert_eq!(spent.decisions, 7);
}

#[test]
fn an_accounted_attempt_refunds_its_unused_allowance() {
    let shared = budget(100);
    let cancellation = Cancellation::default();
    let mut lease = shared.lease(&cancellation);
    lease.tick().unwrap();
    let reservation = lease.reserve(20).unwrap();
    assert_eq!(reservation.allowance(), 20);
    state(&shared, (1, 79, 20));
    // The same receipt is settled after success or a typed kernel refusal.
    reservation.finish(3).unwrap();
    state(&shared, (4, 96, 0));
}

#[test]
fn a_kernel_can_execute_only_the_reserved_remainder() {
    let shared = budget(7);
    let cancellation = Cancellation::default();
    let mut lease = shared.lease(&cancellation);
    let reservation = lease.reserve(20).unwrap();
    assert_eq!(reservation.allowance(), 7);
    state(&shared, (0, 0, 7));
    reservation.finish(7).unwrap();
    state(&shared, (7, 0, 0));
    assert_eq!(lease.tick(), Err(Incomplete::WorkLimit));
}

#[test]
fn a_kernel_reservation_returns_its_lease_before_waiting() {
    let shared = budget(10);
    let cancellation =
        Cancellation::with_deadline(Instant::now() + Duration::from_secs(2)).unwrap();
    let mut first = shared.lease(&cancellation);
    let reservation = first.reserve(8).unwrap();
    thread::scope(|scope| {
        let (started, starting) = mpsc::sync_channel(1);
        let (done, result) = mpsc::sync_channel(1);
        let shared = &shared;
        let cancellation = &cancellation;
        scope.spawn(move || {
            let mut lease = shared.lease(cancellation);
            lease.tick().unwrap();
            started.send(()).unwrap();
            let reservation = lease.reserve(4).unwrap();
            assert_eq!(reservation.allowance(), 4);
            reservation.finish(2).unwrap();
            done.send(()).unwrap();
        });
        starting.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(result.try_recv(), Err(mpsc::TryRecvError::Empty));
        reservation.finish(3).unwrap();
        result.recv_timeout(Duration::from_secs(2)).unwrap();
    });
    state(&shared, (6, 4, 0));
}

#[test]
fn an_unwinding_kernel_consumes_its_reserved_allowance() {
    let shared = budget(10);
    let cancellation = Cancellation::default();
    let mut lease = shared.lease(&cancellation);
    let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _reservation = lease.reserve(8).unwrap();
        panic!("kernel unwound before returning its work receipt");
    }));
    assert!(stopped.is_err());
    state(&shared, (8, 2, 0));
}
