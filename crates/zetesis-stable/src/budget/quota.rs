//! Statically selected quota ownership for local and joined parallel queries.

use super::shared_budget::{WorkLease, WorkReservation};
use crate::Incomplete;
use zetesis_algebra::{Meter, MeterWorkError};

/// Reservations precede the operation's local counter increment. This policy
/// changes quota ownership only; polling and charged operation order stay in Budget.
pub(crate) trait Quota {
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
    /// Ownership of an admitted whole-kernel allowance. The common meter
    /// validates the returned receipt before committing it through this guard.
    type Reservation<'call>: zetesis_algebra::Reservation
    where
        Self: 'call;

    /// Admit one whole-kernel reservation. A shared quota settles its current
    /// lease before waiting; the guard retains its unreceipted-work policy.
    fn reserve_kernel(
        &mut self,
        spent: u64,
        ceiling: u64,
        wanted: u64,
    ) -> Result<Self::Reservation<'_>, Incomplete>;
    /// Take back permits granted by [`Self::reserve_up_to`] and not spent.
    fn refund(&self, unspent: u64);
}

/// Ordinary queries have no shared pointer or per-operation execution-mode test.
pub(crate) struct LocalQuota;

/// A local grant has no shared account to settle; Budget publishes its receipt.
pub(crate) struct LocalReservation {
    granted: u64,
}

impl zetesis_algebra::Reservation for LocalReservation {
    fn allowance(&self) -> u64 {
        self.granted
    }

    fn commit(self, _used: u64) {}
}

impl Quota for LocalQuota {
    type Reservation<'call> = LocalReservation;

    fn work(&self, spent: u64, ceiling: u64) -> Result<(), Incomplete> {
        Meter::check_receipt(spent, 1, ceiling)
            .map(|_| ())
            .map_err(|_| Incomplete::WorkLimit)
    }

    fn charge(&self, spent: u64, ceiling: u64, amount: u64) -> Result<(), Incomplete> {
        match Meter::check_receipt(spent, u128::from(amount), ceiling) {
            Ok(_) => Ok(()),
            // Preserve this quota's saturating admission at the largest ceiling.
            // Budget then reports the native receipt carry as CounterOverflow,
            // at its existing post-quota publication boundary.
            Err(MeterWorkError::Limit { observed, .. })
                if ceiling == u64::MAX && observed > u128::from(u64::MAX) =>
            {
                Ok(())
            }
            Err(_) => Err(Incomplete::WorkLimit),
        }
    }

    fn decision(&self, spent: u64, ceiling: u64) -> Result<(), Incomplete> {
        Meter::check_wide_count(u128::from(spent), 1, u128::from(ceiling))
            .map(|_| ())
            .map_err(|_| Incomplete::DecisionLimit)
    }

    fn reserve_up_to(&self, spent: u64, ceiling: u64, wanted: u64) -> Result<u64, Incomplete> {
        match ceiling.saturating_sub(spent).min(wanted) {
            0 => Err(Incomplete::WorkLimit),
            granted => Ok(granted),
        }
    }

    fn reserve_kernel(
        &mut self,
        spent: u64,
        ceiling: u64,
        wanted: u64,
    ) -> Result<LocalReservation, Incomplete> {
        let granted = self.reserve_up_to(spent, ceiling, wanted)?;
        Ok(LocalReservation { granted })
    }

    // The ledger is the budget's own count, which the caller lowers.
    fn refund(&self, _unspent: u64) {}
}

// Worker counters start at zero and measure deltas; the shared budget is seeded
// with previously spent work and owns the cumulative reservation authority.
impl Quota for WorkLease<'_> {
    type Reservation<'call>
        = WorkReservation<'call>
    where
        Self: 'call;

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

    fn reserve_kernel(
        &mut self,
        _spent: u64,
        _ceiling: u64,
        wanted: u64,
    ) -> Result<WorkReservation<'_>, Incomplete> {
        self.reserve(wanted)
    }

    fn refund(&self, unspent: u64) {
        self.give_back(unspent);
    }
}

#[cfg(test)]
mod tests;
