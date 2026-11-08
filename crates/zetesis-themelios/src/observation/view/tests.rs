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
