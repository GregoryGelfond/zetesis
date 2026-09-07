//! Statically selected quota ownership for local and joined parallel queries.

use super::SharedBudget;
use crate::Incomplete;

/// Reservations precede the operation's local counter increment. This policy
/// changes quota ownership only; polling and charged operation order stay in Budget.
pub(crate) trait Quota {
    const BOUNDED_STORAGE: bool = false;
    fn work(&self, spent: u64, ceiling: u64) -> Result<(), Incomplete>;
    fn decision(&self, spent: u64, ceiling: u64) -> Result<(), Incomplete>;
}

/// Ordinary queries have no shared pointer or per-operation execution-mode test.
pub(crate) struct LocalQuota;

impl Quota for LocalQuota {
    fn work(&self, spent: u64, ceiling: u64) -> Result<(), Incomplete> {
        if spent >= ceiling {
            Err(Incomplete::WorkLimit)
        } else {
            Ok(())
        }
    }

    fn decision(&self, spent: u64, ceiling: u64) -> Result<(), Incomplete> {
        if spent >= ceiling {
            Err(Incomplete::DecisionLimit)
        } else {
            Ok(())
        }
    }
}

/// Completion preflights a complete logical workspace before query allocation.
pub(crate) struct BoundedQuota<Q>(pub(crate) Q);

impl<Q: Quota> Quota for BoundedQuota<Q> {
    const BOUNDED_STORAGE: bool = true;

    fn work(&self, spent: u64, ceiling: u64) -> Result<(), Incomplete> {
        self.0.work(spent, ceiling)
    }

    fn decision(&self, spent: u64, ceiling: u64) -> Result<(), Incomplete> {
        self.0.decision(spent, ceiling)
    }
}

// Worker counters start at zero and measure deltas; the shared budget is seeded
// with previously spent work and owns the cumulative reservation authority.
impl Quota for &SharedBudget {
    fn work(&self, _spent: u64, _ceiling: u64) -> Result<(), Incomplete> {
        self.tick()
    }

    fn decision(&self, _spent: u64, _ceiling: u64) -> Result<(), Incomplete> {
        self.decide()
    }
}

#[cfg(test)]
#[path = "../../tests/support/quota_contracts.rs"]
mod tests;
