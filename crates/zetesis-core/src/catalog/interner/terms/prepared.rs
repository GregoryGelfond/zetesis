//! Repeated instantiation borrows the original admitted template columns.

use super::{
    AssignedFailure, AssignmentError, AssignmentSlice, AtomAppender, Failure, Limits, PatternRef,
    PredicateLocations, Projected, ProjectionSource, ReadError, TermLimits, projected_predicate,
    projected_scope, storage,
};
use crate::template::catalog::{PatternData, TermData};

mod rows;
pub use rows::{PreparedRows, RowColumn};

/// One admitted pattern bound to an exact evolving atom writer and fixed logical
/// term limits. This capability borrows the source's immutable argument metadata;
/// it owns no tuple, replacement assignment, lookup result or secondary index.
/// Constants are validated at preparation; selected variables remain checked on
/// each insertion. This is identity preparation, never membership or truth.
/// Candidate predicate locations avoid repeated owner-directory searches while
/// they still name the same signed predicate. Commit or interleaved insertion
/// may make a location stale; insertion checks it and uses ordinary lookup then.
/// No absence result is retained, and no mutation or synchronization is needed.
///
/// The header belongs to the caller's retained metadata accounting. The borrowed
/// source and assignment arrays remain separately owned; insertion still admits
/// the ordinary projected-entry envelope and all canonical/index growth.
#[derive(Debug)]
pub struct PreparedPattern<'a> {
    owner: storage::AtomScope,
    source: storage::Read<'a>,
    pattern: &'a PatternData,
    term_limits: TermLimits,
    locations: PredicateLocations,
}

impl AtomAppender<'_> {
    /// Authenticate one immutable admitted pattern for repeated insertion into
    /// this writer. Construction input returns `None` and keeps the general
    /// [`Self::insert_pattern_with`] path; no metadata is copied or allocated.
    /// This phase checks constants before any assignment is supplied.
    /// It also visits the current canonical predicate blocks once and searches
    /// the discovery predicate directory. Candidate locations remain immutable;
    /// repeated use after they become stale is correct but can repeat lookup.
    ///
    /// # Errors
    /// Refuses population/storage bounds, foreign or inaccessible source IDs,
    /// constant logical measures, or caller work. No identity is published and
    /// the owner's retained capacity and peak receipt are unchanged.
    pub fn prepare_pattern_with<'a, E>(
        &self,
        pattern: PatternRef<'a>,
        term_limits: TermLimits,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<PreparedPattern<'a>>, AssignedFailure<E>> {
        self.lookup().admit(0, limits)?;
        before().map_err(Failure::Stopped)?;
        let Some((source, data)) = pattern.admitted_parts() else {
            return Ok(None);
        };
        let read = storage::Read::from(&*self.store);
        let predicate = projected_predicate(read, pattern, &mut before)?;
        for argument in &data.terms {
            before().map_err(Failure::Stopped)?;
            if let TermData::Constant(id) = *argument {
                before().map_err(Failure::Stopped)?;
                if !read.contains_term(id) {
                    return Err(AssignedFailure::Assignment(AssignmentError::Read(
                        ReadError::OutsidePrefix,
                    )));
                }
                self.store
                    .check_projected_term_with(id, term_limits, &mut before)
                    .map_err(assigned_storage)?;
            }
        }
        before().map_err(Failure::Stopped)?;
        let column = self
            .store
            .atom_column_location_with(predicate, &mut before)
            .map_err(Failure::Stopped)?;
        let subtree =
            super::super::subtree_of(self.store, self.subtrees, pattern.predicate(), &mut before)
                .map_err(Failure::Stopped)?
                .unwrap_or_else(|position| position);
        Ok(Some(PreparedPattern {
            owner: self.store.atom_scope(),
            source,
            pattern: data,
            term_limits,
            locations: PredicateLocations { column, subtree },
        }))
    }

    /// Instantiate a prepared pattern using only its referenced assignment slots.
    /// Slot order, repeated occurrences and signed predicates remain unchanged.
    /// The exact writer is authenticated before entry storage; assignment scope
    /// and variables are checked after the entry envelope, as in general insertion.
    /// Fixed constant limits cannot be changed after preparation, and variable
    /// limits apply on every call, including occupied discovery rows.
    ///
    /// # Errors
    /// Refuses a different writer, foreign assignment, missing variables,
    /// logical/storage/population bounds, or caller work. Failed insertion never
    /// publishes a discovery position; admitted capacity/components may remain.
    pub fn insert_prepared_pattern_with<E>(
        &mut self,
        pattern: &PreparedPattern<'_>,
        values: AssignmentSlice<'_>,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, AssignedFailure<E>> {
        before().map_err(Failure::Stopped)?;
        if !storage::Read::from(&*self.store).accepts_atom_scope(&pattern.owner) {
            return Err(AssignedFailure::Assignment(AssignmentError::Read(
                ReadError::ForeignCatalog,
            )));
        }
        self.admit_entry(Projected::HEADER_BYTES, limits)?;
        projected_scope(storage::Read::from(&*self.store), values, &mut before)?;
        for argument in &pattern.pattern.terms {
            before().map_err(Failure::Stopped)?;
            if let TermData::Variable(slot) = *argument {
                let id = values
                    .slots
                    .get(slot)
                    .ok_or(AssignedFailure::Assignment(AssignmentError::Slot {
                        slot,
                        len: values.len(),
                    }))?
                    .ok_or(AssignedFailure::Assignment(AssignmentError::Unbound {
                        slot,
                    }))?;
                // Assignment IDs are sealed and scoped. A growing vocabulary
                // has one live writer; a writer over a frozen vocabulary has
                // its complete extent. Thus matching scope covers every assigned
                // ID here. This does not hold for older immutable snapshots,
                // whose general lookup must still check each prefix extent.
                self.store
                    .check_projected_term_with(id, pattern.term_limits, &mut before)
                    .map_err(assigned_storage)?;
            }
        }
        let projected = Projected {
            predicate: pattern.pattern.predicate,
            values: values.slots,
            arguments: ProjectionSource::Admitted(pattern.source, &pattern.pattern.terms),
        };
        self.insert_projection_at(&projected, Some(pattern.locations), limits, before)
    }
}

fn assigned_storage<E>(error: storage::Failure<E>) -> AssignedFailure<E> {
    match error {
        storage::Failure::Storage(error) => Failure::Catalog(error),
        storage::Failure::Stopped(error) => Failure::Stopped(error),
    }
    .into()
}

#[cfg(test)]
mod tests;
