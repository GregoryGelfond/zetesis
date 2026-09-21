//! One cumulative admission ceiling shared by independent constraint checkers.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use themelios_base::span::Location;

use crate::{
    ConstraintCheckLimits, ConstraintCheckStatistics, ExpansionFailure, ExpansionResource,
    FormulaFailure, FormulaResource,
};

/// Shared cumulative work, substitution and copied-scalar allowances.
///
/// Clones share the same monotone counters. Each charge is admitted before its
/// operation; stopping or dropping a checker does not refund accepted charges.
/// No limit is multiplied by the number of workers. Separate checkers retain
/// independent join/evaluation state and their own local receipts.
#[derive(Clone, Debug)]
pub struct ConstraintAllowance(Arc<Shared>);

#[derive(Debug)]
struct Shared {
    limits: ConstraintCheckLimits,
    work: AtomicU64,
    substitutions: AtomicU64,
    scalar_bytes: AtomicU64,
}

impl ConstraintAllowance {
    /// Start a finite allowance shared by all checkers attached to its clones.
    #[must_use]
    pub fn new(limits: ConstraintCheckLimits) -> Self {
        Self(Arc::new(Shared {
            limits,
            work: AtomicU64::new(0),
            substitutions: AtomicU64::new(0),
            scalar_bytes: AtomicU64::new(0),
        }))
    }

    /// The same configured ceilings apply to every attached checker.
    #[must_use]
    pub fn limits(&self) -> ConstraintCheckLimits {
        self.0.limits
    }

    /// Total accepted charges, including preparation and interrupted prefixes.
    /// After workers join this is exact. During concurrent checks each field is
    /// a monotone observation, not a simultaneous snapshot of all three fields.
    #[must_use]
    pub fn statistics(&self) -> ConstraintCheckStatistics {
        self.0.receipt()
    }

    pub(crate) fn work(&self, amount: u128, location: Location) -> Result<(), FormulaFailure> {
        reserve(&self.0.work, amount, u128::from(self.0.limits.max_work)).map_err(|observed| {
            FormulaFailure::Limit {
                resource: FormulaResource::Work,
                limit: u128::from(self.0.limits.max_work),
                observed,
                location,
            }
        })
    }

    pub(crate) fn substitution(&self, location: Location) -> Result<(), FormulaFailure> {
        reserve(
            &self.0.substitutions,
            1,
            u128::from(self.0.limits.max_substitutions),
        )
        .map_err(|observed| FormulaFailure::Limit {
            resource: FormulaResource::Substitutions,
            limit: u128::from(self.0.limits.max_substitutions),
            observed,
            location,
        })
    }

    pub(crate) fn scalar(&self, amount: u128, location: Location) -> Result<(), ExpansionFailure> {
        reserve(
            &self.0.scalar_bytes,
            amount,
            self.0.limits.max_scalar_bytes as u128,
        )
        .map_err(|observed| ExpansionFailure::Limit {
            resource: ExpansionResource::ScalarBytes,
            limit: self.0.limits.max_scalar_bytes as u128,
            observed,
            location,
        })
    }
}

impl Shared {
    fn receipt(&self) -> ConstraintCheckStatistics {
        ConstraintCheckStatistics {
            work: self.work.load(Ordering::Relaxed),
            substitutions: self.substitutions.load(Ordering::Relaxed),
            scalar_bytes: usize::try_from(self.scalar_bytes.load(Ordering::Relaxed))
                .expect("scalar allowance is bounded by usize"),
        }
    }
}

/// Only counters synchronize here, never program state or verdict publication.
/// Failed reservations change nothing; a successful reservation precedes the
/// caller's infallible local increment and the charged operation.
fn reserve(counter: &AtomicU64, amount: u128, limit: u128) -> Result<(), u128> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
            let observed = u128::from(used).saturating_add(amount);
            (observed <= limit).then(|| u64::try_from(observed).expect("bounded u64 allowance"))
        })
        .map(|_| ())
        .map_err(|used| u128::from(used).saturating_add(amount))
}
