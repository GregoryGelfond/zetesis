//! A deadline is observed like cancellation: a flag read at each boundary.

use std::thread;
use std::time::{Duration, Instant};

use zetesis_cpu::{Control, Stop};

const SOON: Duration = Duration::from_millis(30);
const LATER: Duration = Duration::from_millis(150);

#[test]
fn an_expired_deadline_stops_the_first_poll() {
    let control = Control::with_deadline(Instant::now()).unwrap();
    assert_eq!(control.poll(), Err(Stop::Deadline));
}

#[test]
fn a_deadline_expires_while_the_control_is_not_polled() {
    // No boundary polls between arming and expiry, so the expiry is the
    // timer's doing, not a clock read by the poll.
    let control = Control::with_deadline(Instant::now() + SOON).unwrap();
    assert_eq!(control.poll(), Ok(()));
    thread::sleep(LATER);
    assert_eq!(control.poll(), Err(Stop::Deadline));
}

#[test]
fn clones_share_the_deadline_and_the_cancellation() {
    let control = Control::with_deadline(Instant::now() + SOON).unwrap();
    let clone = control.clone();
    assert_eq!(clone.poll(), Ok(()));
    thread::sleep(LATER);
    assert_eq!(clone.poll(), Err(Stop::Deadline));
    assert_eq!(control.poll(), Err(Stop::Deadline));
    clone.cancel();
    assert_eq!(control.poll(), Err(Stop::Cancelled));
}

#[test]
fn cancellation_precedes_an_expired_deadline() {
    let control = Control::with_deadline(Instant::now()).unwrap();
    control.cancel();
    assert_eq!(control.poll(), Err(Stop::Cancelled));
}

#[test]
fn a_distant_deadline_never_interrupts_a_short_computation() {
    let control = Control::with_deadline(Instant::now() + Duration::from_hours(1)).unwrap();
    for _ in 0..1_000_000 {
        assert_eq!(control.poll(), Ok(()));
    }
}
