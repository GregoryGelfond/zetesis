//! Ordinal extrema reduction over already-computed predecessor truth.
//!
//! A row's ranks belong to one guard-independent ordered carrier. Rank zero
//! represents its least endpoint and `rank_count - 1` its greatest endpoint.
//! Typed value ordering and the preservation of guard comparisons are separate
//! preparation obligations; this primitive does not compare source values.

use zetesis_cpu::Stop;

#[cfg(test)]
mod tests;

/// A borrowed ordinal row, retaining every original condition occurrence.
///
/// Columns have equal lengths, `rank_count` is at least two, and each rank is
/// smaller than `rank_count`. Repeated conditions may occur at different ranks;
/// reducing both extrema must retain all those occurrences. Child identity and
/// preceding-node validation belong to the enclosing formula owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExtremumRow<'a> {
    /// Ordered formula child IDs, one per contribution occurrence.
    pub conditions: &'a [usize],
    /// Ordinal ranks in the same occurrence order.
    pub ranks: &'a [u64],
    /// Size of this row's ordered carrier, including both endpoints.
    pub rank_count: u64,
}

/// An ordinal boundary tested against the corresponding selected extremum.
///
/// A boundary lies in `0..=rank_count`. Formula admission validates that range;
/// a rule and a measure must refer to the same row's carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExtremumCutRule {
    /// The selected minimum is strictly smaller than this boundary.
    Prefix(u64),
    /// The selected maximum is at least this boundary.
    Suffix(u64),
}

/// A borrowed cut descriptor and its authoritative ordinal row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExtremumCut<'a> {
    /// Contribution occurrences and their row-local rank authority.
    pub row: ExtremumRow<'a>,
    /// Validated boundary in this exact row's carrier.
    pub rule: &'a ExtremumCutRule,
}

/// Both selected extrema within one row's ordered carrier.
///
/// The empty selection has minimum `rank_count - 1` and maximum zero. These are
/// the actual endpoint ranks, so an empty selection and a selected endpoint can
/// agree on one extremum without agreeing on the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExtremaMeasure {
    /// Least selected rank, or the greatest endpoint for an empty selection.
    pub minimum: u64,
    /// Greatest selected rank, or the least endpoint for an empty selection.
    pub maximum: u64,
}

impl ExtremumRow<'_> {
    /// Reduce both extrema from predecessor truth without allocation.
    ///
    /// A successful call reads every occurrence exactly once in row order,
    /// including repeated conditions and either endpoint rank. It does not stop
    /// after finding either extreme. The callback owns truth acquisition and
    /// work/cancellation charges, including validation of child IDs. This
    /// operation retains two ranks and performs linear work in the row length.
    /// A frozen consumer supplies reduced predecessor truth and separately
    /// applies the original cut's truth gate; this primitive does neither.
    ///
    /// # Errors
    /// Propagates callback errors without further reads. Unequal column lengths,
    /// a carrier smaller than two, or any out-of-carrier rank (even for a false
    /// condition) returns [`Stop::InvalidProgram`]. No partial measure is returned.
    #[inline]
    pub fn reduce<E: From<Stop>>(
        self,
        mut read: impl FnMut(usize) -> Result<bool, E>,
    ) -> Result<ExtremaMeasure, E> {
        if self.conditions.len() != self.ranks.len() || self.rank_count < 2 {
            return Err(Stop::InvalidProgram.into());
        }
        let mut measure = ExtremaMeasure {
            minimum: self.rank_count - 1,
            maximum: 0,
        };
        for (&condition, &rank) in self.conditions.iter().zip(self.ranks) {
            if rank >= self.rank_count {
                return Err(Stop::InvalidProgram.into());
            }
            if read(condition)? {
                measure.minimum = measure.minimum.min(rank);
                measure.maximum = measure.maximum.max(rank);
            }
        }
        Ok(measure)
    }
}

impl ExtremumCutRule {
    /// Test this boundary in constant time against a measure from the same row.
    ///
    /// The caller validates `boundary <= rank_count` and carrier identity. This
    /// operation does not validate an arbitrary externally constructed measure.
    #[must_use]
    pub const fn holds(self, measure: ExtremaMeasure) -> bool {
        match self {
            Self::Prefix(boundary) => measure.minimum < boundary,
            Self::Suffix(boundary) => measure.maximum >= boundary,
        }
    }
}

impl ExtremumCut<'_> {
    /// Recognize only endpoint-bound constants in an admitted carrier.
    ///
    /// The caller establishes `rank_count >= 2` and a boundary in
    /// `0..=rank_count`. Interior cuts, including rows with no qualifying
    /// occurrence, need a caller-metered witness scan before they can be
    /// classified as constant. This does not inspect conditions or apply a
    /// frozen original-node gate.
    #[must_use]
    pub fn endpoint_truth(self) -> Option<bool> {
        match *self.rule {
            ExtremumCutRule::Prefix(0) => Some(false),
            ExtremumCutRule::Suffix(0) => Some(true),
            ExtremumCutRule::Prefix(boundary) if boundary == self.row.rank_count => Some(true),
            ExtremumCutRule::Suffix(boundary) if boundary == self.row.rank_count => Some(false),
            _ => None,
        }
    }

    /// Select one qualifying witness from this exact row's admitted carrier.
    ///
    /// The caller establishes `rank < rank_count` and a valid cut boundary.
    /// Handle [`Self::endpoint_truth`] first: true endpoint cuts need no witness,
    /// so this predicate alone does not characterize whole-cut truth. A frozen
    /// consumer still applies the original cut gate separately.
    #[must_use]
    pub fn qualifies(self, rank: u64) -> bool {
        match *self.rule {
            ExtremumCutRule::Prefix(boundary) => rank < boundary,
            ExtremumCutRule::Suffix(boundary) => rank >= boundary,
        }
    }
}

#[cfg(test)]
mod cpu_tests;
