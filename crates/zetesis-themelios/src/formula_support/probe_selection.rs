//! Completed necessary selection of one short, immutable indexed posting.
//!
//! The join owns one receipt per original positive occurrence. Its resolved
//! relation and immutable row filter remain borrowed for that join's lifetime.
//! Bits denote positions in the exact borrowed posting, never atom identities.
//! Matching, scalar evaluation and positive-row evidence are not retained here.

use super::{
    Computation, Context, Counters, GroundingWork, Join, PatternOccurrence, Probe, RowFilter,
    delta, reserve_exact,
};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

#[derive(Default)]
pub(super) struct Receipt<'a> {
    posting: Option<&'a [usize]>,
    selected: u64,
    covered: usize,
    permits_open: bool,
    complete: bool,
    replay: bool,
}

impl<'a> Receipt<'a> {
    /// Only an exhausted traversal of this exact posting and eligibility mode
    /// can be replayed. A different probe discards the previous evidence.
    pub(super) fn start(&mut self, probe: &Probe<'a, '_>, permits_open: bool) {
        let Probe::Indexed(delta::Rows::Posting(posting)) = probe else {
            *self = Self::default();
            return;
        };
        if !(2..=u64::BITS as usize).contains(&posting.len()) {
            *self = Self::default();
            return;
        }
        if self.complete
            && self.permits_open == permits_open
            && self
                .posting
                .is_some_and(|previous| std::ptr::eq(previous, *posting))
        {
            self.replay = true;
        } else {
            *self = Self {
                posting: Some(posting),
                permits_open,
                ..Self::default()
            };
        }
    }

    pub(super) fn next(
        &self,
        probe: &Probe<'_, '_>,
        position: &mut usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<usize>, FormulaFailure> {
        if !self.replay {
            return probe.next(position, limits, counters, location);
        }
        counters.work(limits, location)?;
        let posting = self.posting.expect("completed posting retained");
        if *position >= posting.len() {
            return Ok(None);
        }
        let remaining = self.selected & (u64::MAX << *position);
        if remaining == 0 {
            *position = posting.len();
            return Ok(None);
        }
        let selected = remaining.trailing_zeros() as usize;
        *position = selected + 1;
        Ok(Some(posting[selected]))
    }

    /// Record only a successful necessary-selection decision, before matching.
    /// A fault leaves a gap in coverage, so retrying cannot certify that probe.
    pub(super) fn record(&mut self, position: usize, permitted: bool) {
        if !self.replay
            && self.posting.is_some_and(|posting| position < posting.len())
            && position == self.covered
        {
            if permitted {
                self.selected |= 1 << position;
            }
            self.covered += 1;
        }
    }

    pub(super) fn finish(&mut self) {
        if !self.replay {
            self.complete = self
                .posting
                .is_some_and(|posting| self.covered == posting.len());
        }
    }
}

impl<'a> Join<'a, '_> {
    /// Advance the current occurrence's authenticated source. Completed
    /// necessary selection can omit rows; every offered row still belongs to
    /// the live probe, and matching remains the enclosing join's responsibility.
    pub(super) fn advance_probe(
        &mut self,
        pattern: PatternOccurrence<'a>,
        context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Option<usize>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let preparing = self.probes[self.depth].is_none();
        self.prepare_probe(
            pattern,
            Context::new(computation, limits, counters, location),
        )?;
        let probe = self.probes[self.depth].as_ref().expect("prepared probe");
        let Some(selection) = self.probe_selections.get_mut(self.depth) else {
            return probe.next(&mut self.positions[self.depth], limits, counters, location);
        };
        if preparing {
            selection.start(
                probe,
                self.positive_prefix
                    .as_ref()
                    .is_none_or(super::rows::PositivePrefix::permits_open),
            );
        }
        let row = selection.next(
            probe,
            &mut self.positions[self.depth],
            limits,
            counters,
            location,
        )?;
        if row.is_none() {
            selection.finish();
        }
        Ok(row)
    }

    /// The metadata is optional. Decline a capacity proposal that would displace
    /// the existing join; once allocation starts, its actual capacity and any
    /// failure retain the ordinary typed accounting boundary.
    pub(super) fn prepare_selections(
        &mut self,
        context: Context<'_, &super::Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        if self.plan.patterns.len() < 2
            || !self.row_filter.is_some_and(RowFilter::immutable_selection)
        {
            return Ok(());
        }
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let count = self.plan.patterns.len();
        let other = self.storage_bytes();
        let proposed = other as u128 + count as u128 * size_of::<Receipt<'_>>() as u128;
        if proposed > computation.allowance(&self.lease, limits, location)? as u128 {
            return Ok(());
        }
        reserve_exact(
            &mut self.probe_selections,
            count,
            &mut self.lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        for _ in 0..count {
            counters.work(limits, location)?;
            self.probe_selections.push(Receipt::default());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
