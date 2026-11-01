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

use zetesis_algebra::{Cancellation, region::Region};
use zetesis_ferraris::Theory;
use zetesis_themelios::{
    ConstraintAllowance, ConstraintCheckFailure, ConstraintChecker, ConstraintRegionPass,
    ConstraintRegionVerdict, StreamedCore,
};

pub(crate) use zetesis_themelios::ConstraintConsequence;

use crate::{Incomplete, RegionFeasibility, subject::Pass};

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

impl StreamedConstraints {
    /// Prepare once for an actual non-refuted candidate region. The borrowed
    /// checker is retained across pulls by each serial/native worker or joined
    /// producer; determined-candidate checks retain their per-operation scope.
    /// Its original-source allowance and failure receipt are separate from
    /// search work, including failed preparation and interrupted checks.
    pub(crate) fn worker(
        &self,
        theory: &Theory,
        cancellation: &Cancellation,
    ) -> Result<Box<Worker<'_>>, Incomplete> {
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

pub(crate) struct Worker<'a> {
    source: &'a StreamedConstraints,
    checker: ConstraintChecker<'a>,
}

impl Worker<'_> {
    pub(crate) fn consequence(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
        pass: Pass,
    ) -> Result<ConstraintConsequence, Incomplete> {
        let pass = match pass {
            Pass::First => ConstraintRegionPass::First,
            Pass::Continue => ConstraintRegionPass::Continue,
        };
        self.checker
            .consequence_region(theory, region, cancellation, pass)
            .map_err(|error| self.source.stopped(error))
    }

    pub(crate) fn check(
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
