//! A prepared head borrows exact immutable source rows instead of a copied frame.

use super::{
    AssignedFailure, AtomAppender, Failure, Limits, PatternRef, PreparedPattern, Projected,
    ProjectionSource, ReadError, TermData, TermLimits, assigned_storage, storage,
};
use crate::catalog::AssignmentError;
use crate::catalog::interner::query::RowArguments;
use crate::relation::{Relation, Row};

/// One variable argument projected from an original source-row occurrence.
/// Input order is the caller's declared relation order, not a posting position,
/// dictionary coordinate, source atom identity or assertion of truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowColumn {
    /// Index of the immutable relation supplied during preparation.
    pub input: usize,
    /// Original argument column of that relation.
    pub column: usize,
}

/// One pattern and its exact immutable row sources, bound to a single writer.
/// This owns only projection metadata; all canonical tuples remain in their
/// existing authority. It establishes no join, injectivity, freshness, support
/// membership or complete producer family. The caller establishes those separate
/// source properties and supplies only the actually selected rows.
///
/// Preparation allocates nothing: it takes ownership of the supplied vectors.
/// Their capacities and this header belong to the caller's retained accounting,
/// exposed by [`Self::storage_bytes`], and are excluded from interner `Limits`
/// during both preparation and insertion. Borrowed source owners are separate.
pub struct PreparedRows<'rows, 'source> {
    pattern: PreparedPattern<'source>,
    sources: Vec<&'rows Relation<'source>>,
    columns: Vec<Option<RowColumn>>,
    check_terms: bool,
}

impl PreparedRows<'_, '_> {
    /// This owned header and metadata capacity, excluding borrowed sources and
    /// the writer. Moving the input vectors creates no second allocation.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + self.sources.capacity() as u128 * size_of::<&Relation<'_>>() as u128
            + self.columns.capacity() as u128 * size_of::<Option<RowColumn>>() as u128
    }
}

impl AtomAppender<'_> {
    /// Prepare a pattern projected from exact immutable canonical relations.
    /// `columns` follows head argument order: constants require `None`, variables
    /// require a source column, and repeated occurrences of the same variable
    /// require the same column. Different variables may share a column.
    ///
    /// Only metadata is inspected: no relation row is visited, selected or
    /// published. Construction patterns or noncanonical relations return `None`
    /// for the general route. Fixed constant limits are checked here; finite
    /// variable limits apply only to rows later supplied for insertion. For `a`
    /// head arguments and `b` sources, mapping validation costs O(a² + b)
    /// metadata visits; repeated-slot comparisons are charged individually.
    ///
    /// # Errors
    /// Refuses incompatible canonical owners, invalid column mappings, constant
    /// logical limits, interner population/storage bounds or caller work.
    /// Supplied vectors are consumed on every outcome; no writer state changes.
    pub fn prepare_row_pattern_with<'rows, 'source, E>(
        &self,
        pattern: PatternRef<'source>,
        sources: Vec<&'rows Relation<'source>>,
        columns: Vec<Option<RowColumn>>,
        term_limits: TermLimits,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<PreparedRows<'rows, 'source>>, AssignedFailure<E>> {
        let Some(pattern) = self.prepare_pattern_with(pattern, term_limits, limits, &mut before)?
        else {
            return Ok(None);
        };
        let read = storage::Read::from(&*self.store);
        for source in &sources {
            before().map_err(Failure::Stopped)?;
            let Some(source) = source.canonical_read() else {
                return Ok(None);
            };
            if !read.same_vocabulary(source.storage()) {
                return Err(AssignedFailure::Assignment(AssignmentError::Read(
                    ReadError::ForeignCatalog,
                )));
            }
        }
        if columns.len() != pattern.pattern.terms.len() {
            return Err(shape());
        }
        for (index, term) in pattern.pattern.terms.iter().enumerate() {
            before().map_err(Failure::Stopped)?;
            match (*term, columns[index]) {
                (TermData::Constant(_), None) => {}
                (TermData::Variable(slot), Some(column)) => {
                    let Some(source) = sources.get(column.input) else {
                        return Err(shape());
                    };
                    if column.column >= source.predicate().arity() {
                        return Err(shape());
                    }
                    for (prior, term) in pattern.pattern.terms[..index].iter().enumerate() {
                        before().map_err(Failure::Stopped)?;
                        if matches!(*term, TermData::Variable(known) if known == slot)
                            && columns[prior] != Some(column)
                        {
                            return Err(shape());
                        }
                    }
                }
                _ => return Err(shape()),
            }
        }
        // This uses admitted Store terms, not arbitrary Measures. Every new
        // term passes Measures::check before publication, including the u128
        // canonical+rendered byte sum against a usize limit. Hence that sum,
        // nodes and depth all fit usize::MAX. Snapshots retain those same terms.
        let check_terms = term_limits
            != TermLimits {
                max_nodes: usize::MAX,
                max_depth: usize::MAX,
                max_bytes: usize::MAX,
            };
        Ok(Some(PreparedRows {
            pattern,
            sources,
            columns,
            check_terms,
        }))
    }

    /// Instantiate the prepared pattern from its exact selected relation rows.
    /// The original signed predicate, constants, order and repeated arguments
    /// are retained. No raw term IDs, assignment frame or tuple copy is accepted.
    /// Identity, collision checks and both discovery indexes use the ordinary
    /// canonical publication path; old and duplicate identities remain exact.
    ///
    /// # Errors
    /// Refuses a different writer/relation, missing rows, selected logical term
    /// limits, interner bounds or caller work. Failed insertion publishes no
    /// discovery position; complete canonical components/capacities may remain.
    ///
    /// # Panics
    /// Panics if an admitted canonical relation row loses its canonical term
    /// coordinates. Safe relation construction and the exact-owner checks keep
    /// those coordinates immutable for this operation.
    pub fn insert_prepared_rows_with<E>(
        &mut self,
        prepared: &PreparedRows<'_, '_>,
        rows: &[Row<'_, '_>],
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, AssignedFailure<E>> {
        before().map_err(Failure::Stopped)?;
        if !storage::Read::from(&*self.store).accepts_atom_scope(&prepared.pattern.owner) {
            return Err(AssignedFailure::Assignment(AssignmentError::Read(
                ReadError::ForeignCatalog,
            )));
        }
        self.admit_entry(
            Projected::HEADER_BYTES + size_of::<RowArguments<'_>>() as u128,
            limits,
        )?;
        if rows.len() != prepared.sources.len() {
            return Err(shape());
        }
        for (row, source) in rows.iter().zip(&prepared.sources) {
            before().map_err(Failure::Stopped)?;
            if !row.belongs_to(source) {
                return Err(AssignedFailure::Assignment(AssignmentError::Read(
                    ReadError::ForeignCatalog,
                )));
            }
        }
        let arguments = RowArguments {
            source: prepared.pattern.source,
            terms: &prepared.pattern.pattern.terms,
            columns: &prepared.columns,
            rows,
        };
        if prepared.check_terms {
            for (index, term) in arguments.terms.iter().enumerate() {
                if matches!(term, TermData::Variable(_)) {
                    before().map_err(Failure::Stopped)?;
                    let id = arguments
                        .argument(index)
                        .canonical()
                        .expect("authenticated canonical row")
                        .1;
                    self.store
                        .check_projected_term_with(id, prepared.pattern.term_limits, &mut before)
                        .map_err(assigned_storage)?;
                }
            }
        }
        let projected = Projected {
            predicate: prepared.pattern.pattern.predicate,
            values: &[],
            arguments: ProjectionSource::Rows(&arguments),
        };
        self.insert_projection_at(&projected, Some(prepared.pattern.locations), limits, before)
    }
}

fn shape<E>() -> AssignedFailure<E> {
    Failure::Catalog(storage::Fault::Shape).into()
}

#[cfg(test)]
mod tests;
