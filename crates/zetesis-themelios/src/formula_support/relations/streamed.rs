//! Keep the support relations streamed constraints read beside the closed base.

use std::collections::HashSet;
use std::mem::size_of;

use zetesis_core::Predicate;
use zetesis_core::relation::Catalog;

use super::close::SourceCloseFailure;
use super::{CatalogRows, ClosedSource, SnapshotSource, SupportCatalog, failure};
use crate::formula_support::{Counters, GroundingWork};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

/// Support relations of the predicates the retained constraints name, kept
/// after the catalog that discovered their rows was closed. Each relation's
/// membership belongs to that writer, so it is read through the closed
/// catalog's `read`, which keeps the writer's scopes and every row.
pub(crate) struct StreamedRows {
    rows: Vec<CatalogRows>,
    entries: usize,
    bytes: usize,
}

impl CatalogRows {
    /// Named capacity of the relation and its postings: the catalog object and
    /// metadata, each posting map's entries and each posting list's capacity.
    /// Map node overhead is excluded.
    fn retained_bytes(&self) -> usize {
        self.catalog.retained_bytes() - size_of::<Catalog>()
            + size_of::<Self>()
            + self.columns.capacity()
                * size_of::<Option<std::collections::BTreeMap<u32, Vec<usize>>>>()
            + self
                .columns
                .iter()
                .flatten()
                .map(|postings| {
                    postings.len() * size_of::<(u32, Vec<usize>)>()
                        + postings
                            .values()
                            .map(|rows| rows.capacity() * size_of::<usize>())
                            .sum::<usize>()
                })
                .sum::<usize>()
    }

    /// Row identifiers retained in this relation's postings.
    fn entries(&self) -> usize {
        self.columns
            .iter()
            .flatten()
            .map(|postings| postings.values().map(Vec::len).sum::<usize>())
            .sum()
    }
}

impl SupportCatalog {
    /// Close this owner, keeping the relations of the predicates in `keep`
    /// (every relation when `keep` is absent) with their postings and dropping every other relation, the discovery
    /// state and the order indexes. The kept relations' bytes are admitted with
    /// the closed envelope.
    pub(in crate::formula_support) fn into_streamed(
        mut self,
        keep: Option<&HashSet<Predicate>>,
        work: GroundingWork<'_>,
    ) -> Result<(ClosedSource, StreamedRows), SourceCloseFailure> {
        let location = work.location;
        let read = self.owner.read();
        let mut kept_mask = Vec::new();
        kept_mask.try_reserve_exact(self.rows.len()).map_err(|_| {
            SourceCloseFailure::new(
                failure(zetesis_core::relation::Failure::Allocation, location),
                0,
            )
        })?;
        for rows in &self.rows {
            let predicate = rows
                .catalog
                .predicate(read)
                .map_err(|error| SourceCloseFailure::new(failure(error, location), 0))?;
            kept_mask.push(keep.is_none_or(|keep| keep.contains(&owned(predicate))));
        }
        let mut kept = Vec::new();
        let mut dropped = Vec::new();
        for (rows, keep) in std::mem::take(&mut self.rows).into_iter().zip(kept_mask) {
            if keep {
                kept.push(rows);
            } else {
                dropped.push(rows);
            }
        }
        self.rows = dropped;
        let bytes = kept.iter().map(CatalogRows::retained_bytes).sum::<usize>()
            + kept.capacity() * size_of::<CatalogRows>();
        let entries = kept.iter().map(CatalogRows::entries).sum();
        let closed = self.into_closed(bytes as u128, work)?;
        Ok((
            closed,
            StreamedRows {
                rows: kept,
                entries,
                bytes,
            },
        ))
    }
}

impl StreamedRows {
    /// Named bytes of the kept relations and postings.
    pub(crate) const fn bytes(&self) -> usize {
        self.bytes
    }

    /// An immutable view of the kept relations over the closed base. Views
    /// borrow the existing columns and postings; rows are not copied.
    pub(crate) fn snapshot<'a>(
        &'a self,
        closed: &'a ClosedSource,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<super::Relations<'a>, FormulaFailure> {
        let bytes = usize::try_from(closed.storage_bytes())
            .ok()
            .and_then(|closed| closed.checked_add(self.bytes))
            .ok_or_else(|| failure(zetesis_core::relation::Failure::Overflow, location))?;
        SnapshotSource {
            sources: &self.rows,
            components: closed.components.as_ref(),
            read: closed.storage.read(),
            entries: self.entries,
            bytes,
            growth: None,
        }
        .build(limits, counters, location)
    }
}

fn owned(predicate: zetesis_core::catalog::PredicateRef<'_>) -> Predicate {
    Predicate::with_sign(predicate.name(), predicate.arity(), predicate.sign())
        .expect("an admitted predicate is a valid signature")
}
