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

impl SourceBatch {
    /// Merge this batch into an existing selection of the same source scope,
    /// visiting the local coordinate of every distinct batch atom: existing
    /// coordinates first, in increasing source order, then new ones as they
    /// are appended. The sparse inverse is extended by one backward linear
    /// merge rather than one shifting insertion per atom.
    ///
    /// R pushed occurrences against N selected atoms cost O(R log R) sorting,
    /// O(R log N) lookups and O(N + R) merging work, all charged. Both buffers
    /// are reserved before any coordinate is published; a refusal after
    /// reservation publishes nothing, though the selection may keep capacity.
    pub(crate) fn merge_into(
        mut self,
        selection: &mut SourceSelection,
        bound: (FormulaResource, usize),
        context: Context<'_, &Computation<'_, '_>>,
        mut visit: impl FnMut(usize, &mut Counters) -> Result<(), FormulaFailure>,
    ) -> Result<(), FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        counters.work(limits, location)?;
        if !self.0.scope.same(&selection.scope) {
            return Err(owner_failure(location));
        }
        sort::by(
            &mut self.0.positions,
            GroundingWork::new(limits, counters, location),
            |left, right, _| Ok(left.cmp(right)),
        )?;
        // Visit already selected atoms, and compact the absent distinct ones.
        let mut absent = 0;
        for read in 0..self.0.positions.len() {
            counters.work(limits, location)?;
            let position = self.0.positions[read];
            if absent != 0 && self.0.positions[absent - 1] == position {
                continue;
            }
            let atom = SourceAtom {
                scope: self.0.scope.clone(),
                position,
            };
            if let Ok(entry) = selection.search(&atom, limits, counters, location)? {
                // Repeated present positions are adjacent after sorting.
                if read == 0 || self.0.positions[read - 1] != position {
                    visit(selection.inverse[entry].1, counters)?;
                }
            } else {
                self.0.positions[absent] = position;
                absent += 1;
            }
        }
        self.0.positions.truncate(absent);
        let before = selection.positions.len();
        ceiling(
            bound.0,
            before as u128 + absent as u128,
            bound.1 as u128,
            location,
        )?;
        reserve(
            &mut selection.positions,
            absent,
            &mut selection.lease,
            size_of::<SourceSelection>()
                + selection.inverse.capacity() * size_of::<(usize, usize)>(),
            Context::new(computation, limits, counters, location),
        )?;
        reserve(
            &mut selection.inverse,
            absent,
            &mut selection.lease,
            size_of::<SourceSelection>() + selection.positions.capacity() * size_of::<usize>(),
            Context::new(computation, limits, counters, location),
        )?;
        counters.charge_work(
            selection.inverse.len() as u128 + 2 * absent as u128 + 1,
            limits,
            location,
        )?;
        // Publication: append coordinates, then merge the inverse from the back.
        selection.positions.extend_from_slice(&self.0.positions);
        let old = selection.inverse.len();
        selection.inverse.resize(old + absent, (0, 0));
        let (mut left, mut right, mut write) = (old, absent, old + absent);
        while right > 0 {
            write -= 1;
            let incoming = self.0.positions[right - 1];
            if left > 0 && selection.inverse[left - 1].0 > incoming {
                selection.inverse[write] = selection.inverse[left - 1];
                left -= 1;
            } else {
                selection.inverse[write] = (incoming, before + right - 1);
                right -= 1;
            }
        }
        for local in before..before + absent {
            visit(local, counters)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
