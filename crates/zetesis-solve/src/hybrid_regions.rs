//! Original source constraints composed with the candidate-region traversal.
//!
//! The factory owns an immutable source handle. A worker borrows that handle
//! while retaining its own prepared lookup and check state. Only the cumulative
//! allowances and first fault are shared; candidate truth remains worker-local.
//! Neither this adapter nor its source checker participates in a frozen-reduct
//! proper-subset query.

use std::sync::{Mutex, PoisonError};

use zetesis_cpu::{Cancellation, regions::Region};
use zetesis_ferraris::Theory;
use zetesis_sat::{Incomplete, RegionFeasibility, RegionFilter, RegionFilterWorker};
use zetesis_themelios::{
    ConstraintAllowance, ConstraintCheckFailure, ConstraintChecker, ConstraintRegionVerdict,
    StreamedCore,
};

pub(crate) struct Constraints {
    owner: StreamedCore,
    allowance: ConstraintAllowance,
    failure: Mutex<Option<ConstraintCheckFailure>>,
}

impl Constraints {
    pub(crate) fn new(owner: StreamedCore, allowance: ConstraintAllowance) -> Self {
        Self {
            owner,
            allowance,
            failure: Mutex::new(None),
        }
    }

    /// Take the first source failure after the traversal has stopped its workers.
    /// Its receipt remains local to the failed checker. The hybrid outcome
    /// separately reports the joined allowance across all participating checks.
    pub(crate) fn take_failure(&self) -> Option<ConstraintCheckFailure> {
        let mut failure = self.failure.lock().unwrap_or_else(PoisonError::into_inner);
        failure.take()
    }

    /// Retain the first recorded source failure across worker and final checks.
    /// Concurrent recording order is not a claim about physical failure order.
    pub(crate) fn record_failure(&self, failure: ConstraintCheckFailure) {
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

impl std::fmt::Debug for Constraints {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Constraints")
            .field("owner", &self.owner)
            .field("allowance", &self.allowance.statistics())
            .finish_non_exhaustive()
    }
}

impl RegionFilter for Constraints {
    fn worker(
        &self,
        theory: &Theory,
        cancellation: &Cancellation,
    ) -> Result<Box<dyn RegionFilterWorker + '_>, Incomplete> {
        cancellation.poll()?;
        // Authenticate the factory's subject at worker preparation; the checker
        // repeats it at each operation before interpreting any dense position.
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
    source: &'a Constraints,
    checker: ConstraintChecker<'a>,
}

impl RegionFilterWorker for Worker<'_> {
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
