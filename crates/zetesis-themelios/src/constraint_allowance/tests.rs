//! Shared receipts retain their public representation across checker clones.

use super::ConstraintAllowance;
use crate::ConstraintCheckLimits;

#[test]
fn scalar_receipts_accumulate_across_clones() {
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits {
        max_scalar_bytes: 7,
        ..Default::default()
    });
    allowance.scalar(7);
    allowance.clone().scalar(7);
    assert_eq!(allowance.statistics().scalar_bytes, 14);
}

#[test]
fn scalar_receipts_saturate_at_their_public_extent() {
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits {
        max_scalar_bytes: usize::MAX,
        ..Default::default()
    });
    // Both charges fit a separate check. Their sum can exceed the shared
    // usize receipt even on a host whose AtomicU64 would still have room.
    allowance.scalar(usize::MAX as u128);
    assert_eq!(allowance.statistics().scalar_bytes, usize::MAX);
    let other = allowance.clone();
    other.scalar(1);
    assert_eq!(allowance.statistics().scalar_bytes, usize::MAX);
    other.scalar(7);
    assert_eq!(other.statistics().scalar_bytes, usize::MAX);
}
