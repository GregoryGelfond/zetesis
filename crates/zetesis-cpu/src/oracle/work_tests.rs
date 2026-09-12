//! Batched bookkeeping preserves the scalar quota transition.

use std::time::Instant;

use super::Work;
use crate::{Control, Stop};

// Independent pre-batching transition: each bookkeeping unit polls control,
// refuses an exhausted quota, and otherwise records one completed unit.
fn scalar_charge(
    control: &Control,
    limit: u64,
    recorded: &mut u64,
    amount: usize,
) -> Result<(), Stop> {
    for _ in 0..amount {
        control.poll()?;
        if *recorded >= limit {
            return Err(Stop::WorkLimit);
        }
        *recorded += 1;
    }
    Ok(())
}

#[test]
fn batched_charges_preserve_scalar_quota_transitions() {
    let control = Control::default();
    for limit in 0..=24 {
        for prior in 0..=limit {
            for amount in 0..=32 {
                let mut work = Work::source(&control, limit);
                work.statistics.work = prior;
                let mut expected = prior;
                let result = scalar_charge(&control, limit, &mut expected, amount);
                assert_eq!(work.charge(amount), result);
                assert_eq!(work.statistics.work, expected);
            }
        }
    }
}

#[test]
fn zero_charge_does_not_poll_cancelled_control() {
    let control = Control::with_deadline(Instant::now());
    control.cancel();
    let mut work = Work::source(&control, 0);
    assert_eq!(work.charge(0), Ok(()));
    assert_eq!(work.statistics.work, 0);
}

#[test]
fn cancellation_has_first_refusal_priority() {
    let control = Control::with_deadline(Instant::now());
    let mut work = Work::source(&control, 3);
    work.statistics.work = 3;
    control.cancel();
    assert_eq!(work.charge(usize::MAX), Err(Stop::Cancelled));
    assert_eq!(work.statistics.work, 3);
}

#[test]
fn deadline_precedes_exhausted_quota() {
    let control = Control::with_deadline(Instant::now());
    let mut work = Work::source(&control, 3);
    work.statistics.work = 3;
    assert_eq!(work.charge(1), Err(Stop::Deadline));
    assert_eq!(work.statistics.work, 3);
}

#[test]
fn cancellation_between_charges_preserves_prior_work() {
    let control = Control::default();
    let mut work = Work::source(&control, 10);
    work.charge(3).unwrap();
    control.cancel();
    assert_eq!(work.charge(4), Err(Stop::Cancelled));
    assert_eq!(work.statistics.work, 3);
}

#[test]
fn maximal_payload_does_not_require_a_per_unit_loop() {
    let control = Control::default();
    let mut work = Work::source(&control, u64::MAX);
    // This is bookkeeping only: no payload allocation or traversal occurs.
    work.charge(usize::MAX).unwrap();
    assert_eq!(work.statistics.work, u64::try_from(usize::MAX).unwrap());
}

#[test]
fn exhausted_counter_never_overflows() {
    let control = Control::default();
    let mut work = Work::source(&control, u64::MAX);
    work.statistics.work = u64::MAX - 2;
    work.charge(2).unwrap();
    assert_eq!(work.charge(1), Err(Stop::WorkLimit));
    assert_eq!(work.statistics.work, u64::MAX);
}

#[test]
fn oversized_charge_records_only_the_available_prefix() {
    let control = Control::default();
    let mut work = Work::source(&control, u64::MAX);
    work.statistics.work = u64::MAX - 2;
    assert_eq!(work.charge(usize::MAX), Err(Stop::WorkLimit));
    assert_eq!(work.statistics.work, u64::MAX);
}
