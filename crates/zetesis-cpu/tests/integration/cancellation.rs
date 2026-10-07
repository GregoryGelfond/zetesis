//! A deadline is observed like cancellation: a flag read at each boundary.

use std::time::{Duration, Instant};

use zetesis_cpu::{Cancellation, CancellationSlot, Stop};

#[test]
fn an_expired_deadline_stops_the_first_poll() {
    let cancellation = Cancellation::with_deadline(Instant::now()).unwrap();
    assert_eq!(cancellation.poll(), Err(Stop::Deadline));
}

#[test]
fn clones_share_the_cancellation() {
    let cancellation = Cancellation::default();
    let clone = cancellation.clone();
    assert_eq!(cancellation.poll(), Ok(()));
    clone.cancel();
    assert_eq!(cancellation.poll(), Err(Stop::Cancelled));
}

#[test]
fn cancellation_precedes_an_expired_deadline() {
    let cancellation = Cancellation::with_deadline(Instant::now()).unwrap();
    cancellation.cancel();
    assert_eq!(cancellation.poll(), Err(Stop::Cancelled));
}

#[test]
fn a_distant_deadline_never_interrupts_a_short_computation() {
    let cancellation =
        Cancellation::with_deadline(Instant::now() + Duration::from_hours(1)).unwrap();
    for _ in 0..1_000_000 {
        assert_eq!(cancellation.poll(), Ok(()));
    }
}

#[test]
fn borrowed_polls_observe_later_token_cancellation() {
    for cancellation in [
        Cancellation::default(),
        Cancellation::with_deadline(Instant::now() + Duration::from_hours(1)).unwrap(),
    ] {
        let polling = cancellation.polling();
        assert_eq!(polling.poll(), Ok(()));
        cancellation.clone().cancel();
        assert_eq!(polling.poll(), Err(Stop::Cancelled));
    }
}

#[test]
fn borrowed_polls_observe_later_slot_cancellation() {
    for deadline in [None, Some(Instant::now() + Duration::from_hours(1))] {
        let slot = CancellationSlot::default();
        let run = slot.open(deadline).unwrap();
        let polling = run.cancellation().polling();
        assert_eq!(polling.poll(), Ok(()));
        slot.cancel();
        assert_eq!(polling.poll(), Err(Stop::Cancelled));
    }
}

#[test]
fn borrowed_polls_observe_run_retirement() {
    for deadline in [None, Some(Instant::now() + Duration::from_hours(1))] {
        let slot = CancellationSlot::default();
        let run = slot.open(deadline).unwrap();
        let token = run.cancellation().clone();
        let polling = token.polling();
        assert_eq!(polling.poll(), Ok(()));
        drop(run);
        assert_eq!(polling.poll(), Err(Stop::Cancelled));
    }
}

#[test]
fn borrowed_polls_observe_run_replacement() {
    for deadline in [None, Some(Instant::now() + Duration::from_hours(1))] {
        let slot = CancellationSlot::default();
        let run = slot.open(deadline).unwrap();
        let polling = run.cancellation().polling();
        assert_eq!(polling.poll(), Ok(()));
        let replacement = slot.open(None).unwrap();
        assert_eq!(polling.poll(), Err(Stop::Cancelled));
        assert_eq!(replacement.cancellation().polling().poll(), Ok(()));
    }
}

#[test]
fn borrowed_polls_prioritize_cancellation_over_expiry() {
    let token = Cancellation::with_deadline(Instant::now()).unwrap();
    let polling = token.polling();
    assert_eq!(polling.poll(), Err(Stop::Deadline));
    token.cancel();
    assert_eq!(polling.poll(), Err(Stop::Cancelled));

    let slot = CancellationSlot::default();
    let run = slot.open(Some(Instant::now())).unwrap();
    let polling = run.cancellation().polling();
    assert_eq!(polling.poll(), Err(Stop::Deadline));
    slot.cancel();
    assert_eq!(polling.poll(), Err(Stop::Cancelled));
}
