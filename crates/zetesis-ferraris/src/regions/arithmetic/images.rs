//! Optional original-root image closure; no evidence enters the reduct checker.
//!
//! Checked mandatory groups borrow immutable source bounds. Forbidden pairs
//! come from independently reread original clauses. All indices and working
//! masks are temporary; only a complete narrowed root or refutation survives.

pub(super) mod kernel;
pub(super) mod pairs;
mod storage;
mod transaction;

use zetesis_algebra::{
    Quota, Stop,
    words::{self, Words},
};

use super::{
    Budget, BudgetError, CardinalityStatistics, ParityStatistics, PreparationError,
    PreparationStatus, RootPreparation, Storage,
};
use crate::regions::adjacency::{Adjacency, Construction};
use crate::{Narrowing, Region, Theory};
use storage::{IndexConstruction, contains};

/// Exact original-root image preparation observations. No per-entry activity is included.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RootImageStatistics {
    /// Outcome of this image transaction, independently of earlier preparation.
    pub status: PreparationStatus,
    /// Authenticated unguarded mandatory support occurrences selected.
    pub groups: usize,
    /// Independently checked original forbidden-pair occurrences indexed.
    pub pairs: usize,
    /// Performed selection, source-checking and index construction work.
    pub preparation_work: u64,
    /// Performed work constructing and copying one private root.
    pub copying_work: u64,
    /// Performed pivot closure and final publication work.
    pub closure_work: u64,
    /// Shared logical requirement high-water through this attempt, including
    /// earlier live owners and refused requirements; not incremental bytes or RSS.
    pub peak_bytes: u128,
    /// Fully completed rounds; a partial interrupted or absorbing round is excluded.
    pub completed_rounds: u64,
    /// Cuts performed in the private root, including a discarded prefix.
    /// Publication is determined by `status` and the enclosing preparation result.
    pub applied_cuts: u64,
    /// A completed checked semantic contradiction, retained even if its later
    /// publication refuses. This is not a retained root or solve-completion claim;
    /// consult `status` and the enclosing preparation result for publication.
    pub root_refuted: bool,
}

/// Private experiment variants, not a public execution option or chosen default.
#[derive(Clone, Copy)]
pub(super) struct Policy {
    pub enabled: bool,
    pub retain_on_decline: bool,
}

// This matched local comparison uses T; no release policy is selected here.
pub(super) const POLICY: Policy = Policy {
    enabled: true,
    retain_on_decline: true,
};

pub(super) fn apply<Q: Quota + ?Sized>(
    result: Result<Option<RootPreparation>, PreparationError<zetesis_algebra::Failure<Q::Error>>>,
    theory: &Theory,
    policy: Policy,
    parity: &mut ParityStatistics,
    cardinality: &mut CardinalityStatistics,
    statistics: &mut RootImageStatistics,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<RootPreparation>, PreparationError<zetesis_algebra::Failure<Q::Error>>> {
    let Some(mut prepared) = result? else {
        return Ok(None);
    };
    if !policy.enabled {
        return Ok(Some(prepared));
    }
    if prepared.root_refuted() {
        return Ok(Some(prepared));
    }
    let outcome = transaction::run(theory, &mut prepared, statistics, budget);
    statistics.peak_bytes = budget.peak_bytes();
    match outcome {
        Ok(entered) => {
            statistics.status = if entered {
                PreparationStatus::Prepared
            } else {
                PreparationStatus::NotPrepared
            };
            Ok(Some(prepared))
        }
        Err(error) => {
            statistics.status = status(&error);
            let optional = matches!(
                error,
                BudgetError::WorkDeclined | BudgetError::StorageDeclined { .. }
            );
            if optional && policy.retain_on_decline {
                return Ok(Some(prepared));
            }
            parity.status = statistics.status;
            cardinality.status = statistics.status;
            parity.retained_bytes = 0;
            cardinality.retained_bytes = 0;
            match error {
                BudgetError::WorkDeclined | BudgetError::StorageDeclined { .. } => Ok(None),
                BudgetError::Stopped(stop) => Err(PreparationError::Stopped(
                    zetesis_algebra::Failure::Stopped(stop),
                )),
                BudgetError::Refused(error) => Err(PreparationError::Stopped(
                    zetesis_algebra::Failure::Refused(error),
                )),
                BudgetError::Source(error) => Err(PreparationError::Source(error)),
            }
        }
    }
}

fn status<E>(error: &BudgetError<E>) -> PreparationStatus {
    match error {
        BudgetError::WorkDeclined => PreparationStatus::WorkDeclined,
        BudgetError::StorageDeclined { .. } => PreparationStatus::StorageDeclined,
        BudgetError::Stopped(_) | BudgetError::Refused(_) | BudgetError::Source(_) => {
            PreparationStatus::Incomplete
        }
    }
}
