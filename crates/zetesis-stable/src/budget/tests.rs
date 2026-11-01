//! Checked counter publication preserves the previous value on overflow.

use super::increment;
use crate::Incomplete;

#[test]
fn increment_accepts_the_last_value_without_wrapping_or_partial_publication() {
    let mut count = u64::MAX - 1;
    assert_eq!(increment(&mut count), Ok(()));
    assert_eq!(count, u64::MAX);
    assert_eq!(increment(&mut count), Err(Incomplete::CounterOverflow));
    assert_eq!(count, u64::MAX);
}
