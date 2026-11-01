//! The cumulative work and decision allowance every search charges, and the
//! quotas that own it locally or share it between joined workers.

mod quota;
mod shared_budget;

pub(crate) use quota::{LocalQuota, Quota};
pub(crate) use shared_budget::{SharedBudget, WorkLease};

use crate::{Cancellation, Incomplete};
use zetesis_algebra::reduction::{CheckedSum, Reduction as _};
use zetesis_algebra::{Failure, Meter, Reservation as _, Stop};

/// Cumulative search ceilings. Zero permits no operation of that kind.
#[derive(Clone, Copy, Debug)]
pub struct SearchLimits {
    /// Charged operations, cumulatively: region narrowing and traversal, the
    /// original index's construction, proper-subset queries, candidate
    /// restrictions and bounds. Stable-model enumeration also charges optional
    /// certificate work here.
    pub max_work: u64,
    /// Maximum region splits, in the candidate tree and in every proper-subset
    /// query's tree together.
    pub max_decisions: u64,
}
impl Default for SearchLimits {
    fn default() -> Self {
        Self {
            max_work: 100_000_000,
            max_decisions: 1_000_000,
        }
    }
}

/// Accounting for charged search and certificate operations, including failures.
/// A panicking parallel worker conservatively consumes a reservation whose
/// actual-work receipt could not be returned, and reports `WorkerPanicked`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchStatistics {
    /// Charged primitive operations, including reservations lost to worker panic.
    pub work: u64,
    /// Region splits charged against [`SearchLimits::max_decisions`].
    pub decisions: u64,
}

pub(crate) struct Budget<'a, Q = LocalQuota> {
    pub(crate) quota: Q,
    pub(crate) limits: SearchLimits,
    pub(crate) cancellation: &'a Cancellation,
    pub(crate) statistics: SearchStatistics,
}
impl<Q: Quota> Budget<'_, Q> {
    pub(crate) fn tick(&mut self) -> Result<(), Incomplete> {
        self.cancellation.poll()?;
        self.quota
            .work(self.statistics.work, self.limits.max_work)?;
        self.statistics.work += 1;
        Ok(())
    }
    pub(crate) fn decide(&mut self) -> Result<(), Incomplete> {
        self.tick()?;
        self.quota
            .decision(self.statistics.decisions, self.limits.max_decisions)?;
        self.statistics.decisions += 1;
        Ok(())
    }
    /// Grant between one and `wanted` permits to an operation that spends
    /// them one per read: control is polled once for the batch, and the
    /// granted permits count as spent until [`Self::refund`] returns the rest.
    pub(crate) fn reserve_up_to(&mut self, wanted: u64) -> Result<u64, Incomplete> {
        zetesis_algebra::Quota::reserve(&mut self.incremental(), wanted)
    }
    /// Run a finite accounted kernel once under its full reservation, rather
    /// than treating one partial lease grant as the kernel's complete limit.
    /// Actual returned work is committed on success and refusal alike.
    pub(crate) fn with_reservation<T>(
        &mut self,
        wanted: u64,
        run: impl FnOnce(u64) -> (T, u64),
    ) -> Result<T, Incomplete> {
        self.cancellation.poll()?;
        let reservation =
            self.quota
                .reserve_kernel(self.statistics.work, self.limits.max_work, wanted)?;
        if reservation.allowance() == 0 {
            return Err(Incomplete::WorkLimit);
        }
        let (result, used) =
            Meter::under_reservation(reservation, |granted| Ok::<_, Incomplete>(run(granted)))
                .map_err(reservation_failure)?;
        self.statistics.work = self
            .statistics
            .work
            .checked_add(used)
            .ok_or(Incomplete::CounterOverflow)?;
        Ok(result)
    }

    /// Return permits granted by [`Self::reserve_up_to`] and not spent.
    pub(crate) fn refund(&mut self, unspent: u64) {
        zetesis_algebra::Quota::refund(&mut self.incremental(), unspent);
    }
    /// Supply an atomic minimum through the quota's ownership-aware boundary.
    pub(crate) fn replenish(
        &mut self,
        unused: &mut u64,
        minimum: u64,
        capacity: u64,
    ) -> Result<(), Failure<Incomplete>> {
        Q::replenish(self.incremental(), unused, minimum, capacity)
    }

    fn incremental(&mut self) -> quota::Incremental<'_, Q> {
        quota::Incremental {
            quota: &self.quota,
            work: &mut self.statistics.work,
            ceiling: self.limits.max_work,
            cancellation: self.cancellation,
        }
    }

    /// The work left before the ceiling, for an operation that charges its
    /// own work and reports it afterwards through [`Self::charge`].
    pub(crate) fn remaining_work(&self) -> u64 {
        self.limits.max_work.saturating_sub(self.statistics.work)
    }
    /// Charge an operation's work, one poll for the lot, reserved through the
    /// quota as ticks would be. The work may be already performed, or known
    /// in advance and charged before it is performed, as the original index
    /// is; a refused charge adds nothing.
    pub(crate) fn charge(&mut self, work: u64) -> Result<(), Incomplete> {
        self.cancellation.poll()?;
        self.quota
            .charge(self.statistics.work, self.limits.max_work, work)?;
        Meter::charge_receipt(&mut self.statistics.work, u128::from(work), u64::MAX)
            .map_err(|_| Incomplete::CounterOverflow)
    }
}

/// Preserve callback errors and the existing invalid-receipt classification
/// at the whole-kernel boundary, without another control observation.
pub(crate) fn reservation_failure(error: Failure<Incomplete>) -> Incomplete {
    match error {
        Failure::Refused(error) => error,
        Failure::Stopped(Stop::InvalidProgram) => Incomplete::InvalidWitness,
        Failure::Stopped(other) => Incomplete::Verification(other),
    }
}

pub(crate) fn increment(counter: &mut u64) -> Result<(), Incomplete> {
    *counter = CheckedSum
        .combine(Some(*counter), Some(1))
        .ok_or(Incomplete::CounterOverflow)?;
    Ok(())
}

pub(crate) fn storage<T>(count: usize) -> Result<Vec<T>, Incomplete> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Incomplete::Allocation)?;
    Ok(result)
}

#[cfg(test)]
mod tests;
