//! Shared receipts settle each accepted local prefix exactly once.

use super::{ConstraintAllowance, Pending};
use crate::{ConstraintCheckLimits, ConstraintCheckStatistics};

#[test]
fn in_flight_charges_remain_local() {
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits::default());
    let mut pending = Pending::new(allowance.clone());
    pending.work(13);
    pending.substitution();
    pending.scalar(7);
    assert_eq!(allowance.statistics(), ConstraintCheckStatistics::default());
    pending.settle();
    assert_eq!(
        allowance.statistics(),
        ConstraintCheckStatistics {
            work: 13,
            substitutions: 1,
            scalar_bytes: 7,
        }
    );
}

#[test]
fn settlement_publishes_each_charge_once() {
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits::default());
    let mut pending = Pending::new(allowance.clone());
    pending.work(13);
    pending.substitution();
    pending.scalar(7);
    pending.settle();
    pending.settle();
    pending.work(3);
    pending.substitution();
    pending.scalar(5);
    drop(pending);
    assert_eq!(
        allowance.statistics(),
        ConstraintCheckStatistics {
            work: 16,
            substitutions: 2,
            scalar_bytes: 12,
        }
    );
}

#[test]
fn a_fork_publishes_only_its_new_charges() {
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits::default());
    let mut original = Pending::new(allowance.clone());
    original.work(13);
    original.substitution();
    original.scalar(7);
    let mut fork = original.fork();
    fork.work(3);
    fork.substitution();
    fork.scalar(5);
    drop(fork);
    assert_eq!(
        allowance.statistics(),
        ConstraintCheckStatistics {
            work: 3,
            substitutions: 1,
            scalar_bytes: 5,
        }
    );
    drop(original);
    assert_eq!(
        allowance.statistics(),
        ConstraintCheckStatistics {
            work: 16,
            substitutions: 2,
            scalar_bytes: 12,
        }
    );
}

#[test]
fn scalar_receipts_accumulate_across_clones() {
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits {
        max_scalar_bytes: 7,
        ..Default::default()
    });
    for _ in 0..2 {
        let mut pending = Pending::new(allowance.clone());
        pending.scalar(7);
    }
    assert_eq!(allowance.statistics().scalar_bytes, 14);
}

#[test]
fn receipts_saturate_at_their_public_extents() {
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits::default());
    // Seed the public boundary without an impractical number of substitutions.
    // Each later pending prefix still consists of separately accepted charges.
    allowance.record(ConstraintCheckStatistics {
        work: u64::MAX - 1,
        substitutions: u64::MAX - 1,
        scalar_bytes: usize::MAX - 1,
    });
    for _ in 0..3 {
        let mut pending = Pending::new(allowance.clone());
        pending.work(1);
        pending.substitution();
        pending.scalar(1);
    }
    assert_eq!(
        allowance.statistics(),
        ConstraintCheckStatistics {
            work: u64::MAX,
            substitutions: u64::MAX,
            scalar_bytes: usize::MAX,
        }
    );
}

#[test]
fn a_pending_prefix_saturates_before_publication() {
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits::default());
    let mut pending = Pending::new(allowance.clone());
    pending.work(u64::MAX);
    pending.work(1);
    pending.charges.substitutions = u64::MAX;
    pending.substitution();
    pending.scalar(u128::MAX);
    pending.scalar(1);
    pending.settle();
    assert_eq!(
        allowance.statistics(),
        ConstraintCheckStatistics {
            work: u64::MAX,
            substitutions: u64::MAX,
            scalar_bytes: usize::MAX,
        }
    );
}
