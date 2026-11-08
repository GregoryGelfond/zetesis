mod counts;
mod quota;
mod receipt;
mod reservation;

use super::*;
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn charge_ranges_observe_exactly_the_contained_boundaries() {
    for charged in 0..1024 {
        for amount in 0..=1024 {
            let boundary =
                u128::from(charged).div_ceil(u128::from(POLL_INTERVAL)) * u128::from(POLL_INTERVAL);
            assert_eq!(
                poll_due(charged, amount),
                boundary < u128::from(charged) + u128::from(amount),
                "charged={charged}, amount={amount}"
            );
        }
    }
}

#[test]
fn boundary_decisions_do_not_overflow_at_the_counter_limit() {
    for charged in [0, 1, u64::MAX - 256, u64::MAX - 1, u64::MAX] {
        for amount in [0, 1, 255, 256, 257, u64::MAX] {
            let boundary =
                u128::from(charged).div_ceil(u128::from(POLL_INTERVAL)) * u128::from(POLL_INTERVAL);
            assert_eq!(
                poll_due(charged, amount),
                boundary < u128::from(charged) + u128::from(amount),
                "charged={charged}, amount={amount}"
            );
        }
    }
}

fn wait_for_expiry(cancellation: &Cancellation) {
    let deadline = &cancellation.deadline.as_ref().unwrap().deadline;
    let timeout = Instant::now() + Duration::from_secs(5);
    // Observe the flag directly: a boundary poll must not cause the expiry.
    // The bound detects a stalled timer, not a promised scheduling latency.
    while !deadline.expired.load(Ordering::Relaxed) {
        assert!(Instant::now() < timeout, "deadline timer did not expire");
        thread::sleep(Duration::from_millis(1));
    }
}

/// Charge once through a meter that has admitted `charged` operations under
/// `limit`; returns the result and the count after it.
fn charge_from(
    cancellation: &Cancellation,
    charged: u64,
    limit: u64,
) -> (Result<(), ChargeError>, u64) {
    let mut meter = Meter {
        count: Isolated {
            _before: 0,
            value: charged,
            _after: 0,
        },
        limit,
        poll: Some(cancellation.polling()),
        quota: None,
    };
    let result = meter.charge();
    (result, meter.charged())
}

#[test]
fn charges_observe_a_stop_only_at_metered_boundaries() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    for charged in 0..=3 * POLL_INTERVAL {
        let expected = if charged.is_multiple_of(POLL_INTERVAL) {
            (Err(ChargeError::Stopped(Stop::Cancelled)), charged)
        } else {
            (Ok(()), charged + 1)
        };
        assert_eq!(
            charge_from(&cancellation, charged, u64::MAX),
            expected,
            "{charged}"
        );
    }
}

#[test]
fn a_requested_stop_precedes_the_work_limit_between_boundaries() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    // Seven is not a boundary, so only the precedence observation sees the stop.
    assert_eq!(
        charge_from(&cancellation, 7, 7),
        (Err(ChargeError::Stopped(Stop::Cancelled)), 7)
    );
}

#[test]
fn the_work_limit_refuses_without_counting() {
    let cancellation = Cancellation::default();
    assert_eq!(
        charge_from(&cancellation, 7, 7),
        (Err(ChargeError::WorkLimit), 7)
    );
    assert_eq!(
        charge_from(&cancellation, 0, 0),
        (Err(ChargeError::WorkLimit), 0)
    );
}

#[test]
fn an_admitted_charge_counts_its_operation() {
    let cancellation = Cancellation::default();
    assert_eq!(charge_from(&cancellation, 6, 7), (Ok(()), 7));
}

#[test]
fn nested_work_counts_within_the_remaining_allowance() {
    let cancellation = Cancellation::default();
    let metered = cancellation.metered(10, |meter| {
        for _ in 0..3 {
            meter.charge().unwrap();
        }
        assert_eq!(meter.remaining(), 7);
        meter.absorb(7).map_err(ChargeError::into_stop)
    });
    assert_eq!(metered.result, Ok(()));
    assert_eq!(metered.charged, 10);
}

#[test]
fn an_overrun_absorb_counts_up_to_the_limit() {
    // The refusal is returned as a value, so the exit leaves it alone.
    let cancellation = Cancellation::default();
    let metered = cancellation.metered(10, |meter| {
        for _ in 0..3 {
            meter.charge().unwrap();
        }
        Ok::<_, Stop>((meter.absorb(8), meter.remaining()))
    });
    assert_eq!(metered.result, Ok((Err(ChargeError::WorkLimit), 0)));
    assert_eq!(metered.charged, 10);
}

#[test]
fn an_overrun_absorb_reports_a_requested_stop() {
    // The refusal is returned as a value, so the exit leaves it alone.
    let cancellation = Cancellation::default();
    let metered = cancellation.metered(10, |meter| {
        cancellation.cancel();
        Ok::<_, Stop>(meter.absorb(11))
    });
    assert_eq!(
        metered.result,
        Ok(Err(ChargeError::Stopped(Stop::Cancelled)))
    );
    assert_eq!(metered.charged, 10);
}

/// The settled outcome of an operation that returns `outcome`, with a stop
/// requested inside it when `stop` holds.
fn metered_outcome(stop: bool, outcome: Result<u8, Stop>) -> Result<u8, Stop> {
    let cancellation = Cancellation::default();
    cancellation
        .metered(u64::MAX, |_| {
            if stop {
                cancellation.cancel();
            }
            outcome
        })
        .result
}

#[test]
fn a_requested_stop_settles_a_resource_refusal() {
    assert_eq!(
        metered_outcome(false, Err(Stop::StorageLimit)),
        Err(Stop::StorageLimit)
    );
    assert_eq!(
        metered_outcome(true, Err(Stop::StorageLimit)),
        Err(Stop::Cancelled)
    );
}

#[test]
fn settling_leaves_results_and_shape_errors_alone() {
    assert_eq!(metered_outcome(true, Ok(3)), Ok(3));
    for shape in [Stop::WrongProgram, Stop::InvalidProgram] {
        assert_eq!(metered_outcome(true, Err(shape)), Err(shape));
    }
}

#[test]
fn a_metered_operation_reports_its_work_on_every_outcome() {
    let cancellation = Cancellation::default();
    let metered = cancellation.metered(5, |meter| {
        while meter.charge().is_ok() {}
        Err::<(), _>(Stop::WorkLimit)
    });
    assert_eq!(metered.result, Err(Stop::WorkLimit));
    assert_eq!(metered.charged, 5);
}

#[test]
fn the_count_has_padding_on_both_sides() {
    assert_eq!(std::mem::offset_of!(Isolated, value), size_of::<u64>());
    assert_eq!(size_of::<Isolated>(), 3 * size_of::<u64>());
}

#[test]
fn stops_classify_resource_refusals() {
    for stop in [
        Stop::WorkLimit,
        Stop::RoundLimit,
        Stop::StorageLimit,
        Stop::DerivedAtomLimit,
        Stop::CandidateLimit,
        Stop::CarrierLimit,
        Stop::Allocation,
    ] {
        assert!(stop.is_resource_refusal(), "{stop:?}");
    }
    for stop in [
        Stop::Cancelled,
        Stop::Deadline,
        Stop::WrongProgram,
        Stop::InvalidProgram,
    ] {
        assert!(!stop.is_resource_refusal(), "{stop:?}");
    }
}

#[test]
fn a_deadline_expires_without_polling() {
    let cancellation =
        Cancellation::with_deadline(Instant::now() + Duration::from_millis(30)).unwrap();
    let polling = cancellation.polling();
    wait_for_expiry(&cancellation);
    assert_eq!(polling.poll(), Err(Stop::Deadline));
    assert_eq!(cancellation.poll(), Err(Stop::Deadline));
}

#[test]
fn clones_share_the_deadline() {
    let cancellation =
        Cancellation::with_deadline(Instant::now() + Duration::from_millis(30)).unwrap();
    let clone = cancellation.clone();
    assert!(Arc::ptr_eq(
        cancellation.deadline.as_ref().unwrap(),
        clone.deadline.as_ref().unwrap(),
    ));
    wait_for_expiry(&cancellation);
    assert_eq!(clone.poll(), Err(Stop::Deadline));
    assert_eq!(cancellation.poll(), Err(Stop::Deadline));
}

#[test]
fn dropping_the_last_clone_retires_the_timer() {
    // A second waiter on the same deadline stands in for the timer thread,
    // whose handle is not retained: retirement wakes every waiter.
    let owner = DeadlineOwner::arm(Instant::now() + Duration::from_hours(1)).unwrap();
    let deadline = Arc::clone(&owner.deadline);
    let (retired, observer) = mpsc::channel();
    thread::spawn(move || {
        deadline.wait();
        retired.send(()).unwrap();
    });
    assert!(observer.recv_timeout(Duration::from_millis(100)).is_err());
    drop(owner);
    assert!(observer.recv_timeout(Duration::from_secs(5)).is_ok());
}

#[test]
fn retirement_does_not_expire_the_deadline() {
    let owner = DeadlineOwner::arm(Instant::now() + Duration::from_hours(1)).unwrap();
    let deadline = Arc::clone(&owner.deadline);
    drop(owner);
    deadline.wait();
    assert!(!deadline.expired.load(Ordering::Relaxed));
}
