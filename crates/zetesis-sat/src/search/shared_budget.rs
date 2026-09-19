//! Scoped work permits for independent queries in one joined completion batch.
//!
//! Committed work + available permits + outstanding grants equals the initial
//! allowance. A lease consumes its grant locally, then commits used permits and
//! returns unused ones on every exit. Outstanding grants are not reported as
//! spent work. Decision reservations remain individually shared and nonblocking.

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

use crate::{Control, Incomplete, SearchLimits, SearchStatistics};

/// A scheduling policy, not a measured crossover: at most 64 uncommitted work
/// permits belong to a query. No grant changes the charged operation sequence.
const WORK_QUANTUM: u64 = 64;
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

    pub(crate) fn lease<'a>(&'a self, control: &'a Control) -> WorkLease<'a> {
        WorkLease {
            shared: self,
            control,
            granted: Cell::new(0),
            remaining: Cell::new(0),
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
                (spent < self.limits.max_decisions).then(|| spent + 1)
            })
            .map(|_| ())
            .map_err(|_| Incomplete::DecisionLimit)
    }

    /// Commit `work` permits at once, for work done outside any lease. The
    /// permits are taken from those available; when they do not cover it
    /// the charge is refused as exhaustion without waiting for outstanding
    /// grants to return, so a charge within the workers' grants of the
    /// ceiling, at most `WORK_QUANTUM` each, is refused early.
    pub(crate) fn charge(&self, work: u64) -> Result<(), Incomplete> {
        let mut permits = self.lock();
        if permits.available < work {
            return Err(Incomplete::WorkLimit);
        }
        permits.available -= work;
        permits.spent += work;
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
    control: &'a Control,
    granted: Cell<u64>,
    remaining: Cell<u64>,
}

impl WorkLease<'_> {
    pub(crate) fn tick(&self) -> Result<(), Incomplete> {
        if self.remaining.get() == 0 {
            self.settle();
            self.refill()?;
        }
        self.remaining.set(self.remaining.get() - 1);
        Ok(())
    }

    pub(crate) fn decide(&self) -> Result<(), Incomplete> {
        self.shared.decide()
    }

    /// Consume `amount` permits, refilling grant by grant; a refill that
    /// finds the allowance exhausted fails after the permits already taken.
    pub(crate) fn take(&self, mut amount: u64) -> Result<(), Incomplete> {
        while amount > 0 {
            if self.remaining.get() == 0 {
                self.settle();
                self.refill()?;
            }
            let taken = self.remaining.get().min(amount);
            self.remaining.set(self.remaining.get() - taken);
            amount -= taken;
        }
        Ok(())
    }

    fn refill(&self) -> Result<(), Incomplete> {
        loop {
            self.control.poll()?;
            let mut work = self.shared.lock();
            if let Some(grant) = work.grant()? {
                self.granted.set(grant);
                self.remaining.set(grant);
                return Ok(());
            }
            // Another query can still return unused permits. The timed wait
            // releases this sole shared lock, and the next iteration polls.
            drop(
                self.shared
                    .returned
                    .wait_timeout(work, CONTROL_WAIT)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
        }
    }

    fn settle(&self) {
        let granted = self.granted.replace(0);
        if granted == 0 {
            return;
        }
        let unused = self.remaining.replace(0);
        {
            let mut work = self.shared.lock();
            work.outstanding -= granted;
            work.spent += granted - unused;
            work.available += unused;
        }
        self.shared.returned.notify_all();
    }
}

impl Drop for WorkLease<'_> {
    fn drop(&mut self) {
        self.settle();
    }
}

#[cfg(test)]
#[path = "../../tests/support/work_leases.rs"]
mod tests;
