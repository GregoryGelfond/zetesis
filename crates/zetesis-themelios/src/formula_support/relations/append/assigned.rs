//! Source identity admission and support selection are separate mutations.

use std::sync::Arc;

use crate::formula_support::GroundingWork;
use zetesis_core::PatternRef;
use zetesis_core::catalog::AssignmentSlice;

use super::{
    AtomRef, Counters, Failure, FormulaFailure, FormulaLimits, Location, Memory, SupportAppend,
    atom_failure, atom_interner, failure, owner_limits, record_owner_peak, size_of,
};

/// One evolving source authority, independent of every relation membership and
/// every checker's resource ledger. The token retains no logical payload.
#[derive(Clone, Debug)]
pub(crate) struct SourceScope(Arc<()>);

impl SourceScope {
    pub(in crate::formula_support) fn new() -> Self {
        Self(Arc::new(()))
    }

    pub(in crate::formula_support) fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// The shared strong/weak reference counters; inline handles are counted by
    /// their owners. Allocator bookkeeping is outside the named byte measure.
    pub(in crate::formula_support) const fn shared_bytes() -> usize {
        2 * size_of::<usize>()
    }
}

/// A discovered source atom is not evidence of support, truth or emission.
/// Retained selections store the scope once and retain only private positions.
#[derive(Clone, Debug)]
pub(crate) struct SourceAtom {
    pub(in crate::formula_support) scope: SourceScope,
    pub(in crate::formula_support) position: usize,
}

impl SupportAppend<'_> {
    /// Selected-slot lookup resolves discovery, then independently tests support.
    /// It cannot add rows or change the shared owner's retained/peak counters.
    pub(in crate::formula_support) fn contains_pattern(
        &self,
        pattern: PatternRef<'_>,
        values: AssignmentSlice<'_>,
        workspace: usize,
        work: GroundingWork<'_>,
    ) -> Result<bool, FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let outer = self.outer_bytes(workspace);
        let checked = owner_limits(limits, outer, location)?;
        let bytes = self.owner.pattern_lookup_bytes();
        // Record only an admitted envelope, before any callback can stop or
        // unwind. The core lookup applies these same checks in this order.
        if self.owner.len() <= checked.max_atoms && bytes <= checked.max_bytes {
            record_owner_peak(bytes, outer, counters);
        }
        let position = self
            .owner
            .find_pattern_with(pattern, values, checked, || counters.work(limits, location))
            .map_err(|error| match error {
                atom_interner::AssignedFailure::Assignment(error) => {
                    crate::formula_binding::assignment(error, location)
                }
                atom_interner::AssignedFailure::Interner(error) => {
                    atom_failure(error, limits, outer, location)
                }
            })?;
        counters.work(limits, location)?;
        Ok(position.is_some_and(|position| self.is_supported(position)))
    }

    pub(in crate::formula_support) fn signed(
        &self,
        atom: &SourceAtom,
        sign: zetesis_core::Sign,
        workspace: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<SourceAtom>, FormulaFailure> {
        let value = self.source_at(&atom.scope, atom.position, limits, counters, location)?;
        let outer = self.outer_bytes(workspace);
        let position = self
            .owner
            .find_signed_atom_with(value, sign, owner_limits(limits, outer, location)?, || {
                counters.work(limits, location)
            })
            .map_err(|error| atom_failure(error, limits, outer, location))?;
        Ok(position.map(|position| SourceAtom {
            scope: self.scope.clone(),
            position,
        }))
    }

    pub(in crate::formula_support) fn source_scope(&self) -> SourceScope {
        self.scope.clone()
    }

    pub(in crate::formula_support) fn source_at(
        &self,
        scope: &SourceScope,
        position: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<AtomRef<'_>, FormulaFailure> {
        counters.work(limits, location)?;
        if !self.scope.same(scope) {
            return Err(failure(Failure::Owner, location));
        }
        self.owner
            .get(position)
            .ok_or_else(|| failure(Failure::Owner, location))
    }

    pub(in crate::formula_support) fn discover_ref(
        &mut self,
        atom: AtomRef<'_>,
        workspace: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<SourceAtom, FormulaFailure> {
        let outer = self.outer_bytes(workspace);
        let checked = owner_limits(limits, outer, location)?;
        self.owner.restart_storage_peak();
        let result = self
            .owner
            .entry_atom_with(atom, checked, || counters.work(limits, location))
            .and_then(|entry| entry.insert_with(checked, || counters.work(limits, location)));
        let refreshed = self.refresh(workspace, limits, counters, location);
        let position = result.map_err(|error| atom_failure(error, limits, outer, location))?;
        refreshed?;
        Ok(SourceAtom {
            scope: self.scope.clone(),
            position,
        })
    }

    /// Only identity and the discovery map change. Complete components and
    /// reserved capacity may survive refusal; the live growth receipt always does.
    pub(in crate::formula_support) fn assigned(
        &mut self,
        pattern: PatternRef<'_>,
        values: AssignmentSlice<'_>,
        workspace: usize,
        work: GroundingWork<'_>,
    ) -> Result<SourceAtom, FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let outer = self.outer_bytes(workspace);
        let checked = owner_limits(limits, outer, location)?;
        self.owner.restart_storage_peak();
        // Each input already passed source term admission; the shared source
        // storage/work limits bound this tuple operation, without a new depth cap.
        let term_limits = zetesis_core::catalog::Limits {
            max_nodes: usize::MAX,
            max_depth: usize::MAX,
            max_bytes: usize::MAX,
        };
        let result = self
            .owner
            .insert_pattern_with(pattern, values, term_limits, checked, || {
                counters.work(limits, location)
            });
        let refreshed = self.refresh(workspace, limits, counters, location);
        let position = result.map_err(|error| match error {
            atom_interner::AssignedFailure::Assignment(error) => {
                crate::formula_binding::assignment(error, location)
            }
            atom_interner::AssignedFailure::Interner(error) => {
                atom_failure(error, limits, outer, location)
            }
        })?;
        refreshed?;
        Ok(SourceAtom {
            scope: self.scope.clone(),
            position,
        })
    }

    pub(super) fn is_supported(&self, position: usize) -> bool {
        self.supported
            .get(position / 64)
            .is_some_and(|word| word & (1_u64 << (position % 64)) != 0)
    }

    /// A single checked bit covers committed and pending support. Canonical
    /// discovery alone never sets it. Both buffers and all write work are admitted
    /// before the pending position and membership bit are published together.
    pub(in crate::formula_support) fn retain(
        &mut self,
        atom: &SourceAtom,
        workspace: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        counters.work(limits, location)?;
        if !self.scope.same(&atom.scope) || atom.position >= self.owner.len() {
            return Err(failure(Failure::Owner, location));
        }
        if self.is_supported(atom.position) {
            return Ok(());
        }
        self.owner.restart_storage_peak();
        let result = self.reserve_membership(atom.position, workspace, limits, counters, location);
        let refreshed = self.refresh(workspace, limits, counters, location);
        result?;
        refreshed?;
        // The final permit in reserve_membership covers these disjoint writes.
        self.pending.push(atom.position);
        self.supported[atom.position / 64] |= 1_u64 << (atom.position % 64);
        Ok(())
    }

    fn reserve_membership(
        &mut self,
        position: usize,
        workspace: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let bytes = usize::try_from(self.owner.storage_bytes() + self.outer_bytes(0))
            .map_err(|_| failure(Failure::Overflow, location))?;
        let mut memory = Memory::new(bytes, workspace as u128, limits, counters, location);
        let words = position / 64 + 1;
        let additional = words.saturating_sub(self.supported.len());
        if words > self.supported.capacity() {
            counters.charge_work(self.supported.len() as u128, limits, location)?;
            counters.work(limits, location)?;
        }
        memory.reserve(self.supported, additional)?;
        if self.pending.len() == self.pending.capacity() {
            counters.charge_work(self.pending.len() as u128, limits, location)?;
            counters.work(limits, location)?;
        }
        memory.reserve(self.pending, 1)?;
        counters.charge_work(additional as u128, limits, location)?;
        self.supported.resize(words.max(self.supported.len()), 0);
        counters.work(limits, location)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod lookup_tests;
