//! Per-check ceilings and one cumulative receipt shared by independent
//! constraint checkers.

use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

use crate::{ConstraintCheckLimits, ConstraintCheckStatistics};

/// The ceilings every attached checker applies to each of its checks, and the
/// cumulative work, substitution and structural-capture charges all of them
/// accepted. The historical `scalar_bytes` field counts requested capture-delta
/// bytes, which borrow canonical terms rather than copying their payload.
///
/// Each check (one model, region-refutation scan, or candidate closure including
/// all of its consequence passes) gets the ceilings as its own
/// allowance, so they bound the work spent on any one candidate and never the
/// number of candidates a run checks. Clones share monotone receipt counters,
/// published when preparation or a check returns, fails or unwinds, and when
/// a checker drops. In-flight charges can remain local until that settlement.
/// Counters saturate at their public field representation rather than refuse;
/// stopping or dropping a checker never refunds accepted charges. No ceiling
/// is multiplied by the number of workers. Separate checkers retain independent
/// join/evaluation state and their own exact local receipts.
#[derive(Clone, Debug)]
pub struct ConstraintAllowance(Arc<Shared>);

#[derive(Debug)]
struct Shared {
    limits: ConstraintCheckLimits,
    work: AtomicU64,
    substitutions: AtomicU64,
    scalar_bytes: AtomicUsize,
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
            scalar_bytes: AtomicUsize::new(0),
        }))
    }

    /// The ceilings each check of every attached checker gets for itself.
    #[must_use]
    pub fn limits(&self) -> ConstraintCheckLimits {
        self.0.limits
    }

    /// Settled accepted charges, including preparation and interrupted prefixes.
    /// Each field is a monotone lower bound while operations are in flight:
    /// those operations can retain unpublished charges until they return, fail,
    /// unwind or drop. After all operations settle or workers join, this is
    /// exact until the field saturates. Fields are observed independently, not
    /// as a simultaneous snapshot. Check-local and failure receipts remain exact.
    #[must_use]
    pub fn statistics(&self) -> ConstraintCheckStatistics {
        self.0.receipt()
    }

    fn record(&self, charges: ConstraintCheckStatistics) {
        record(&self.0.work, charges.work);
        record(&self.0.substitutions, charges.substitutions);
        if charges.scalar_bytes != 0 {
            // Per-check allowances do not bound the sum shared by many checks.
            // Saturate at the public receipt's usize extent on every host.
            let _ =
                self.0
                    .scalar_bytes
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                        Some(used.saturating_add(charges.scalar_bytes))
                    });
        }
    }
}

impl Shared {
    fn receipt(&self) -> ConstraintCheckStatistics {
        ConstraintCheckStatistics {
            work: self.work.load(Ordering::Relaxed),
            substitutions: self.substitutions.load(Ordering::Relaxed),
            scalar_bytes: self.scalar_bytes.load(Ordering::Relaxed),
        }
    }
}

/// Only counters synchronize here, never program state or verdict publication.
/// The caller has already admitted these charges against its check's own ceiling.
fn record(counter: &AtomicU64, amount: u64) {
    if amount == 0 {
        return;
    }
    // A saturated receipt stays saturated; it never wraps.
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
        Some(used.saturating_add(amount))
    });
}

/// Accepted charges owned by one local operation until settlement. No charge
/// synchronizes with another checker. Moving this owner preserves its pending
/// charges; dropping it publishes them, including during unwinding.
///
/// This is separate from the check's enforcement counters: settlement resets
/// only pending publication, never the resource ceilings or accepted history.
pub(crate) struct Pending {
    allowance: ConstraintAllowance,
    charges: ConstraintCheckStatistics,
}

impl Pending {
    pub(crate) fn new(allowance: ConstraintAllowance) -> Self {
        Self {
            allowance,
            charges: ConstraintCheckStatistics::default(),
        }
    }

    /// A tentative budget shares the destination, not the original's pending
    /// history. Only future charges on the fork are newly publishable.
    pub(crate) fn fork(&self) -> Self {
        Self::new(self.allowance.clone())
    }

    #[inline]
    pub(crate) fn work(&mut self, amount: u64) {
        self.charges.work = self.charges.work.saturating_add(amount);
    }

    #[inline]
    pub(crate) fn substitution(&mut self) {
        self.charges.substitutions = self.charges.substitutions.saturating_add(1);
    }

    pub(crate) fn scalar(&mut self, amount: u128) {
        let amount = usize::try_from(amount).unwrap_or(usize::MAX);
        self.charges.scalar_bytes = self.charges.scalar_bytes.saturating_add(amount);
    }

    /// Publish each accepted charge once, including after repeated settlement.
    /// Taking the pending fields first also leaves Drop with nothing to repeat.
    pub(crate) fn settle(&mut self) {
        self.allowance.record(std::mem::take(&mut self.charges));
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        self.settle();
    }
}

#[cfg(test)]
mod tests;
