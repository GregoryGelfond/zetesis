//! Batched bookkeeping preserves the scalar quota transition.

use std::time::Instant;

use super::Work;
use crate::{Cancellation, Stop};

// Independent pre-batching transition: each bookkeeping unit polls control,
// refuses an exhausted quota, and otherwise records one completed unit.
fn scalar_charge(
    cancellation: &Cancellation,
    limit: u64,
    recorded: &mut u64,
    amount: usize,
) -> Result<(), Stop> {
    for _ in 0..amount {
        cancellation.poll()?;
        if *recorded >= limit {
            return Err(Stop::WorkLimit);
        }
        *recorded += 1;
    }
    Ok(())
}

#[test]
fn batched_charges_preserve_scalar_quota_transitions() {
    let cancellation = Cancellation::default();
    for limit in 0..=24 {
        for prior in 0..=limit {
            for amount in 0..=32 {
                let mut work = Work::source(&cancellation, limit);
                work.statistics.work = prior;
                let mut expected = prior;
                let result = scalar_charge(&cancellation, limit, &mut expected, amount);
                assert_eq!(work.charge(amount), result);
                assert_eq!(work.statistics.work, expected);
            }
        }
    }
}

#[test]
fn zero_charge_does_not_poll_cancelled_control() {
    let cancellation = Cancellation::with_deadline(Instant::now()).unwrap();
    cancellation.cancel();
    let mut work = Work::source(&cancellation, 0);
    assert_eq!(work.charge(0), Ok(()));
    assert_eq!(work.statistics.work, 0);
}

#[test]
fn cancellation_has_first_refusal_priority() {
    let cancellation = Cancellation::with_deadline(Instant::now()).unwrap();
    let mut work = Work::source(&cancellation, 3);
    work.statistics.work = 3;
    cancellation.cancel();
    assert_eq!(work.charge(usize::MAX), Err(Stop::Cancelled));
    assert_eq!(work.statistics.work, 3);
}

#[test]
fn deadline_precedes_exhausted_quota() {
    let cancellation = Cancellation::with_deadline(Instant::now()).unwrap();
    let mut work = Work::source(&cancellation, 3);
    work.statistics.work = 3;
    assert_eq!(work.charge(1), Err(Stop::Deadline));
    assert_eq!(work.statistics.work, 3);
}

#[test]
fn cancellation_between_charges_preserves_prior_work() {
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, 10);
    work.charge(3).unwrap();
    cancellation.cancel();
    assert_eq!(work.charge(4), Err(Stop::Cancelled));
    assert_eq!(work.statistics.work, 3);
}

#[test]
fn maximal_payload_does_not_require_a_per_unit_loop() {
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, u64::MAX);
    // This is bookkeeping only: no payload allocation or traversal occurs.
    work.charge(usize::MAX).unwrap();
    assert_eq!(work.statistics.work, u64::try_from(usize::MAX).unwrap());
}

#[test]
fn exhausted_counter_never_overflows() {
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, u64::MAX);
    work.statistics.work = u64::MAX - 2;
    work.charge(2).unwrap();
    assert_eq!(work.charge(1), Err(Stop::WorkLimit));
    assert_eq!(work.statistics.work, u64::MAX);
}

#[test]
fn oversized_charge_records_only_the_available_prefix() {
    let cancellation = Cancellation::default();
    let mut work = Work::source(&cancellation, u64::MAX);
    work.statistics.work = u64::MAX - 2;
    assert_eq!(work.charge(usize::MAX), Err(Stop::WorkLimit));
    assert_eq!(work.statistics.work, u64::MAX);
}
