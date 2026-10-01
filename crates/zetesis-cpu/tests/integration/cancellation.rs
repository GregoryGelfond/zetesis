//! A deadline is observed like cancellation: a flag read at each boundary.

use std::time::{Duration, Instant};

use zetesis_cpu::{Cancellation, Stop};

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
