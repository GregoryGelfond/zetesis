//! Source identity admission and support selection are separate mutations.

use std::sync::Arc;

use crate::formula_support::GroundingWork;
use zetesis_core::PatternRef;
use zetesis_core::atom_interner::{PreparedFacts, PreparedPattern, PreparedRows, RowColumn};
use zetesis_core::catalog::{AssignmentSlice, PredicateRef};
use zetesis_core::relation::{Relation, Row};

use super::{
    AtomRef, Counters, Failure, FormulaFailure, FormulaLimits, Memory, ProgramSite, SupportAppend,
    atom_failure, atom_interner, failure, owner_limits, record_owner_peak, size_of,
};

/// A fact block prepared at the owner, and where support's pending
/// selection stood when its rows were reserved there.
#[derive(Debug)]
pub(crate) struct FactRows {
    facts: PreparedFacts,
    pending: (usize, usize),
}

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
        self.supported_position(pattern, values, workspace, work)
            .map(|position| position.is_some())
    }

    /// The identity of the atom `pattern` names under `values` when it is
    /// supported, published or pending: the lookup of
    /// [`Self::contains_pattern`], with the same charges, keeping what it
    /// found.
    pub(in crate::formula_support) fn supported_pattern(
        &self,
        pattern: PatternRef<'_>,
        values: AssignmentSlice<'_>,
        workspace: usize,
        work: GroundingWork<'_>,
    ) -> Result<Option<SourceAtom>, FormulaFailure> {
        Ok(self
            .supported_position(pattern, values, workspace, work)?
            .map(|position| SourceAtom {
                scope: self.scope.clone(),
                position,
            }))
    }

    fn supported_position(
        &self,
        pattern: PatternRef<'_>,
        values: AssignmentSlice<'_>,
        workspace: usize,
        work: GroundingWork<'_>,
    ) -> Result<Option<usize>, FormulaFailure> {
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
        Ok(position.filter(|&position| self.is_supported(position)))
    }

    pub(in crate::formula_support) fn signed(
        &self,
        atom: &SourceAtom,
        sign: zetesis_core::Sign,
        workspace: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
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
        location: ProgramSite,
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
        location: ProgramSite,
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
        let term_limits = source_term_limits();
        let result = self
            .owner
            .insert_pattern_with(pattern, values, term_limits, checked, || {
                counters.work(limits, location)
            });
        let refreshed = self.refresh(workspace, limits, counters, location);
        let position = result.map_err(|error| assigned_failure(error, limits, outer, location))?;
        refreshed?;
        Ok(SourceAtom {
            scope: self.scope.clone(),
            position,
        })
    }

    /// Authenticate a fact block's predicate once and reserve its rows:
    /// their atoms, at the owner, and their support membership.
    pub(in crate::formula_support) fn prepare_facts(
        &mut self,
        predicate: PredicateRef<'_>,
        rows: usize,
        workspace: usize,
        work: GroundingWork<'_>,
    ) -> Result<FactRows, FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let outer = self.outer_bytes(workspace);
        let checked = owner_limits(limits, outer, location)?;
        self.owner.restart_storage_peak();
        let result = self
            .owner
            .prepare_facts_with(predicate, rows, checked, || counters.work(limits, location));
        let refreshed = self.refresh(workspace, limits, counters, location);
        let facts = result.map_err(|error| assigned_failure(error, limits, outer, location))?;
        refreshed?;
        // Support membership: each row may enter the pending selection once.
        let pending = (self.pending.len(), self.pending.capacity());
        let bytes = usize::try_from(self.owner.storage_bytes() + self.outer_bytes(0))
            .map_err(|_| failure(Failure::Overflow, location))?;
        let mut memory = Memory::new(bytes, workspace as u128, limits, counters, location);
        let needed = pending
            .0
            .checked_add(rows)
            .ok_or_else(|| failure(Failure::Overflow, location))?;
        if needed > pending.1 {
            counters.charge_work(pending.0 as u128, limits, location)?;
            counters.work(limits, location)?;
        }
        memory.reserve_rows(self.pending, rows)?;
        Ok(FactRows { facts, pending })
    }

    /// End a prepared fact block: give back the room its rows found rather
    /// than published did not use, as [`Self::prepare_facts`] measured it.
    pub(in crate::formula_support) fn finish_facts(
        &mut self,
        rows: FactRows,
        workspace: usize,
        work: GroundingWork<'_>,
    ) -> Result<(), FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let outer = self.outer_bytes(workspace);
        let checked = owner_limits(limits, outer, location)?;
        let result = self
            .owner
            .finish_facts_with(rows.facts, checked, || counters.work(limits, location));
        result.map_err(|error| atom_failure(error, limits, outer, location))?;
        let (len, capacity) = rows.pending;
        let mut target = capacity;
        while target < self.pending.len() {
            target = target.saturating_mul(2).max(target + 1);
        }
        counters.work(limits, location)?;
        if len <= self.pending.len() && self.pending.capacity() > target {
            counters.charge_work(self.pending.len() as u128, limits, location)?;
            self.pending.shrink_to(target);
        }
        self.refresh(workspace, limits, counters, location)
    }

    /// [`Self::assigned`] for one row of a prepared fact block.
    pub(in crate::formula_support) fn fact_assigned(
        &mut self,
        rows: &FactRows,
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
        let result = self.owner.insert_fact_with(
            &rows.facts,
            pattern,
            values,
            (source_term_limits(), checked),
            || counters.work(limits, location),
        );
        let refreshed = self.refresh(workspace, limits, counters, location);
        let position = result.map_err(|error| assigned_failure(error, limits, outer, location))?;
        refreshed?;
        Ok(SourceAtom {
            scope: self.scope.clone(),
            position,
        })
    }

    /// Prepare `rows` heads of `predicate` as one fact block, for a run of
    /// fact rules that follow one another. Any open block is ended first.
    pub(in crate::formula_support) fn open_head_block(
        &mut self,
        predicate: PredicateRef<'_>,
        rows: usize,
        workspace: usize,
        work: GroundingWork<'_>,
    ) -> Result<(), FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        self.close_head_block(workspace, GroundingWork::new(limits, counters, location))?;
        let rows = self.prepare_facts(
            predicate,
            rows,
            workspace,
            GroundingWork::new(limits, counters, location),
        )?;
        self.head_block = Some(rows);
        Ok(())
    }

    /// End the open head block, giving back the room its unpublished rows left.
    pub(in crate::formula_support) fn close_head_block(
        &mut self,
        workspace: usize,
        work: GroundingWork<'_>,
    ) -> Result<(), FormulaFailure> {
        self.head_rule = None;
        match self.head_block.take() {
            Some(rows) => self.finish_facts(rows, workspace, work),
            None => Ok(()),
        }
    }

    /// The fact rule `index` comes next in the open head block: its head's
    /// atom is recorded, when a cell for it was reserved.
    pub(in crate::formula_support) fn expect_head(&mut self, index: usize) {
        self.head_rule = self.head_block.as_ref().map(|_| index);
    }

    /// Whether a head of `pattern`'s predicate enters the open head block.
    pub(in crate::formula_support) fn in_head_block(&self, pattern: PatternRef<'_>) -> bool {
        self.head_block
            .as_ref()
            .is_some_and(|rows| self.owner.prepared_for(&rows.facts, pattern.predicate()))
    }

    /// [`Self::assigned`] for one head of the open head block.
    pub(in crate::formula_support) fn head_block_assigned(
        &mut self,
        pattern: PatternRef<'_>,
        values: AssignmentSlice<'_>,
        workspace: usize,
        work: GroundingWork<'_>,
    ) -> Result<SourceAtom, FormulaFailure> {
        let location = work.location;
        let rows = self
            .head_block
            .take()
            .ok_or_else(|| failure(Failure::Owner, location))?;
        let result = self.fact_assigned(&rows, pattern, values, workspace, work);
        self.head_block = Some(rows);
        if let (Ok(atom), Some(rule), Some(heads)) = (
            &result,
            self.head_rule.take(),
            self.head_rows.as_deref_mut(),
        ) && heads.len() < heads.capacity()
            && heads.last().is_none_or(|&(last, _)| last < rule)
        {
            heads.push((rule, atom.position));
        }
        result
    }

    /// Source templates and their constants already passed source admission.
    /// Preparation retains only the caller-accounted borrowed header.
    pub(in crate::formula_support) fn prepare_pattern<'pattern>(
        &self,
        pattern: PatternRef<'pattern>,
        workspace: usize,
        work: GroundingWork<'_>,
    ) -> Result<Option<PreparedPattern<'pattern>>, FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let outer = self.outer_bytes(workspace);
        self.owner
            .prepare_pattern_with(
                pattern,
                source_term_limits(),
                owner_limits(limits, outer, location)?,
                || counters.work(limits, location),
            )
            .map_err(|error| assigned_failure(error, limits, outer, location))
    }

    /// Projection metadata already has its source lease. Core preparation
    /// consumes the vectors without allocating or counting that lease again.
    pub(in crate::formula_support) fn prepare_rows<'rows, 'source>(
        &self,
        pattern: PatternRef<'source>,
        sources: Vec<&'rows Relation<'source>>,
        columns: Vec<Option<RowColumn>>,
        workspace: usize,
        work: GroundingWork<'_>,
    ) -> Result<Option<PreparedRows<'rows, 'source>>, FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let outer = self.outer_bytes(workspace);
        self.owner
            .prepare_row_pattern_with(
                pattern,
                sources,
                columns,
                source_term_limits(),
                owner_limits(limits, outer, location)?,
                || counters.work(limits, location),
            )
            .map_err(|error| assigned_failure(error, limits, outer, location))
    }

    pub(in crate::formula_support) fn rows_assigned(
        &mut self,
        pattern: &PreparedRows<'_, '_>,
        rows: &[Row<'_, '_>],
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
        let result = self
            .owner
            .insert_prepared_rows_with(pattern, rows, checked, || counters.work(limits, location));
        let refreshed = self.refresh(workspace, limits, counters, location);
        let position = result.map_err(|error| assigned_failure(error, limits, outer, location))?;
        refreshed?;
        Ok(SourceAtom {
            scope: self.scope.clone(),
            position,
        })
    }

    pub(in crate::formula_support) fn prepared_assigned(
        &mut self,
        pattern: &PreparedPattern<'_>,
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
        let result = self
            .owner
            .insert_prepared_pattern_with(pattern, values, checked, || {
                counters.work(limits, location)
            });
        let refreshed = self.refresh(workspace, limits, counters, location);
        let position = result.map_err(|error| assigned_failure(error, limits, outer, location))?;
        refreshed?;
        Ok(SourceAtom {
            scope: self.scope.clone(),
            position,
        })
    }

    pub(super) fn is_supported(&self, position: usize) -> bool {
        self.supported.get_bit(position).unwrap_or(false)
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
        location: ProgramSite,
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
        self.supported.insert(atom.position);
        Ok(())
    }

    fn reserve_membership(
        &mut self,
        position: usize,
        workspace: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
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
        memory.reserve(self.supported.storage_mut(), additional)?;
        if self.pending.len() == self.pending.capacity() {
            counters.charge_work(self.pending.len() as u128, limits, location)?;
            counters.work(limits, location)?;
        }
        memory.reserve(self.pending, 1)?;
        counters.charge_work(additional as u128, limits, location)?;
        let initialized = words.max(self.supported.len());
        self.supported.storage_mut().resize(initialized, 0);
        counters.work(limits, location)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod lookup_tests;

fn source_term_limits() -> zetesis_core::catalog::Limits {
    // Each input passed source admission. Do not impose a second logical cap.
    zetesis_core::catalog::Limits {
        max_nodes: usize::MAX,
        max_depth: usize::MAX,
        max_bytes: usize::MAX,
    }
}

fn assigned_failure(
    error: atom_interner::AssignedFailure<FormulaFailure>,
    limits: &FormulaLimits,
    outer: u128,
    location: ProgramSite,
) -> FormulaFailure {
    match error {
        atom_interner::AssignedFailure::Assignment(error) => {
            crate::formula_binding::assignment(error, location)
        }
        atom_interner::AssignedFailure::Interner(error) => {
            atom_failure(error, limits, outer, location)
        }
    }
}
