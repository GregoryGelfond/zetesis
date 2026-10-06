//! Per-check ceilings and one cumulative receipt shared by independent
//! constraint checkers.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use crate::{ConstraintCheckLimits, ConstraintCheckStatistics};

/// The ceilings every attached checker applies to each of its checks, and the
/// cumulative work, substitution and structural-capture charges all of them
/// accepted. The historical `scalar_bytes` field counts requested capture-delta
/// bytes, which borrow canonical terms rather than copying their payload.
///
/// Each check (one candidate model or region) gets the ceilings as its own
/// allowance, so they bound the work spent on any one candidate and never the
/// number of candidates a run checks. Clones share the same monotone receipt
/// counters, which saturate rather than refuse; stopping or dropping a checker
/// does not refund accepted charges. No ceiling is multiplied by the number of
/// workers. Separate checkers retain independent join/evaluation state and
/// their own local receipts.
#[derive(Clone, Debug)]
pub struct ConstraintAllowance(Arc<Shared>);

#[derive(Debug)]
struct Shared {
    limits: ConstraintCheckLimits,
    work: AtomicU64,
    substitutions: AtomicU64,
    scalar_bytes: AtomicU64,
}

impl ConstraintAllowance {
    /// Start a receipt shared by all checkers attached to its clones, each
    /// check of which is bounded by `limits`.
    #[must_use]
    pub fn new(limits: ConstraintCheckLimits) -> Self {
        Self(Arc::new(Shared {
            limits,
            work: AtomicU64::new(0),
            substitutions: AtomicU64::new(0),
            scalar_bytes: AtomicU64::new(0),
        }))
    }

    /// The ceilings each check of every attached checker gets for itself.
    #[must_use]
    pub fn limits(&self) -> ConstraintCheckLimits {
        self.0.limits
    }

    /// Total accepted charges, including preparation and interrupted prefixes.
    /// After workers join this is exact. During concurrent checks each field is
    /// a monotone observation, not a simultaneous snapshot of all three fields.
    #[must_use]
    pub fn statistics(&self) -> ConstraintCheckStatistics {
        self.0.receipt()
    }

    pub(crate) fn work(&self, amount: u128) {
        record(&self.0.work, amount);
    }

    pub(crate) fn substitution(&self) {
        record(&self.0.substitutions, 1);
    }

    pub(crate) fn scalar(&self, amount: u128) {
        record(&self.0.scalar_bytes, amount);
    }
}

impl Shared {
    fn receipt(&self) -> ConstraintCheckStatistics {
        ConstraintCheckStatistics {
            work: self.work.load(Ordering::Relaxed),
            substitutions: self.substitutions.load(Ordering::Relaxed),
            scalar_bytes: usize::try_from(self.scalar_bytes.load(Ordering::Relaxed))
                .expect("scalar allowance is bounded by usize"),
        }
    }
}

/// Only counters synchronize here, never program state or verdict publication.
/// The caller has already admitted the charge against its check's own ceiling.
fn record(counter: &AtomicU64, amount: u128) {
    let amount = u64::try_from(amount).unwrap_or(u64::MAX);
    // A saturated receipt stays saturated; it never wraps.
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
        Some(used.saturating_add(amount))
    });
}
