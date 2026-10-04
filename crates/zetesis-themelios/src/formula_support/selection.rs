//! Local atom coordinates select one source authority without copying payload.

use crate::ProgramSite;
use crate::formula_support::Context;
use zetesis_core::catalog::AtomRef;

use super::{Computation, Counters, SourceAtom, SourceScope, StorageLease, reserve};
use crate::formula::ceiling;
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

/// First selection fixes the local coordinate. A sparse inverse identifies a
/// previously selected source atom; unrelated source discoveries occupy no
/// cells here. Neither selection nor canonical discovery establishes truth.
pub(crate) struct SourceSelection {
    scope: SourceScope,
    positions: Vec<usize>,
    inverse: Vec<(usize, usize)>,
    lease: StorageLease,
}

impl SourceSelection {
    pub(crate) fn new(
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let scope = computation.source_scope(location)?;
        let mut lease = computation.lease();
        lease.observe(size_of::<Self>(), location)?;
        computation.storage_observed(&lease, 0, size_of::<Self>(), limits, counters, location)?;
        Ok(Self {
            scope,
            positions: Vec::new(),
            inverse: Vec::new(),
            lease,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.positions.len()
    }

    pub(crate) fn source(
        &self,
        position: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<SourceAtom, FormulaFailure> {
        counters.work(limits, location)?;
        let position = *self
            .positions
            .get(position)
            .ok_or_else(|| owner_failure(location))?;
        Ok(SourceAtom {
            scope: self.scope.clone(),
            position,
        })
    }

    pub(crate) fn position(
        &self,
        atom: &SourceAtom,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<usize>, FormulaFailure> {
        self.search(atom, limits, counters, location)
            .map(|found| found.ok().map(|entry| self.inverse[entry].1))
    }

    /// Both maps reserve and all relocation work is admitted before either
    /// membership write. A refused operation may retain capacity, never a
    /// half-published local coordinate. The boolean reports a new selection.
    pub(crate) fn insert(
        &mut self,
        atom: &SourceAtom,
        bound: (FormulaResource, usize),
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(usize, bool), FormulaFailure> {
        // Even an existing coordinate must authenticate the supplied reader.
        computation.source_atom(atom, limits, counters, location)?;
        let entry = match self.search(atom, limits, counters, location)? {
            Ok(entry) => return Ok((self.inverse[entry].1, false)),
            Err(entry) => entry,
        };
        ceiling(
            bound.0,
            self.positions.len() as u128 + 1,
            bound.1 as u128,
            location,
        )?;
        reserve(
            &mut self.positions,
            1,
            &mut self.lease,
            size_of::<Self>() + self.inverse.capacity() * size_of::<(usize, usize)>(),
            Context::new(computation, limits, counters, location),
        )?;
        reserve(
            &mut self.inverse,
            1,
            &mut self.lease,
            size_of::<Self>() + self.positions.capacity() * size_of::<usize>(),
            Context::new(computation, limits, counters, location),
        )?;
        counters.charge_work((self.inverse.len() - entry) as u128 + 2, limits, location)?;
        let local = self.positions.len();
        self.positions.push(atom.position);
        self.inverse.insert(entry, (atom.position, local));
        Ok((local, true))
    }

    pub(crate) fn atom<'read>(
        &self,
        position: usize,
        computation: &'read Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<AtomRef<'read>, FormulaFailure> {
        counters.work(limits, location)?;
        let source = self
            .positions
            .get(position)
            .ok_or_else(|| owner_failure(location))?;
        computation.source_at(&self.scope, *source, limits, counters, location)
    }

    /// Consume this local numbering and return semantic atom order. Sorting and
    /// rebuilding the sparse inverse use no replacement buffer. A refusal drops
    /// the consumed selection, so no partially renumbered map can escape.
    pub(crate) fn order(
        mut self,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        counters.work(limits, location)?;
        if !self.scope.same(&computation.source_scope(location)?) {
            return Err(owner_failure(location));
        }
        let len = self.positions.len();
        for root in (0..len / 2).rev() {
            self.sift(root, len, computation, limits, counters, location)?;
        }
        for end in (1..len).rev() {
            counters.work(limits, location)?;
            self.positions.swap(0, end);
            self.sift(0, end, computation, limits, counters, location)?;
        }
        for local in 0..len {
            let source = self.source(local, limits, counters, location)?;
            let entry = self
                .search(&source, limits, counters, location)?
                .map_err(|_| owner_failure(location))?;
            counters.work(limits, location)?;
            self.inverse[entry].1 = local;
        }
        Ok(self)
    }

    fn sift(
        &mut self,
        mut root: usize,
        end: usize,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        while root < end / 2 {
            let mut child = root * 2 + 1;
            if child + 1 < end
                && self.less(child, child + 1, computation, limits, counters, location)?
            {
                child += 1;
            }
            if !self.less(root, child, computation, limits, counters, location)? {
                break;
            }
            counters.work(limits, location)?;
            self.positions.swap(root, child);
            root = child;
        }
        Ok(())
    }

    fn less(
        &self,
        left: usize,
        right: usize,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        let left = self.atom(left, computation, limits, counters, location)?;
        let right = self.atom(right, computation, limits, counters, location)?;
        Ok(left
            .compare_ref_with(right, || counters.work(limits, location))?
            .is_lt())
    }

    fn search(
        &self,
        atom: &SourceAtom,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Result<usize, usize>, FormulaFailure> {
        counters.work(limits, location)?;
        if !self.scope.same(&atom.scope) {
            return Err(owner_failure(location));
        }
        let mut start = 0;
        let mut end = self.inverse.len();
        while start < end {
            counters.work(limits, location)?;
            let middle = start + (end - start) / 2;
            match self.inverse[middle].0.cmp(&atom.position) {
                std::cmp::Ordering::Less => start = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => return Ok(Ok(middle)),
            }
        }
        Ok(Err(start))
    }

    pub(super) fn storage_lease(&self) -> &StorageLease {
        &self.lease
    }

    pub(super) fn scope(&self) -> &SourceScope {
        &self.scope
    }
    pub(super) fn positions(&self) -> &[usize] {
        &self.positions
    }
}

fn owner_failure(location: ProgramSite) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Owner,
        location,
    }
}

#[cfg(test)]
mod tests;
