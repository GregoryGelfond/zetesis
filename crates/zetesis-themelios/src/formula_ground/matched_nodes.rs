//! One per-traversal owner of nodes named from unchanged positive witnesses.
//!
//! The owner never crosses a rule, join, support snapshot, or Builder. Every
//! ordinary passing row names each held positive occurrence in literal order.
//! Thus a populated unchanged prefix belongs to the immediately preceding
//! passing row. Aggregate transactions append/detach only their new suffix;
//! already interned atomic nodes remain in the Builder's authoritative prefix.
//! Producer compiled groups never populate this per-atom cache.

#[cfg(test)]
pub(super) mod testing;
#[cfg(test)]
mod tests;

use super::Builder;
use crate::formula_support::{Buffer, witnesses::WitnessRow};
use crate::grounding_observer::Event;
use crate::{FormulaFailure, ProgramSite};

pub(super) struct MatchedNodes {
    nodes: Buffer<Option<usize>>,
    literal_depths: Option<Buffer<usize>>,
}

impl MatchedNodes {
    /// The producer's depth-ordered buffer keeps precisely its old allocation
    /// and initialization charges. No inverse is needed for that consumer.
    pub(super) fn new(
        width: usize,
        builder: &mut Builder<'_, '_, '_>,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let mut nodes = Buffer::new(
            builder.computation,
            builder.limits,
            builder.counters,
            location,
        )?;
        nodes.resize(
            width,
            None,
            builder.computation,
            builder.limits,
            builder.counters,
            location,
        )?;
        Ok(Self {
            nodes,
            literal_depths: None,
        })
    }

    pub(super) fn for_body(
        row: &WitnessRow<'_, '_>,
        literals: usize,
        builder: &mut Builder<'_, '_, '_>,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let width = row.sources().len();
        let mut result = Self::new(width, builder, location)?;
        let mut identity = width == literals;
        for (depth, source) in row.sources().enumerate() {
            builder.work(location)?;
            identity &= depth == source;
        }
        if !identity {
            let mut inverse = Buffer::new(
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?;
            inverse.resize(
                literals,
                usize::MAX,
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?;
            for (depth, source) in row.sources().enumerate() {
                builder.work(location)?;
                inverse.slice_mut()[source] = depth;
            }
            result.literal_depths = Some(inverse);
        }
        Ok(result)
    }

    /// A planned literal can be absent from an owned completion's record.
    /// Negative/non-planned literals and that deepest literal use general naming.
    pub(super) fn literal(
        &mut self,
        literal: usize,
        row: &WitnessRow<'_, '_>,
        builder: &mut Builder<'_, '_, '_>,
        location: ProgramSite,
    ) -> Result<Option<usize>, FormulaFailure> {
        let depth = self
            .literal_depths
            .as_ref()
            .map_or(literal, |inverse| inverse.slice()[literal]);
        if depth >= row.rows.len() {
            return Ok(None);
        }
        let reused = self.reusable(depth, row);
        let node = self.node(depth, row, builder, location)?;
        builder.counters.record(Event::RowAtom);
        if reused {
            builder.counters.record(Event::RowAtomReuse);
        }
        #[cfg(test)]
        testing::record(literal, depth, reused);
        Ok(Some(node))
    }

    fn reusable(&self, depth: usize, row: &WitnessRow<'_, '_>) -> bool {
        depth < row.unchanged && self.nodes.slice()[depth].is_some()
    }

    /// One dispatch unit also observes cancellation on the unchanged path.
    /// Producer callers transfer their existing input-dispatch charge here.
    pub(super) fn node(
        &mut self,
        depth: usize,
        row: &WitnessRow<'_, '_>,
        builder: &mut Builder<'_, '_, '_>,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        builder.work(location)?;
        self.dispatched(depth, row, builder, location)
    }

    /// Producer input dispatch already paid its unit before its existing test
    /// cancellation hook. Keep that boundary while sharing the prefix rule.
    pub(super) fn dispatched(
        &mut self,
        depth: usize,
        row: &WitnessRow<'_, '_>,
        builder: &mut Builder<'_, '_, '_>,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        if self.reusable(depth, row) {
            Ok(self.cached(depth))
        } else {
            self.refresh(depth, row, builder, location)
        }
    }

    /// A known changed suffix needs no second dispatch check. The store keeps
    /// the producer's existing unit; no counter or graph owner is copied here.
    pub(super) fn refresh(
        &mut self,
        depth: usize,
        row: &WitnessRow<'_, '_>,
        builder: &mut Builder<'_, '_, '_>,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        let node = builder.matched_atom(row.rows[depth], location)?;
        builder.work(location)?;
        self.nodes.slice_mut()[depth] = Some(node);
        Ok(node)
    }

    pub(super) fn cached(&self, depth: usize) -> usize {
        self.nodes.slice()[depth].expect("current selected positive atom")
    }
}
