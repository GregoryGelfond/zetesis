//! Local atom coordinates select one source authority without copying payload.

mod batch;
pub(crate) use batch::SourceBatch;

use crate::ProgramSite;
use crate::formula_support::Context;
use zetesis_core::catalog::AtomRef;

use super::{Computation, Counters, GroundingWork, SourceAtom, SourceScope, StorageLease, reserve};
use crate::formula::ceiling;
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

/// First selection fixes the local coordinate. A sparse inverse identifies a
/// previously selected source atom; unrelated source discoveries occupy no
/// cells here. Neither selection nor canonical discovery establishes truth.
///
/// The inverse is an open-addressed table over local coordinates, keyed by
/// their source positions: each slot holds a local coordinate plus one, or
/// zero. Its length is zero or a power of two at least twice the selection,
/// so a lookup or insertion probes a constant expected number of slots and
/// growth rebuilds it from the coordinates in linear work.
pub(crate) struct SourceSelection {
    scope: SourceScope,
    positions: Vec<usize>,
    inverse: Vec<usize>,
    lease: StorageLease,
}

/// The fewest inverse slots a table may have once it is not empty.
const MIN_SLOTS: usize = 16;

/// The first slot `position` probes in an inverse of `slots` (a power of two).
fn home(position: usize, slots: usize) -> usize {
    let mask = u64::try_from(slots - 1).expect("slot index fits a word");
    let hash = zetesis_core::mix_word(0, u64::try_from(position).expect("position fits a word"));
    usize::try_from(hash & mask).expect("slot index is below the slot count")
}

impl SourceSelection {
    /// Collect source identities without assigning local coordinates. Finishing
    /// sorts and deduplicates the private batch before publishing a selection.
    pub(crate) fn batch(
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &Counters,
        location: ProgramSite,
    ) -> Result<SourceBatch, FormulaFailure> {
        Self::new(computation, limits, counters, location).map(SourceBatch::new)
    }

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
            .map(Result::ok)
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
        if let Ok(local) = self.search(atom, limits, counters, location)? {
            return Ok((local, false));
        }
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
            size_of::<Self>() + self.inverse.capacity() * size_of::<usize>(),
            Context::new(computation, limits, counters, location),
        )?;
        self.grow(1, Context::new(computation, limits, counters, location))?;
        let local = self.positions.len();
        let probes = self.probe(atom.position);
        counters.charge_work(probes as u128 + 2, limits, location)?;
        self.positions.push(atom.position);
        self.place(local);
        Ok((local, true))
    }

    /// Make room for `additional` more coordinates at half load. Growth
    /// reserves the larger table through the lease, then clears it and
    /// re-places every coordinate: one unit per slot and per coordinate.
    fn grow(
        &mut self,
        additional: usize,
        context: Context<'_, &Computation<'_, '_>>,
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
        let needed = self
            .positions
            .len()
            .checked_add(additional)
            .and_then(|count| count.checked_mul(2))
            .ok_or_else(|| overflow(location))?;
        if needed <= self.inverse.len() {
            return Ok(());
        }
        let slots = needed
            .max(MIN_SLOTS)
            .checked_next_power_of_two()
            .ok_or_else(|| overflow(location))?;
        let current = self.inverse.len();
        crate::formula_support::reserve_exact(
            &mut self.inverse,
            slots - current,
            &mut self.lease,
            size_of::<Self>() + self.positions.capacity() * size_of::<usize>(),
            Context::new(computation, limits, counters, location),
        )?;
        for _ in 0..slots + self.positions.len() {
            counters.work(limits, location)?;
        }
        self.inverse.clear();
        self.inverse.resize(slots, 0);
        for local in 0..self.positions.len() {
            self.place(local);
        }
        Ok(())
    }

    /// Re-place every coordinate after a renumbering, in the same table.
    fn rebuild(
        &mut self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        for _ in 0..self.inverse.len() + self.positions.len() {
            counters.work(limits, location)?;
        }
        self.inverse.fill(0);
        for local in 0..self.positions.len() {
            self.place(local);
        }
        Ok(())
    }

    /// The slots an insertion of `position` probes before its vacancy. The
    /// table has room and does not hold `position`.
    fn probe(&self, position: usize) -> usize {
        let mask = self.inverse.len() - 1;
        let mut slot = home(position, self.inverse.len());
        let mut probes = 1;
        while self.inverse[slot] != 0 {
            slot = (slot + 1) & mask;
            probes += 1;
        }
        probes
    }

    /// Place the already-recorded coordinate `local` in the first vacancy.
    fn place(&mut self, local: usize) {
        let mask = self.inverse.len() - 1;
        let mut slot = home(self.positions[local], self.inverse.len());
        while self.inverse[slot] != 0 {
            slot = (slot + 1) & mask;
        }
        self.inverse[slot] = local + 1;
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
        self.rebuild(limits, counters, location)?;
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

    /// The local coordinate of `atom`, or `Err(())` when it is not selected.
    /// One unit, then one per probed slot.
    fn search(
        &self,
        atom: &SourceAtom,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Result<usize, ()>, FormulaFailure> {
        counters.work(limits, location)?;
        if !self.scope.same(&atom.scope) {
            return Err(owner_failure(location));
        }
        if self.inverse.is_empty() {
            return Ok(Err(()));
        }
        let mask = self.inverse.len() - 1;
        let mut slot = home(atom.position, self.inverse.len());
        loop {
            counters.work(limits, location)?;
            match self.inverse[slot] {
                0 => return Ok(Err(())),
                entry if self.positions[entry - 1] == atom.position => return Ok(Ok(entry - 1)),
                _ => slot = (slot + 1) & mask,
            }
        }
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

fn overflow(location: ProgramSite) -> FormulaFailure {
    crate::formula_binding::assignment(
        zetesis_core::catalog::AssignmentError::Storage(zetesis_core::catalog::Error::Overflow),
        location,
    )
}

fn owner_failure(location: ProgramSite) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Owner,
        location,
    }
}

#[cfg(test)]
mod tests;
