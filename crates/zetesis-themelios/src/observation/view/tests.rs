//! Encoding keeps its charged prefix and native refusal precedence.

use super::{Buffer, Cancellation, Stop, ViewError, ViewLimits};

#[test]
fn work_refusal_preserves_the_complete_accepted_prefix() {
    let cancellation = Cancellation::default();
    let mut buffer = Buffer::new(
        ViewLimits {
            max_work: u64::MAX,
            ..ViewLimits::default()
        },
        &cancellation,
    );
    buffer.step(u128::from(u64::MAX)).unwrap();
    buffer.step(0).unwrap();
    assert_eq!(buffer.step(1), Err(ViewError::Work));
    assert_eq!(buffer.work, u64::MAX);
    cancellation.cancel();
    assert_eq!(buffer.step(1), Err(ViewError::Stopped(Stop::Cancelled)));
    assert_eq!(buffer.work, u64::MAX);
    assert!(buffer.text.is_empty());
}

#[test]
fn byte_refusal_retains_encoding_work_before_any_output() {
    let cancellation = Cancellation::default();
    let mut buffer = Buffer::new(
        ViewLimits {
            max_bytes: 0,
            max_work: 2,
            ..ViewLimits::default()
        },
        &cancellation,
    );
    assert_eq!(buffer.text("a"), Err(ViewError::Bytes));
    assert_eq!(buffer.work, 2);
    assert!(buffer.text.is_empty());
    assert_eq!(buffer.text.capacity(), 0);
    assert_eq!(buffer.text("a"), Err(ViewError::Work));
    assert_eq!(buffer.work, 2);
    assert!(buffer.text.is_empty());
}

#[test]
fn symbol_depth_refusal_retains_only_the_opening_prefix() {
    let cancellation = Cancellation::default();
    let mut buffer = Buffer::new(
        ViewLimits {
            max_depth: 0,
            ..ViewLimits::default()
        },
        &cancellation,
    );
    assert_eq!(
        buffer.symbol(&super::Symbol::Infimum),
        Err(ViewError::Depth)
    );
    assert_eq!(buffer.text, "[");
    assert_eq!(buffer.work, 2);
    let mut refused = Buffer::new(
        ViewLimits {
            max_depth: 0,
            max_work: 0,
            ..ViewLimits::default()
        },
        &cancellation,
    );
    assert_eq!(
        refused.symbol(&super::Symbol::Infimum),
        Err(ViewError::Work)
    );
    assert!(refused.text.is_empty());
    assert_eq!(refused.work, 0);
}

#[test]
fn an_expired_deadline_stops_the_view_at_a_work_boundary() {
    use std::time::{Duration, Instant};
    let at = Instant::now() + Duration::from_millis(30);
    let cancellation = Cancellation::with_deadline(at).unwrap();
    let mut buffer = Buffer::new(ViewLimits::default(), &cancellation);
    buffer.step(1).unwrap();
    while Instant::now() < at {
        std::thread::sleep(Duration::from_millis(1));
    }
    // Inside a work interval the view reads control flags only.
    buffer.step(1).unwrap();
    let interval = zetesis_algebra::POLL_INTERVAL;
    let stopped = loop {
        match buffer.step(1) {
            Ok(()) => assert!(
                buffer.work <= 64 * interval,
                "the view must observe its deadline at a coarse boundary"
            ),
            Err(error) => break error,
        }
    };
    assert_eq!(stopped, ViewError::Stopped(Stop::Deadline));
    assert_eq!(buffer.work % interval, 0);
}
