//! Statically selected quota ownership for local and joined parallel queries.

use super::shared_budget::WorkLease;
use crate::Incomplete;

/// Reservations precede the operation's local counter increment. This policy
/// changes quota ownership only; polling and charged operation order stay in Budget.
pub(crate) trait Quota {
    const BOUNDED_STORAGE: bool = false;
    fn work(&self, spent: u64, ceiling: u64) -> Result<(), Incomplete>;
    fn decision(&self, spent: u64, ceiling: u64) -> Result<(), Incomplete>;
    /// Reserve `amount` units of work at once, for an operation that counts
    /// its own work and reports it afterwards.
    fn charge(&self, spent: u64, ceiling: u64, amount: u64) -> Result<(), Incomplete>;
    /// Grant between one and `wanted` permits to an operation that spends
    /// them one per read and returns the unspent rest through [`Self::refund`];
    /// a grant never passes the ceiling, so the operation is refused at the
    /// read [`Self::work`] would refuse.
    fn reserve_up_to(&self, spent: u64, ceiling: u64, wanted: u64) -> Result<u64, Incomplete>;
    /// Take back permits granted by [`Self::reserve_up_to`] and not spent.
    fn refund(&self, unspent: u64);
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

    fn charge(&self, spent: u64, ceiling: u64, amount: u64) -> Result<(), Incomplete> {
        if spent.saturating_add(amount) > ceiling {
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

    fn reserve_up_to(&self, spent: u64, ceiling: u64, wanted: u64) -> Result<u64, Incomplete> {
        match ceiling.saturating_sub(spent).min(wanted) {
            0 => Err(Incomplete::WorkLimit),
            granted => Ok(granted),
        }
    }

    // The ledger is the budget's own count, which the caller lowers.
    fn refund(&self, _unspent: u64) {}
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

    fn charge(&self, spent: u64, ceiling: u64, amount: u64) -> Result<(), Incomplete> {
        self.0.charge(spent, ceiling, amount)
    }

    fn reserve_up_to(&self, spent: u64, ceiling: u64, wanted: u64) -> Result<u64, Incomplete> {
        self.0.reserve_up_to(spent, ceiling, wanted)
    }

    fn refund(&self, unspent: u64) {
        self.0.refund(unspent);
    }
}

// Worker counters start at zero and measure deltas; the shared budget is seeded
// with previously spent work and owns the cumulative reservation authority.
impl Quota for WorkLease<'_> {
    fn work(&self, _spent: u64, _ceiling: u64) -> Result<(), Incomplete> {
        self.tick()
    }

    fn decision(&self, _spent: u64, _ceiling: u64) -> Result<(), Incomplete> {
        self.decide()
    }

    fn charge(&self, _spent: u64, _ceiling: u64, amount: u64) -> Result<(), Incomplete> {
        self.take(amount)
    }

    fn reserve_up_to(&self, _spent: u64, _ceiling: u64, wanted: u64) -> Result<u64, Incomplete> {
        self.take_up_to(wanted)
    }

    fn refund(&self, unspent: u64) {
        self.give_back(unspent);
    }
}

#[cfg(test)]
mod tests;
