//! Runtime attribution carried by the existing regional receipt.
//!
//! These are observations, not allowances. No membership subject produces
//! them, and copying knowledge only contributes to the ordinary work field.

use zetesis_algebra::region::SubjectStep;
use zetesis_ferraris::ArithmeticAttempt;

use crate::Incomplete;

/// Private dimensions of `RegionCounts`, projected onto its public arithmetic
/// summaries. Actual record headers include this payload exactly once.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ArithmeticCounts {
    pub(crate) parity_passes: u64,
    pub(crate) parity_forced: u64,
    pub(crate) parity_refuted: u64,
    pub(crate) cardinality_passes: u64,
    pub(crate) cardinality_refuted: u64,
}

/// Attribution is fixed by the two concrete candidate-only adapter arms.
#[derive(Clone, Copy)]
pub(super) enum ArithmeticKind {
    Parity,
    Cardinality,
}

impl ArithmeticKind {
    /// Failed attempts have no completed batch, but retain real P1 decisions.
    /// A refutation completes the batch at its absorbing result. Bookkeeping
    /// neither polls nor debits the Meter after that result.
    pub(super) fn counts<E>(self, attempt: &ArithmeticAttempt<E>) -> ArithmeticCounts {
        let passes = u64::from(attempt.result.is_ok());
        let refuted = u64::from(matches!(&attempt.result, Ok(SubjectStep::Refuted)));
        match self {
            Self::Parity => ArithmeticCounts {
                parity_passes: passes,
                // Fresh applications partition the original atom positions;
                // their sum is bounded by this admitted attempt's word work.
                parity_forced: attempt.statistics.held + attempt.statistics.cut,
                parity_refuted: refuted,
                ..ArithmeticCounts::default()
            },
            Self::Cardinality => ArithmeticCounts {
                cardinality_passes: passes,
                cardinality_refuted: refuted,
                ..ArithmeticCounts::default()
            },
        }
    }
}

impl ArithmeticCounts {
    /// `RegionCounts` commits this together with all its other checked sums.
    pub(crate) fn checked_add(self, other: Self) -> Result<Self, Incomplete> {
        Ok(Self {
            parity_passes: self
                .parity_passes
                .checked_add(other.parity_passes)
                .ok_or(Incomplete::CounterOverflow)?,
            parity_forced: self
                .parity_forced
                .checked_add(other.parity_forced)
                .ok_or(Incomplete::CounterOverflow)?,
            parity_refuted: self
                .parity_refuted
                .checked_add(other.parity_refuted)
                .ok_or(Incomplete::CounterOverflow)?,
            cardinality_passes: self
                .cardinality_passes
                .checked_add(other.cardinality_passes)
                .ok_or(Incomplete::CounterOverflow)?,
            cardinality_refuted: self
                .cardinality_refuted
                .checked_add(other.cardinality_refuted)
                .ok_or(Incomplete::CounterOverflow)?,
        })
    }

    /// Existing returned multipass/live snapshots saturate their observations;
    /// they are never reused as a live search allowance or debited again.
    pub(crate) fn saturating_add(self, other: Self) -> Self {
        Self {
            parity_passes: self.parity_passes.saturating_add(other.parity_passes),
            parity_forced: self.parity_forced.saturating_add(other.parity_forced),
            parity_refuted: self.parity_refuted.saturating_add(other.parity_refuted),
            cardinality_passes: self
                .cardinality_passes
                .saturating_add(other.cardinality_passes),
            cardinality_refuted: self
                .cardinality_refuted
                .saturating_add(other.cardinality_refuted),
        }
    }
}

#[cfg(test)]
mod tests;
