//! Complete deterministic evidence restricts the proposed answer family to one row.

use std::sync::Arc;

use super::Certification;
use crate::Incomplete;
use crate::ferraris::{Limits, Proposer, Statistics};
use crate::search::Budget;
use zetesis_ferraris::{Interpretation, Theory};

/// A successful original-owner deterministic plan is the coverage certificate. The
/// existing proposer retains candidate restrictions, filter and preparation
/// receipts; its region frontier is never entered while this cursor is active.
#[derive(Debug)]
pub(in crate::ferraris) struct DeterminedCandidates {
    certificate: Arc<Certification>,
    remaining: bool,
}

impl DeterminedCandidates {
    pub(in crate::ferraris) fn new(certificate: Arc<Certification>) -> Self {
        Self {
            certificate,
            remaining: true,
        }
    }

    pub(in crate::ferraris) fn propose(
        &mut self,
        source: &mut Proposer,
        theory: &Theory,
        limits: Limits,
        budget: &mut Budget<'_>,
        statistics: &mut Statistics,
    ) -> Result<Option<Interpretation>, Incomplete> {
        budget.cancellation.poll()?;
        let plan = self
            .certificate
            .determined()
            .ok_or(Incomplete::InvalidWitness)?;
        if !theory.same_instance(plan.interpretation.theory()) {
            return Err(Incomplete::WrongTheory);
        }
        if !self.remaining {
            return Ok(None);
        }
        if plan.failed_constraint.is_some() {
            self.remaining = false;
            return Ok(None);
        }
        let candidate = plan.interpretation;
        let permitted = match source {
            Proposer::Clauses(_) => return Err(Incomplete::InvalidWitness),
            Proposer::Regions(regions) => {
                regions.permits_determined(candidate, budget, &mut statistics.phase_timings)
            }
            Proposer::Parallel(parallel) => {
                parallel.permits_determined(candidate, budget, &mut statistics.phase_timings)
            }
            Proposer::Proposals(proposals) => {
                proposals.permits_determined(candidate, budget, &mut statistics.phase_timings)
            }
        }?;
        budget.cancellation.poll()?;
        if !permitted {
            self.remaining = false;
            return Ok(None);
        }
        if statistics.candidates >= limits.max_candidates {
            return Err(Incomplete::CandidateLimit);
        }
        let candidate = copy(candidate, budget)?;
        budget.cancellation.poll()?;
        self.remaining = false;
        Ok(Some(candidate))
    }
}

/// Reserve one charged unit per copied packed word before the fallible copy.
/// The primitive retains the exact original theory and adds no atom carrier.
fn copy(source: &Interpretation, budget: &mut Budget<'_>) -> Result<Interpretation, Incomplete> {
    let words = u64::try_from(source.theory().atom_count().div_ceil(64))
        .map_err(|_| Incomplete::CounterOverflow)?;
    budget.charge(words)?;
    source.try_clone().map_err(|error| match error {
        zetesis_ferraris::AdmissionError::Allocation => Incomplete::Allocation,
        _ => Incomplete::InvalidWitness,
    })
}
