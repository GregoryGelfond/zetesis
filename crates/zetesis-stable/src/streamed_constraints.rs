//! An admitted program's streamed integrity constraints, checked over candidate
//! regions of its core theory.
//!
//! A hybrid owner's core is materialized and its integrity constraints are
//! streamed over it: the core's stable models are proposals, and only those
//! satisfying the streamed constraints are the owner's answer sets. This type
//! composes zetesis-themelios's checker for those constraints with candidate
//! narrowing. The constraints are the admitted program's own, so a refutation
//! excludes only interpretations that violate them, and exhaustion covers
//! exactly the owner's answer sets. No caller code takes part.
//!
//! The value owns an immutable source handle. Workers share its catalog index
//! and row positions, while retaining their own rule plans and check state.
//! Cumulative receipts and the first fault are shared. Each candidate closure
//! has its own allowance; candidate truth remains worker-local.
//! Neither this check nor its source checker participates in a frozen-reduct
//! proper-subset query.

use std::sync::{Mutex, PoisonError};

use zetesis_cpu::{Cancellation, regions::Region};
use zetesis_ferraris::Theory;
use zetesis_themelios::{
    ConstraintAllowance, ConstraintCheckFailure, ConstraintChecker, ConstraintConsequence,
    ConstraintRegionPass, ConstraintRegionVerdict, StreamedCore,
};

use crate::{
    Incomplete, RegionConsequence, RegionFeasibility, RegionFilter, RegionFilterWorker, RegionPass,
};

/// The streamed integrity constraints of an admitted hybrid program, checked
/// over candidate regions of its core theory. Install it on a candidate stream
/// with [`crate::StableModels::set_streamed_constraints`].
///
/// Each check charges `allowance`, which every checker of this value shares,
/// including a caller's final full-model checks. A check that stops records its
/// typed failure here and reports [`Incomplete::RegionFilter`]; read the first
/// recorded failure with [`StreamedConstraints::take_failure`]. A failure never
/// means a refutation or exhaustion.
pub struct StreamedConstraints {
    owner: StreamedCore,
    allowance: ConstraintAllowance,
    failure: Mutex<Option<ConstraintCheckFailure>>,
}

impl StreamedConstraints {
    /// Check `core`'s streamed constraints over its own core theory's candidate
    /// regions, charging `allowance`.
    #[must_use]
    pub fn new(core: StreamedCore, allowance: ConstraintAllowance) -> Self {
        Self {
            owner: core,
            allowance,
            failure: Mutex::new(None),
        }
    }

    /// Take the first source failure after the traversal has stopped its workers.
    /// Its receipt remains local to the failed checker. The hybrid outcome
    /// separately reports the joined allowance across all participating checks.
    pub fn take_failure(&self) -> Option<ConstraintCheckFailure> {
        let mut failure = self.failure.lock().unwrap_or_else(PoisonError::into_inner);
        failure.take()
    }

    /// Retain the first recorded source failure across worker and final checks.
    /// Concurrent recording order is not a claim about physical failure order.
    pub fn record_failure(&self, failure: ConstraintCheckFailure) {
        self.failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert(failure);
    }

    fn stopped(&self, failure: ConstraintCheckFailure) -> Incomplete {
        self.record_failure(failure);
        Incomplete::RegionFilter
    }
}

impl std::fmt::Debug for StreamedConstraints {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StreamedConstraints")
            .field("owner", &self.owner)
            .field("allowance", &self.allowance.statistics())
            .finish_non_exhaustive()
    }
}

impl RegionFilter for StreamedConstraints {
    fn worker(
        &self,
        theory: &Theory,
        cancellation: &Cancellation,
    ) -> Result<Box<dyn RegionFilterWorker + '_>, Incomplete> {
        cancellation.poll()?;
        // Authenticate the subject at worker preparation; the checker repeats
        // it at each operation before interpreting any dense position.
        if !theory.same_instance(self.owner.core_theory()) {
            return Err(self.stopped(ConstraintCheckFailure {
                cause: zetesis_themelios::ConstraintCheckCause::WrongProgram,
                statistics: zetesis_themelios::ConstraintCheckStatistics::default(),
            }));
        }
        let checker = self
            .owner
            .checker_with_allowance(&self.allowance, cancellation)
            .map_err(|error| self.stopped(error))?;
        Ok(Box::new(Worker {
            source: self,
            checker,
        }))
    }
}

struct Worker<'a> {
    source: &'a StreamedConstraints,
    checker: ConstraintChecker<'a>,
}

impl RegionFilterWorker for Worker<'_> {
    fn consequence(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
        pass: RegionPass,
    ) -> Result<RegionConsequence, Incomplete> {
        let pass = match pass {
            RegionPass::First => ConstraintRegionPass::First,
            RegionPass::Continue => ConstraintRegionPass::Continue,
        };
        self.checker
            .consequence_region(theory, region, cancellation, pass)
            .map(|consequence| match consequence {
                ConstraintConsequence::NoConsequence => RegionConsequence::Unchanged,
                ConstraintConsequence::Refuted { .. } => RegionConsequence::Refuted,
                ConstraintConsequence::Hold { atom, .. } => RegionConsequence::Hold(atom),
                ConstraintConsequence::Cut { atom, .. } => RegionConsequence::Cut(atom),
            })
            .map_err(|error| self.source.stopped(error))
    }

    fn check(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
    ) -> Result<RegionFeasibility, Incomplete> {
        self.checker
            .check_region(theory, region, cancellation)
            .map(|verdict| match verdict {
                ConstraintRegionVerdict::NotRefuted => RegionFeasibility::NotRefuted,
                ConstraintRegionVerdict::Refuted { .. } => RegionFeasibility::Refuted,
            })
            .map_err(|error| self.source.stopped(error))
    }
}

#[cfg(test)]
mod tests;
