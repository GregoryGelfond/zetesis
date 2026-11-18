//! Ground rows of one predicate, discovered as a batch.
//!
//! A program's fact block names one predicate for many rows. Its predicate is
//! authenticated and its column block located once, and room for every row
//! is reserved before the first is published, so no row pays a predicate
//! check, a column-block search or a buffer's geometric regrowth. Each row
//! still checks its own constants and finds or publishes its own identity.

use super::{
    AssignedFailure, AssignmentError, AssignmentSlice, AtomAppender, Failure, Limits, PatternRef,
    PredicateLocations, ProjectionSource, ReadError, TermLimits, storage,
};
use crate::catalog::PredicateRef;

/// One predicate authenticated for, and with rows reserved in, one writer.
/// Its column block is a candidate location, checked at each publication.
#[derive(Debug)]
pub struct PreparedFacts {
    owner: storage::AtomScope,
    predicate: storage::PredicateId,
    locations: PredicateLocations,
    /// Discovery positions' length and capacity before the reservation.
    pending: (usize, usize),
    store: storage::RowsStart,
}

impl AtomAppender<'_> {
    /// Authenticate `predicate` and reserve room for `rows` new atoms of it:
    /// discovery positions, its column block, the atom locators and the atom
    /// index's links, each at the capacity publishing the rows one at a time
    /// would reach, moving its live cells once. Hash tables still grow as
    /// rows arrive, by their own policy. [`Self::finish_facts_with`] gives
    /// back the room of rows found rather than published.
    ///
    /// # Errors
    /// Refuses an uninterned, foreign or inaccessible predicate, population or
    /// storage bounds, or caller work. Reserved capacity may remain; no
    /// identity or discovery is published.
    pub fn prepare_facts_with<E>(
        &mut self,
        predicate: PredicateRef<'_>,
        rows: usize,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<PreparedFacts, AssignedFailure<E>> {
        self.lookup().admit(0, limits)?;
        before().map_err(Failure::Stopped)?;
        let (source, id) =
            predicate
                .canonical()
                .ok_or(AssignedFailure::Assignment(AssignmentError::Read(
                    ReadError::Uninterned,
                )))?;
        before().map_err(Failure::Stopped)?;
        let read = storage::Read::from(&*self.store);
        if !read.same_vocabulary(source) {
            return Err(AssignedFailure::Assignment(AssignmentError::Read(
                ReadError::ForeignCatalog,
            )));
        }
        before().map_err(Failure::Stopped)?;
        if !read.contains_predicate(id) {
            return Err(AssignedFailure::Assignment(AssignmentError::Read(
                ReadError::OutsidePrefix,
            )));
        }
        let required = self.len().checked_add(rows).ok_or(Failure::Overflow)?;
        super::population(required, limits)?;
        let pending = (self.pending.len(), self.pending.capacity());
        let live = self.storage_bytes();
        {
            let mut checked = || before().map_err(Failure::Stopped);
            super::super::reserve(
                self.pending,
                rows,
                limits.max_atoms,
                live,
                self.peak,
                limits,
                &mut checked,
            )?;
        }
        let column = self
            .store
            .atom_column_location_with(id, &mut before)
            .map_err(Failure::Stopped)?;
        let arity = predicate.arity();
        let (block, store) = self.import_with_extra(limits, 0, |store| {
            store.reserve_atom_rows_with(id, arity, (column, rows), &mut before)
        })?;
        Ok(PreparedFacts {
            owner: self.store.atom_scope(),
            predicate: id,
            locations: PredicateLocations { column: block },
            pending,
            store,
        })
    }

    /// End a prepared block: give back the room its found rows did not use,
    /// so the capacities are those publishing its new rows one at a time
    /// would have left. Shrinking moves each live cell once.
    ///
    /// # Errors
    /// Refuses a different writer or caller work.
    pub fn finish_facts_with<E>(
        &mut self,
        facts: PreparedFacts,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        let PreparedFacts {
            owner,
            locations,
            pending: (len, capacity),
            store,
            ..
        } = facts;
        before().map_err(Failure::Stopped)?;
        if !storage::Read::from(&*self.store).accepts_atom_scope(&owner) {
            return Err(Failure::Catalog(crate::catalog::Error::Shape));
        }
        let target = if self.pending.len() <= capacity {
            capacity
        } else {
            super::super::grown(capacity, self.pending.len(), limits.max_atoms)
        };
        before().map_err(Failure::Stopped)?;
        if len <= self.pending.len() && self.pending.capacity() > target {
            for _ in 0..self.pending.len() {
                before().map_err(Failure::Stopped)?;
            }
            self.pending.shrink_to(target);
        }
        self.store
            .trim_atom_rows_with(locations.column, store, &mut before)
            .map_err(|error| match error {
                storage::Failure::Storage(error) => super::store_failure(error, 0, limits),
                storage::Failure::Stopped(error) => Failure::Stopped(error),
            })
    }

    /// Find or publish one row of `facts`: `pattern` must name the prepared
    /// predicate; its terms are checked as [`Self::insert_pattern_with`]
    /// checks them, under `values`' scope.
    ///
    /// # Errors
    /// Refuses a different writer or predicate, then whatever
    /// [`Self::insert_pattern_with`] refuses. No failed insertion publishes a
    /// discovery position.
    pub fn insert_fact_with<E>(
        &mut self,
        facts: &PreparedFacts,
        pattern: PatternRef<'_>,
        values: AssignmentSlice<'_>,
        (term_limits, limits): (TermLimits, Limits),
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, AssignedFailure<E>> {
        before().map_err(Failure::Stopped)?;
        let read = storage::Read::from(&*self.store);
        if !read.accepts_atom_scope(&facts.owner) {
            return Err(AssignedFailure::Assignment(AssignmentError::Read(
                ReadError::ForeignCatalog,
            )));
        }
        before().map_err(Failure::Stopped)?;
        let same = pattern
            .predicate()
            .canonical()
            .is_some_and(|(source, id)| id == facts.predicate && read.same_vocabulary(source));
        if !same {
            return Err(AssignedFailure::Assignment(AssignmentError::Read(
                ReadError::ForeignCatalog,
            )));
        }
        self.insert_projected_with(
            facts.predicate,
            values,
            ProjectionSource::Pattern(pattern.terms()),
            (term_limits, limits),
            Some(facts.locations),
            before,
        )
    }
}

#[cfg(test)]
mod tests;
