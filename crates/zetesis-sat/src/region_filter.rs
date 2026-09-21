//! Candidate-only feasibility, independent of the frozen-reduct traversal.

use std::fmt::Debug;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use zetesis_cpu::regions::Region;
use zetesis_ferraris::Theory;

use crate::timing::{self, Phase};
use crate::{Cancellation, Incomplete, SearchPhaseTimings};

/// What an original-program condition establishes about a candidate region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionFeasibility {
    /// No exclusion was established. This certifies neither satisfaction nor
    /// answer-set membership, even when every atom is decided.
    NotRefuted,
    /// No interpretation represented by this region may be returned.
    Refuted,
}

/// A caller-owned original-program condition used only on candidate regions.
///
/// The factory authenticates the exact theory and atom coordinate meanings.
/// A refutation must exclude every sought answer in the region. The callback
/// never runs on a proper-subset query of a frozen reduct, and cannot replace
/// its membership decision. A caller can retain source-specific failure
/// evidence and return [`Incomplete::RegionFilter`] to identify it.
///
/// A checker is prepared on the first region not refuted by original narrowing.
/// Each persistent native worker retains that checker, borrowing this factory.
/// Scalar pulls and joined producer rounds prepare checkers per operation;
/// preparation is not assumed free or retained between those operations.
/// The factory owns all external resource limits: shared allowances must be
/// charged before work across every worker, including failed preparation and
/// checks. SAT's search-work counters exclude this separately accounted work.
pub trait RegionFilter: Debug + Send + Sync {
    /// Prepare independent mutable scratch for this worker or scalar operation.
    ///
    /// # Errors
    /// Preserve authentication, cancellation and resource failures as incomplete
    /// outcomes. Returning an error never establishes candidate coverage.
    fn worker(
        &self,
        theory: &Theory,
        cancellation: &Cancellation,
    ) -> Result<Box<dyn RegionFilterWorker + '_>, Incomplete>;
}

/// Mutable scratch borrowed from one original-program filter factory.
pub trait RegionFilterWorker {
    /// Read a region after original-theory and candidate-restriction narrowing.
    /// The theory and region are immutable; no learned restriction enters the
    /// reduct. Poll cancellation during bounded work and before a verdict.
    ///
    /// # Errors
    /// Any failure leaves this region unresolved and enumeration incomplete.
    fn check(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
    ) -> Result<RegionFeasibility, Incomplete>;
}

/// Callback attempts, independent of source-work and reduct-work accounting.
/// Live parallel snapshots need not be coherent; joined snapshots are exact
/// unless counter overflow explicitly marked the measurement incomplete.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegionFilterStatistics {
    /// Worker preparations attempted, including refused preparations.
    pub preparations: u64,
    /// Region checks attempted, including refused checks.
    pub checks: u64,
    /// Completed region refutations.
    pub refuted: u64,
    /// Preparations or checks that returned an error.
    pub failed: u64,
    /// A counter saturated; its retained value is an incomplete measurement.
    pub overflowed: bool,
}

#[derive(Debug)]
pub(crate) struct Filter {
    factory: Arc<dyn RegionFilter>,
    preparations: AtomicU64,
    checks: AtomicU64,
    refuted: AtomicU64,
    failed: AtomicU64,
    overflowed: AtomicBool,
}

impl Filter {
    pub(crate) fn new(factory: Arc<dyn RegionFilter>) -> Self {
        Self {
            factory,
            preparations: AtomicU64::new(0),
            checks: AtomicU64::new(0),
            refuted: AtomicU64::new(0),
            failed: AtomicU64::new(0),
            overflowed: AtomicBool::new(false),
        }
    }

    fn increment(&self, counter: &AtomicU64) {
        if counter
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .is_err()
        {
            self.overflowed.store(true, Ordering::Relaxed);
        }
    }

    pub(crate) fn statistics(&self) -> RegionFilterStatistics {
        RegionFilterStatistics {
            preparations: self.preparations.load(Ordering::Relaxed),
            checks: self.checks.load(Ordering::Relaxed),
            refuted: self.refuted.load(Ordering::Relaxed),
            failed: self.failed.load(Ordering::Relaxed),
            overflowed: self.overflowed.load(Ordering::Relaxed),
        }
    }

    /// Prepare only for an actual non-refuted original region, then retain
    /// the borrowed checker in the caller's operation-local worker slot.
    pub(crate) fn check<'a>(
        &'a self,
        worker: &mut Option<Worker<'a>>,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
        timings: &mut Option<SearchPhaseTimings>,
    ) -> Result<RegionFeasibility, Incomplete> {
        let started = timing::start(timings.as_ref());
        let result = (|| {
            let worker = match worker {
                Some(worker) => worker,
                None => worker.insert(self.worker(theory, cancellation)?),
            };
            worker.check(theory, region, cancellation)
        })();
        timing::finish(timings, Phase::OriginalValidation, started);
        result
    }

    fn worker(
        &self,
        theory: &Theory,
        cancellation: &Cancellation,
    ) -> Result<Worker<'_>, Incomplete> {
        self.increment(&self.preparations);
        match self.factory.worker(theory, cancellation) {
            Ok(checker) => Ok(Worker {
                filter: self,
                checker,
            }),
            Err(error) => {
                self.increment(&self.failed);
                Err(error)
            }
        }
    }
}

pub(crate) struct Worker<'a> {
    filter: &'a Filter,
    checker: Box<dyn RegionFilterWorker + 'a>,
}

impl Worker<'_> {
    fn check(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
    ) -> Result<RegionFeasibility, Incomplete> {
        self.filter.increment(&self.filter.checks);
        let result = self.checker.check(theory, region, cancellation);
        match result {
            Ok(RegionFeasibility::Refuted) => self.filter.increment(&self.filter.refuted),
            Err(_) => self.filter.increment(&self.filter.failed),
            Ok(RegionFeasibility::NotRefuted) => {}
        }
        result
    }
}
