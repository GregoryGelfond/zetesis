//! The cumulative work and decision allowance every search charges, and the
//! quotas that own it locally or share it between joined workers.

mod quota;
mod shared_budget;

pub(crate) use quota::{LocalQuota, Quota};
pub(crate) use shared_budget::{SharedBudget, WorkLease};

use crate::{Cancellation, Incomplete};

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
        self.cancellation.poll()?;
        let granted =
            self.quota
                .reserve_up_to(self.statistics.work, self.limits.max_work, wanted)?;
        self.statistics.work += granted;
        Ok(granted)
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
        let (result, used) =
            self.quota
                .with_reservation(self.statistics.work, self.limits.max_work, wanted, run)?;
        self.statistics.work = self
            .statistics
            .work
            .checked_add(used)
            .ok_or(Incomplete::CounterOverflow)?;
        Ok(result)
    }

    /// Return permits granted by [`Self::reserve_up_to`] and not spent.
    pub(crate) fn refund(&mut self, unspent: u64) {
        self.quota.refund(unspent);
        self.statistics.work -= unspent;
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
        self.statistics.work = self
            .statistics
            .work
            .checked_add(work)
            .ok_or(Incomplete::CounterOverflow)?;
        Ok(())
    }
}

pub(crate) fn increment(counter: &mut u64) -> Result<(), Incomplete> {
    *counter = counter.checked_add(1).ok_or(Incomplete::CounterOverflow)?;
    Ok(())
}

pub(crate) fn storage<T>(count: usize) -> Result<Vec<T>, Incomplete> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Incomplete::Allocation)?;
    Ok(result)
}
