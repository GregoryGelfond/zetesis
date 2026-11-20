mod counts;
mod quota;
mod receipt;
mod reservation;

use super::*;
use std::thread;
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
    while Instant::now() < deadline.at {
        thread::sleep(Duration::from_millis(1));
    }
    // Nothing observes the clock until a coarse boundary: the flag is clear.
    assert!(!deadline.expired.load(Ordering::Relaxed));
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

/// Start this thread's clock stride afresh, so its next coarse observation
/// reads the clock regardless of which tests ran on it before.
fn fresh_clock_stride() {
    UNTIL_CLOCK.with(|until| until.set(0));
}

fn clock_reads() -> u64 {
    CLOCK_READS.with(std::cell::Cell::get)
}

fn latched(cancellation: &Cancellation) -> bool {
    cancellation
        .deadline
        .as_ref()
        .unwrap()
        .deadline
        .expired
        .load(Ordering::Relaxed)
}

#[test]
fn the_inlined_poll_reads_flags_only_and_a_coarse_boundary_latches() {
    fresh_clock_stride();
    let cancellation =
        Cancellation::with_deadline(Instant::now() + Duration::from_millis(30)).unwrap();
    let polling = cancellation.polling();
    wait_for_expiry(&cancellation);
    let reads = clock_reads();
    for _ in 0..10_000 {
        assert_eq!(polling.poll(), Ok(()));
    }
    assert_eq!(
        clock_reads(),
        reads,
        "the inlined poll never reads the clock"
    );
    assert!(!latched(&cancellation));
    // A fresh test thread reads the clock at its first coarse observation.
    assert_eq!(polling.poll_deadline(), Err(Stop::Deadline));
    assert_eq!(clock_reads(), reads + 1);
    assert!(latched(&cancellation));
    // Once latched, the flag-only poll reports it for every view and clone.
    assert_eq!(polling.poll(), Err(Stop::Deadline));
    assert_eq!(cancellation.clone().polling().poll(), Err(Stop::Deadline));
}

#[test]
fn without_a_deadline_no_poll_reads_the_clock() {
    let cancellation = Cancellation::default();
    let polling = cancellation.polling();
    let reads = clock_reads();
    for _ in 0..1000 {
        assert_eq!(polling.poll(), Ok(()));
        assert_eq!(polling.poll_deadline(), Ok(()));
        assert_eq!(cancellation.poll(), Ok(()));
    }
    let metered = cancellation.metered(u64::MAX, |meter| {
        for _ in 0..10 * POLL_INTERVAL {
            meter.charge().unwrap();
        }
        meter.poll()
    });
    assert_eq!(metered.result, Ok(()));
    assert_eq!(clock_reads(), reads);
}

#[test]
fn an_unexpired_deadline_is_read_only_at_coarse_boundaries() {
    fresh_clock_stride();
    let cancellation =
        Cancellation::with_deadline(Instant::now() + Duration::from_hours(1)).unwrap();
    let reads = clock_reads();
    let metered = cancellation.metered(u64::MAX, |meter| {
        for _ in 0..u64::from(CLOCK_STRIDE) * POLL_INTERVAL {
            meter.charge().unwrap();
        }
        Ok::<_, Stop>(())
    });
    assert_eq!(metered.result, Ok(()));
    // One observation per POLL_INTERVAL boundary, one clock read per stride
    // of observations on this thread.
    assert_eq!(clock_reads(), reads + 1);
    assert!(!latched(&cancellation));
}

#[test]
fn an_owned_handle_poll_observes_an_expired_deadline() {
    fresh_clock_stride();
    let cancellation =
        Cancellation::with_deadline(Instant::now() + Duration::from_millis(30)).unwrap();
    let polling = cancellation.polling();
    wait_for_expiry(&cancellation);
    assert_eq!(polling.poll(), Ok(()));
    assert_eq!(cancellation.poll(), Err(Stop::Deadline));
    assert_eq!(polling.poll(), Err(Stop::Deadline));
}

#[test]
fn a_local_meter_refill_boundary_observes_an_expired_deadline() {
    fresh_clock_stride();
    let cancellation =
        Cancellation::with_deadline(Instant::now() + Duration::from_millis(30)).unwrap();
    wait_for_expiry(&cancellation);
    // Between boundaries a charge reads the flags only and proceeds.
    assert_eq!(charge_from(&cancellation, 7, u64::MAX), (Ok(()), 8));
    assert!(!latched(&cancellation));
    assert_eq!(
        charge_from(&cancellation, POLL_INTERVAL, u64::MAX),
        (Err(ChargeError::Stopped(Stop::Deadline)), POLL_INTERVAL)
    );
    assert!(latched(&cancellation));
}

#[test]
fn a_local_meter_ceiling_observes_an_expired_deadline() {
    fresh_clock_stride();
    let cancellation =
        Cancellation::with_deadline(Instant::now() + Duration::from_millis(30)).unwrap();
    wait_for_expiry(&cancellation);
    // Seven is not a boundary; the refusal at the ceiling observes the deadline.
    assert_eq!(
        charge_from(&cancellation, 7, 7),
        (Err(ChargeError::Stopped(Stop::Deadline)), 7)
    );
}

/// An account that, like the solver's budgets, polls its owned handle at
/// every refill.
struct PollingAccount<'a>(&'a Cancellation);

impl Quota for PollingAccount<'_> {
    type Error = Stop;

    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        self.0.poll()?;
        Ok(wanted)
    }

    fn refund(&mut self, _: u64) {}
}

#[test]
fn a_reserved_meter_refill_observes_an_expired_deadline() {
    let cancellation =
        Cancellation::with_deadline(Instant::now() + Duration::from_millis(30)).unwrap();
    let mut account = PollingAccount(&cancellation);
    let mut meter = Meter::reserved(&mut account);
    // The first refill observes an unexpired deadline on this thread.
    meter.tick().unwrap();
    wait_for_expiry(&cancellation);
    let stopped = loop {
        if let Err(failure) = meter.tick() {
            break failure;
        }
        assert!(
            meter.charged() <= u64::from(CLOCK_STRIDE) * POLL_INTERVAL,
            "a refill must observe the deadline within a clock stride"
        );
    };
    assert_eq!(stopped.into_stop(), Stop::Deadline);
    // Only a refill stops: the meter ran every permit it was granted.
    assert_eq!(meter.charged() % POLL_INTERVAL, 0);
    assert!(latched(&cancellation));
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
fn a_deadline_starts_no_thread_and_latches_at_the_first_late_poll() {
    // The deadline is observed by polling, so --threads bounds every thread.
    let cancellation =
        Cancellation::with_deadline(Instant::now() + Duration::from_hours(1)).unwrap();
    assert_eq!(cancellation.poll(), Ok(()));
    let deadline = &cancellation.deadline.as_ref().unwrap().deadline;
    assert!(!deadline.expired.load(Ordering::Relaxed));
    let past = Cancellation::with_deadline(Instant::now()).unwrap();
    assert!(
        past.deadline
            .as_ref()
            .unwrap()
            .deadline
            .expired
            .load(Ordering::Relaxed)
    );
    assert_eq!(past.poll(), Err(Stop::Deadline));
}
