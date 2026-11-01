//! Scoped work permits for joined queries and parallel region walks.
//!
//! Committed work + available permits + outstanding grants equals the initial
//! allowance. A lease consumes its grant locally, then commits used permits and
//! returns unused ones when it settles: before its holder waits, delivers or
//! reserves, and on every exit. A settled lease refills on its next use.
//! Outstanding grants are not reported as spent work. Decision reservations
//! remain individually shared and nonblocking.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

use crate::{Cancellation, Incomplete, SearchLimits, SearchStatistics};
use zetesis_algebra::{Meter, PermitLease};

/// A scheduling policy, not a measured crossover: an incremental grant holds
/// at most this many permits. A region walker keeps its lease across its
/// consecutive regions, which commonly use a few hundred permits each, so one
/// grant serves many regions and the shared ledger lock is taken about twice
/// per grant rather than twice per region. Accounted kernels reserve their
/// complete bounded allowance separately. No grant changes the charged
/// operation sequence, so the work limit still bounds the identical total; a
/// larger grant only coarsens how the shared allowance is parcelled out among
/// workers and how often a lease rechecks cooperative control.
const WORK_QUANTUM: u64 = 16384;
/// A waiting query rechecks cooperative control at least once per timed wait.
/// Lock acquisition and OS scheduling do not provide a hard response deadline.
const CONTROL_WAIT: Duration = Duration::from_millis(1);

pub(crate) struct SharedBudget {
    limits: SearchLimits,
    work: Mutex<Work>,
    returned: Condvar,
    decisions: AtomicU64,
}

struct Work {
    spent: u64,
    available: u64,
    outstanding: u64,
}

impl Work {
    /// None means another lease can still return permits, not exhaustion.
    fn grant(&mut self) -> Result<Option<u64>, Incomplete> {
        if self.available != 0 {
            let grant = self.available.min(WORK_QUANTUM);
            self.available -= grant;
            self.outstanding += grant;
            Ok(Some(grant))
        } else if self.outstanding == 0 {
            Err(Incomplete::WorkLimit)
        } else {
            Ok(None)
        }
    }

    /// An accounted kernel needs its allowance before entering. Wait for
    /// outstanding grants when they can still cover a shortfall; otherwise
    /// the remaining allowance bounds a possibly incomplete kernel attempt.
    fn reserve(&mut self, wanted: u64) -> Option<u64> {
        if self.available < wanted && self.outstanding != 0 {
            return None;
        }
        let granted = wanted.min(self.available);
        self.available -= granted;
        self.outstanding += granted;
        Some(granted)
    }
}

impl SharedBudget {
    pub(crate) fn new(limits: SearchLimits, spent: SearchStatistics) -> Self {
        Self {
            limits,
            work: Mutex::new(Work {
                spent: spent.work,
                available: limits.max_work.saturating_sub(spent.work),
                outstanding: 0,
            }),
            returned: Condvar::new(),
            decisions: AtomicU64::new(spent.decisions),
        }
    }

    pub(crate) fn lease<'a>(&'a self, cancellation: &'a Cancellation) -> WorkLease<'a> {
        WorkLease {
            shared: self,
            cancellation,
            permits: PermitLease::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Work> {
        // Only infallible conservation updates run while held: no query work,
        // allocation, user callback or control poll can unwind through this lock.
        self.work
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn decide(&self) -> Result<(), Incomplete> {
        self.decisions
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |spent| {
                Meter::check_wide_count(u128::from(spent), 1, u128::from(self.limits.max_decisions))
                    .ok()
                    .map(|_| spent + 1)
            })
            .map(|_| ())
            .map_err(|_| Incomplete::DecisionLimit)
    }

    /// Commit `work` permits at once, for work done outside any lease. The
    /// permits are taken from those available; when they do not cover it
    /// the charge is refused as exhaustion without waiting for outstanding
    /// grants or kernel reservations to return, so held allowances can cause
    /// an early refusal.
    pub(crate) fn charge(&self, work: u64) -> Result<(), Incomplete> {
        let mut permits = self.lock();
        // Outstanding grants are unavailable to this direct charge. The
        // conservation invariant keeps the currently spendable ceiling in u64.
        let ceiling = permits.spent + permits.available;
        let spent = Meter::check_receipt(permits.spent, u128::from(work), ceiling)
            .map_err(|_| Incomplete::WorkLimit)?;
        permits.available -= work;
        permits.spent = spent;
        Ok(())
    }

    /// The work committed so far, while leases may still hold permits: a
    /// lower bound on the work spent, exact once every lease has dropped.
    pub(crate) fn snapshot(&self, statistics: &mut SearchStatistics) {
        let work = self.lock();
        statistics.work = work.spent;
        statistics.decisions = self.decisions.load(Ordering::Relaxed);
    }

    /// Every lease has dropped and all workers have joined before this read,
    /// so the snapshot is exact.
    pub(crate) fn record(&self, statistics: &mut SearchStatistics) {
        debug_assert_eq!(self.lock().outstanding, 0);
        self.snapshot(statistics);
    }
}

/// A query owns at most one grant and never waits while retaining a permit.
/// The Cells are local to that query; the lease is deliberately not Sync.
pub(crate) struct WorkLease<'a> {
    shared: &'a SharedBudget,
    cancellation: &'a Cancellation,
    permits: PermitLease,
}

impl WorkLease<'_> {
    /// Reserve an accounted kernel's finite work bound before it runs.
    /// Settle this lease first, so waiting cannot retain the permits needed
    /// by another reservation. Returned attempts settle their reported work;
    /// an unwinding attempt conservatively consumes its whole reservation.
    pub(crate) fn reserve(&mut self, wanted: u64) -> Result<WorkReservation<'_>, Incomplete> {
        self.settle();
        loop {
            self.cancellation.poll()?;
            let mut work = self.shared.lock();
            if let Some(granted) = work.reserve(wanted) {
                return Ok(WorkReservation {
                    shared: self.shared,
                    granted,
                    used: granted,
                });
            }
            drop(
                self.shared
                    .returned
                    .wait_timeout(work, CONTROL_WAIT)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
        }
    }

    pub(crate) fn tick(&self) -> Result<(), Incomplete> {
        self.permits.tick(|| self.refill())
    }

    pub(crate) fn decide(&self) -> Result<(), Incomplete> {
        self.shared.decide()
    }

    /// Consume `amount` permits, refilling grant by grant; a refill that
    /// finds the allowance exhausted fails after the permits already taken.
    pub(crate) fn take(&self, amount: u64) -> Result<(), Incomplete> {
        self.permits.take(amount, || self.refill())
    }

    /// Take between one and `wanted` permits from the current grant,
    /// refilling it first when it is spent.
    pub(crate) fn take_up_to(&self, wanted: u64) -> Result<u64, Incomplete> {
        self.permits.take_up_to(wanted, || self.refill())
    }

    /// Return permits taken by [`Self::take_up_to`] and not spent to the
    /// current grant, from which they came.
    pub(crate) fn give_back(&self, unspent: u64) {
        self.permits.give_back(unspent);
    }

    fn refill(&self) -> Result<(), Incomplete> {
        loop {
            if let Err(error) = self.cancellation.poll() {
                // A refused renewal still commits the consumed grant before
                // returning, without acquiring another allowance.
                self.settle();
                return Err(error.into());
            }
            let mut work = self.shared.lock();
            // The old grant is wholly consumed. Settle it and acquire its
            // replacement under one lock; no unused permits become available.
            let renewed = self.permits.renew(|prior| {
                work.outstanding -= prior.granted();
                work.spent += prior.used();
                work.grant()
            });
            match renewed {
                Ok(true) => return Ok(()),
                Err(error) => {
                    // Settling the last outstanding grant establishes true
                    // exhaustion. Peers must wake even without a permit return.
                    drop(work);
                    self.shared.returned.notify_all();
                    return Err(error);
                }
                Ok(false) => {}
            }
            // Another query can still return unused permits. The timed wait
            // releases this sole shared lock, and the next iteration polls.
            // This lease retains no grant and has enabled no waiting peer.
            drop(
                self.shared
                    .returned
                    .wait_timeout(work, CONTROL_WAIT)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
        }
    }

    /// Commit the consumed part of the current grant and return its unused
    /// part; the lease stays usable and refills on its next use. A holder calls
    /// this before any wait, delivery or reservation, so a peer never waits for
    /// permits that this lease is not using.
    pub(crate) fn settle(&self) {
        self.permits.settle(|prior| {
            {
                let mut work = self.shared.lock();
                work.outstanding -= prior.granted();
                work.spent += prior.used();
                work.available += prior.unused();
            }
            self.shared.returned.notify_all();
        });
    }
}

impl Drop for WorkLease<'_> {
    fn drop(&mut self) {
        self.settle();
    }
}

/// An accounted operation's pre-reserved allowance. A returned attempt records
/// its actual work and refunds the unused part. Without a returned receipt,
/// including unwinding, the whole grant is charged; the caller must report
/// that failure without establishing enumeration coverage.
pub(crate) struct WorkReservation<'a> {
    shared: &'a SharedBudget,
    granted: u64,
    used: u64,
}

impl zetesis_algebra::Reservation for WorkReservation<'_> {
    fn allowance(&self) -> u64 {
        self.granted
    }

    fn commit(mut self, used: u64) {
        self.used = used;
    }
}

impl Drop for WorkReservation<'_> {
    fn drop(&mut self) {
        {
            let mut work = self.shared.lock();
            work.outstanding -= self.granted;
            work.spent += self.used;
            work.available += self.granted - self.used;
        }
        self.shared.returned.notify_all();
    }
}

#[cfg(test)]
mod tests;
