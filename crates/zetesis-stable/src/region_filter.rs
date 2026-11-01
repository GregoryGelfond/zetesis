//! Original candidate consequences, independent of the frozen-reduct traversal.
//!
//! The source owner is concrete: the admitted hybrid program supplies the
//! checker, and the candidate Subject applies its validated consequence.
//! No caller code can refute regions or decide atoms during a search.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use zetesis_algebra::region::Region;
use zetesis_ferraris::Theory;

use crate::streamed_constraints::{self, ConstraintConsequence, StreamedConstraints};
use crate::subject::Pass;
use crate::timing::{self, Phase};
use crate::{Cancellation, Incomplete, SearchPhaseTimings};

/// What an original-program condition establishes about a candidate region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RegionFeasibility {
    /// No exclusion was established. This certifies neither satisfaction nor
    /// answer-set membership, even when every atom is decided.
    NotRefuted,
    /// No interpretation represented by this region may be returned.
    Refuted,
}

/// The original source retained by a candidate stream. Production has only
/// the admitted hybrid source; test injections are absent from supported builds.
#[derive(Debug)]
pub(crate) enum Source {
    Streamed(Arc<StreamedConstraints>),
    #[cfg(test)]
    Fixture(Arc<dyn tests::injection::Source>),
}

impl Source {
    fn worker(
        &self,
        theory: &Theory,
        cancellation: &Cancellation,
    ) -> Result<Checker<'_>, Incomplete> {
        match self {
            Self::Streamed(source) => source.worker(theory, cancellation).map(Checker::Streamed),
            #[cfg(test)]
            Self::Fixture(source) => source.worker(theory, cancellation).map(Checker::Fixture),
        }
    }
}

/// One boxed source checker, borrowing its exact source owner. It remains
/// transferable between executor threads, never concurrently shared.
enum Checker<'a> {
    Streamed(Box<streamed_constraints::Worker<'a>>),
    #[cfg(test)]
    Fixture(Box<dyn tests::injection::Worker + 'a>),
}

impl Checker<'_> {
    fn check(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
    ) -> Result<RegionFeasibility, Incomplete> {
        match self {
            Self::Streamed(worker) => worker.check(theory, region, cancellation),
            #[cfg(test)]
            Self::Fixture(worker) => worker.check(theory, region, cancellation),
        }
    }

    fn consequence(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
        pass: Pass,
    ) -> Result<ConstraintConsequence, Incomplete> {
        match self {
            Self::Streamed(worker) => worker.consequence(theory, region, cancellation, pass),
            #[cfg(test)]
            Self::Fixture(worker) => worker.consequence(theory, region, cancellation, pass),
        }
    }
}

/// Attempts of the streamed constraints' region check, independent of
/// source-work and reduct-work accounting.
/// Live parallel snapshots need not be coherent; joined snapshots are exact
/// unless counter overflow explicitly marked the measurement incomplete.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegionFilterStatistics {
    /// Checker preparations attempted, by every traversal worker including a
    /// parallel walk's coordinator, including refused preparations.
    pub preparations: u64,
    /// Source passes attempted, including continuations and refused checks.
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
    factory: Source,
    preparations: AtomicU64,
    checks: AtomicU64,
    refuted: AtomicU64,
    failed: AtomicU64,
    overflowed: AtomicBool,
}

impl Filter {
    pub(crate) fn new(factory: Source) -> Self {
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
    /// the borrowed checker in the caller's worker slot.
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
    checker: Checker<'a>,
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

    fn consequence(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
        pass: Pass,
    ) -> Result<ConstraintConsequence, Incomplete> {
        self.filter.increment(&self.filter.checks);
        let result = self
            .checker
            .consequence(theory, region, cancellation, pass)
            .and_then(|consequence| {
                cancellation.poll()?;
                match consequence {
                    ConstraintConsequence::Hold { atom, .. }
                    | ConstraintConsequence::Cut { atom, .. }
                        if !region.is_open(atom) =>
                    {
                        Err(Incomplete::InvalidRegionConsequence)
                    }
                    _ => Ok(consequence),
                }
            });
        match result {
            Ok(ConstraintConsequence::Refuted { .. }) => {
                self.filter.increment(&self.filter.refuted);
            }
            Err(_) => self.filter.increment(&self.filter.failed),
            Ok(_) => {}
        }
        result
    }
}

/// One closure's callback state. The worker survives between closures; the pass
/// marker does not. Neither preparation nor a source pass changes region truth.
pub(crate) struct Check<'a, 'b> {
    pub(crate) filter: &'a Filter,
    pub(crate) worker: &'b mut Option<Worker<'a>>,
    pub(crate) timings: &'b mut Option<SearchPhaseTimings>,
}

impl Check<'_, '_> {
    pub(crate) fn consequence(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
        pass: Pass,
    ) -> Result<ConstraintConsequence, Incomplete> {
        let started = timing::start(self.timings.as_ref());
        let result = (|| {
            let worker = match self.worker {
                Some(worker) => worker,
                None => self
                    .worker
                    .insert(self.filter.worker(theory, cancellation)?),
            };
            worker.consequence(theory, region, cancellation, pass)
        })();
        timing::finish(self.timings, Phase::OriginalValidation, started);
        result
    }
}

#[cfg(test)]
pub(crate) mod tests;
