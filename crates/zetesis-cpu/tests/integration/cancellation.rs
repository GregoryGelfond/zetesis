//! A deadline is observed like cancellation: a flag read at each boundary.

use std::thread;
use std::time::{Duration, Instant};

use zetesis_cpu::{Cancellation, Stop};

const SOON: Duration = Duration::from_millis(30);
const LATER: Duration = Duration::from_millis(150);

#[test]
fn an_expired_deadline_stops_the_first_poll() {
    let cancellation = Cancellation::with_deadline(Instant::now()).unwrap();
    assert_eq!(cancellation.poll(), Err(Stop::Deadline));
}

#[test]
fn a_deadline_expires_without_polling() {
    // No boundary polls between arming and expiry, so the expiry is the
    // timer's doing, not a clock read by the poll.
    let cancellation = Cancellation::with_deadline(Instant::now() + SOON).unwrap();
    assert_eq!(cancellation.poll(), Ok(()));
    thread::sleep(LATER);
    assert_eq!(cancellation.poll(), Err(Stop::Deadline));
}

#[test]
fn clones_share_the_deadline() {
    let cancellation = Cancellation::with_deadline(Instant::now() + SOON).unwrap();
    let clone = cancellation.clone();
    assert_eq!(clone.poll(), Ok(()));
    thread::sleep(LATER);
    assert_eq!(clone.poll(), Err(Stop::Deadline));
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
