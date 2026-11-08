//! Deferred local coordinates for a finite source-identity batch.
//!
//! Push authenticates and appends one occurrence without searching or shifting
//! an inverse. Finish consumes the private batch, sorts IDs using the existing
//! metered constant-scratch heapsort, compacts duplicates, and only then builds
//! the ordinary selection's inverse. No partially sorted/indexed selection is
//! observable. Published local coordinates follow increasing source IDs; their
//! order has no semantic role. Ordinary `SourceSelection` insertion is unchanged.
//!
//! R pushed occurrences use O(R) retained capacity and O(R log R) sorting work.
//! The distinct population, rather than the occurrence count, meets the caller's
//! atom ceiling. Every allocation, moved cell and comparison remains charged.

use super::{Computation, Counters, SourceAtom, SourceSelection, owner_failure};
use crate::formula::ceiling;
use crate::formula_support::{Context, GroundingWork, reserve, sort};
use crate::{FormulaFailure, FormulaLimits, FormulaResource, ProgramSite};

pub(crate) struct SourceBatch(SourceSelection);

impl SourceBatch {
    pub(super) const fn new(selection: SourceSelection) -> Self {
        Self(selection)
    }

    pub(crate) fn push(
        &mut self,
        atom: &SourceAtom,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        computation.source_atom(atom, limits, counters, location)?;
        counters.work(limits, location)?;
        if !self.0.scope.same(&atom.scope) {
            return Err(owner_failure(location));
        }
        reserve(
            &mut self.0.positions,
            1,
            &mut self.0.lease,
            size_of::<SourceSelection>(),
            Context::new(computation, limits, counters, location),
        )?;
        counters.work(limits, location)?;
        self.0.positions.push(atom.position);
        Ok(())
    }

    /// Publish only after complete sorting, compaction and inverse construction.
    /// Any refusal consumes and drops this private batch with its spent receipts.
    pub(crate) fn finish(
        mut self,
        bound: (FormulaResource, usize),
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<SourceSelection, FormulaFailure> {
        counters.work(limits, location)?;
        if !self.0.scope.same(&computation.source_scope(location)?) {
            return Err(owner_failure(location));
        }
        sort::by(
            &mut self.0.positions,
            GroundingWork::new(limits, counters, location),
            |left, right, _| Ok(left.cmp(right)),
        )?;
        let mut distinct = 0;
        for read in 0..self.0.positions.len() {
            counters.work(limits, location)?;
            let position = self.0.positions[read];
            if distinct == 0 || self.0.positions[distinct - 1] != position {
                counters.work(limits, location)?;
                self.0.positions[distinct] = position;
                distinct += 1;
            }
        }
        ceiling(bound.0, distinct as u128, bound.1 as u128, location)?;
        counters.work(limits, location)?;
        self.0.positions.truncate(distinct);
        reserve(
            &mut self.0.inverse,
            distinct,
            &mut self.0.lease,
            size_of::<SourceSelection>() + self.0.positions.capacity() * size_of::<usize>(),
            Context::new(computation, limits, counters, location),
        )?;
        for (local, &source) in self.0.positions.iter().enumerate() {
            counters.work(limits, location)?;
            self.0.inverse.push((source, local));
        }
        Ok(self.0)
    }
}

#[cfg(test)]
mod tests;
