//! One atom owner and immutable column views between support-growth rounds.

use std::collections::{BTreeMap, btree_map::Entry};
use std::mem::size_of;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::ProgramSite;
use zetesis_core::atom_interner::{self, AtomAppender, AtomInterner};
use zetesis_core::catalog::{AtomRef, Atoms, CatalogRead, PredicateRef, TermRef};
use zetesis_core::relation::{Catalog, CatalogFailure, Failure, Limits, Relation, Resource, Row};
#[cfg(test)]
use zetesis_core::{Atom, AtomPattern, Predicate, Value};
use zetesis_core::{
    AtomKey, BindingView, PatternRef, TemplateComponents, TemplateComponentsRef, TemplateTerm,
};

use super::{Counters, GroundingWork};
use crate::formula::ceiling;
use crate::grounding_observer::Event;
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

mod append;
mod close;
mod publication;
pub(super) use append::SupportAppend;
pub(crate) use append::{SourceAtom, SourceScope};
pub(crate) use close::ClosedSource;

#[cfg(test)]
mod tests;

struct CatalogRows {
    catalog: Catalog,
    /// One posting map per column a join can bind; `None` keeps no postings.
    columns: Vec<Option<BTreeMap<u32, Vec<usize>>>>,
    old_rows: usize,
}

/// A column's postings, or the absence of any for a column no join binds.
pub(super) enum Postings<'a> {
    Indexed(&'a BTreeMap<u32, Vec<usize>>),
    Unindexed,
}

/// Canonical payload has one evolving authority. Relations and pending rounds
/// retain only metadata scoped to that authority; discovery positions never escape.
pub(crate) struct SupportCatalog {
    pub(super) owner: AtomInterner,
    pub(super) components: Option<TemplateComponents>,
    scope: SourceScope,
    rows: Vec<CatalogRows>,
    pending: Vec<usize>,
    supported: Vec<u64>,
    // Shared only by a borrowed admission round. Relaxed accounting carries no
    // publication synchronization; completed snapshots omit this observation.
    // An atomic reference also preserves movable prepared checker state.
    growth: AtomicUsize,
    /// The columns joins can bind; absent until support completion installs it,
    /// and then every column of a predicate it does not name is unindexed.
    demand: Option<super::demand::Demand>,
    entries: usize,
    index_bytes: usize,
    prepared_bytes: usize,
}

impl Default for SupportCatalog {
    fn default() -> Self {
        Self {
            owner: AtomInterner::new(),
            components: None,
            scope: SourceScope::new(),
            rows: Vec::new(),
            pending: Vec::new(),
            supported: Vec::new(),
            growth: AtomicUsize::new(0),
            demand: None,
            entries: 0,
            index_bytes: size_of::<Self>() - size_of::<AtomInterner>()
                + SourceScope::shared_bytes(),
            prepared_bytes: 0,
        }
    }
}

impl SupportCatalog {
    pub(super) fn owner(&self) -> &AtomInterner {
        &self.owner
    }
    pub(super) fn prepared_bytes(&mut self, bytes: usize) {
        self.prepared_bytes = bytes;
    }

    pub(super) fn release_preparation(&mut self) {
        self.prepared_bytes = 0;
    }

    /// Keep postings only for demanded columns of relations created from now
    /// on. A relation published earlier keeps postings for every column: a
    /// superset of its demand, which costs memory but no probe its posting.
    pub(super) fn install_demand(&mut self, demand: super::demand::Demand) {
        self.demand = Some(demand);
    }

    pub(in crate::formula_support) fn bytes(
        &self,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        usize::try_from(self.owner.storage_bytes())
            .ok()
            .and_then(|owner| owner.checked_add(self.index_bytes))
            .and_then(|bytes| bytes.checked_add(self.prepared_bytes))
            .and_then(|bytes| bytes.checked_add(self.component_bytes().ok()?))
            .and_then(|bytes| bytes.checked_add(self.pending.capacity() * size_of::<usize>()))
            .and_then(|bytes| bytes.checked_add(self.supported.capacity() * size_of::<u64>()))
            .ok_or_else(|| failure(Failure::Overflow, location))
    }

    // The inline Option already belongs to index_bytes. Only capacities beyond
    // the component header are added here; source payload stays in owner.
    pub(super) fn component_bytes(&self) -> Result<usize, std::num::TryFromIntError> {
        usize::try_from(self.components.as_ref().map_or(0, |components| {
            components.storage_bytes() - size_of::<TemplateComponents>() as u128
        }))
    }

    /// The prior snapshot and append capability share one authority. Pending
    /// positions remain in this catalog throughout derivation and publication.
    pub(crate) fn split(
        &mut self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(Relations<'_>, SupportAppend<'_>), FormulaFailure> {
        debug_assert!(self.pending.is_empty());
        *self.growth.get_mut() = 0;
        let bytes = self
            .bytes(location)?
            .checked_add(size_of::<SupportAppend<'_>>())
            .ok_or_else(|| failure(Failure::Overflow, location))?;
        let owner_bytes = self.owner.storage_bytes();
        let pending_bytes = self.pending.capacity() * size_of::<usize>();
        let supported_bytes = self.supported.capacity() * size_of::<u64>();
        let (committed, appender) = self.owner.split();
        let growth = &self.growth;
        let relations = SnapshotSource {
            sources: &self.rows,
            components: self.components.as_ref(),
            read: committed.read(),
            entries: self.entries,
            bytes,
            growth: Some(growth),
        }
        .build(limits, counters, location)?;
        let append = SupportAppend::new(
            appender,
            &mut self.pending,
            &mut self.supported,
            &self.scope,
            growth,
            append::Base {
                bytes: relations.bytes,
                owner: owner_bytes,
                pending: pending_bytes,
                supported: supported_bytes,
            },
        );
        Ok((relations, append))
    }

    /// Publish only this authority's complete round, after every join and wake
    /// consumer has released its borrow. Failure abandons the whole support build.
    pub(super) fn publish(
        &mut self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        counters.observe_work(Event::SupportPublicationWork, |counters| {
            self.publish_rows(limits, counters, location)
        })
    }

    fn publish_rows(
        &mut self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        for source in &mut self.rows {
            counters.work(limits, location)?;
            source.old_rows = source.catalog.len();
        }
        let owner_bytes = self.owner.storage_bytes();
        let workspace = counters.accounting.workspace.bytes() as u128;
        let outer = self.bytes(location)? as u128 - owner_bytes + workspace;
        self.owner.restart_storage_peak();
        let committed = self
            .owner
            .commit_with(owner_limits(limits, outer, location)?, || {
                counters.work(limits, location)
            });
        record_owner_peak(self.owner.storage_peak_bytes(), outer, counters);
        committed.map_err(|error| atom_failure(error, limits, outer, location))?;
        *self.growth.get_mut() = 0;
        let mut previous: Option<(PredicateRef<'_>, usize)> = None;
        for position in 0..self.pending.len() {
            counters.work(limits, location)?;
            let discovery = self.pending[position];
            let mut memory =
                Memory::new(self.bytes(location)?, workspace, limits, counters, location);
            let read = self.owner.read();
            let atom = self
                .owner
                .get(discovery)
                .expect("same authority's pending discovery");
            let predicate = atom.predicate();
            let index = match previous {
                Some((prior, index))
                    if predicate.equals_ref_with(prior, || counters.work(limits, location))? =>
                {
                    index
                }
                _ => catalog_index(
                    &mut self.rows,
                    self.demand.as_ref(),
                    read,
                    predicate,
                    &mut memory,
                    counters,
                )?,
            };
            // Only this contiguous predicate run reuses the index: inserting a
            // different relation may shift every later directory position.
            previous = Some((predicate, index));
            let source = &mut self.rows[index];
            let old_bytes = source.catalog.retained_bytes();
            let outer_bytes = memory.outside(old_bytes)?;
            let mut checked = relation_limits(
                limits,
                counters,
                predicate.arity(),
                old_bytes + memory.remaining()?,
            );
            checked.max_values = usize::MAX;
            let receipt = source
                .catalog
                .insert(atom, checked)
                .map_err(|error| catalog_failure(error, limits, counters, outer_bytes, location))?;
            counters.record(Event::SupportPeakBytes(
                outer_bytes as u128 + receipt.storage.peak_construction_bytes as u128,
            ));
            counters.charge_work(receipt.storage.construction_work, limits, location)?;
            memory.release(old_bytes);
            memory.add(receipt.storage.retained_bytes)?;
            if receipt.inserted {
                ceiling(
                    FormulaResource::SupportIndexEntries,
                    self.entries as u128 + source.indexed() as u128,
                    limits.max_support_index_entries as u128,
                    location,
                )?;
                source.append_postings(read, receipt.row, &mut memory, counters)?;
                self.entries += source.indexed();
                counters.record(Event::SupportAtom);
            }
            self.index_bytes = memory.bytes
                - usize::try_from(self.owner.storage_bytes())
                    .map_err(|_| failure(Failure::Overflow, location))?
                - self.prepared_bytes
                - self
                    .component_bytes()
                    .map_err(|_| failure(Failure::Overflow, location))?
                - self.pending.capacity() * size_of::<usize>()
                - self.supported.capacity() * size_of::<u64>();
        }
        self.pending.clear();
        Ok(())
    }

    pub(crate) fn snapshot(
        &self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Relations<'_>, FormulaFailure> {
        SnapshotSource {
            sources: &self.rows,
            components: self.components.as_ref(),
            read: self.owner.read(),
            entries: self.entries,
            bytes: self.bytes(location)?,
            growth: None,
        }
        .build(limits, counters, location)
    }

    #[cfg(test)]
    pub(super) fn insert(
        mut self,
        atom: &Atom,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let (_, mut append) = self.split(limits, counters, location)?;
        append.atom(atom.into(), limits, counters, location)?;
        self.publish(limits, counters, location)?;
        Ok(self)
    }
}

struct SnapshotSource<'a> {
    sources: &'a [CatalogRows],
    components: Option<&'a TemplateComponents>,
    read: CatalogRead<'a>,
    entries: usize,
    bytes: usize,
    growth: Option<&'a AtomicUsize>,
}
impl<'a> SnapshotSource<'a> {
    fn build(
        self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Relations<'a>, FormulaFailure> {
        let components = self
            .components
            .map(|components| {
                components
                    .bind_with(self.read, || counters.work(limits, location))
                    .map_err(|error| super::components::failure(error, limits, 0, location))
            })
            .transpose()?;
        let mut memory = Memory::new(
            self.bytes,
            counters.accounting.workspace.bytes() as u128,
            limits,
            counters,
            location,
        );
        memory.add(size_of::<Relations<'_>>())?;
        let mut rows = Vec::new();
        memory.reserve(&mut rows, self.sources.len())?;
        for source in self.sources {
            counters.work(limits, location)?;
            rows.push(RelationRows {
                relation: source
                    .catalog
                    .view(self.read)
                    .map_err(|error| failure(error, location))?,
                columns: &source.columns,
                catalog: &source.catalog,
                old_rows: source.old_rows,
                atoms: source
                    .catalog
                    .atoms(self.read)
                    .map_err(|error| failure(error, location))?,
            });
        }
        Ok(Relations {
            rows,
            components,
            read: Some(self.read),
            entries: self.entries,
            bytes: memory.bytes,
            growth: self.growth,
        })
    }
}

fn find_catalog(
    rows: &[CatalogRows],
    read: CatalogRead<'_>,
    predicate: PredicateRef<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Result<usize, usize>, FormulaFailure> {
    let mut start = 0;
    let mut end = rows.len();
    while start < end {
        counters.work(limits, location)?;
        let middle = start + (end - start) / 2;
        let current = rows[middle]
            .catalog
            .predicate(read)
            .map_err(|error| failure(error, location))?;
        match current.compare_ref_with(predicate, || counters.work(limits, location))? {
            std::cmp::Ordering::Less => start = middle + 1,
            std::cmp::Ordering::Greater => end = middle,
            std::cmp::Ordering::Equal => return Ok(Ok(middle)),
        }
    }
    Ok(Err(start))
}

/// Resolve a predicate run, admitting its relation and directory slot if absent.
fn catalog_index(
    rows: &mut Vec<CatalogRows>,
    demand: Option<&super::demand::Demand>,
    read: CatalogRead<'_>,
    predicate: PredicateRef<'_>,
    memory: &mut Memory<'_>,
    counters: &mut Counters,
) -> Result<usize, FormulaFailure> {
    match find_catalog(
        rows,
        read,
        predicate,
        memory.limits,
        counters,
        memory.location,
    )? {
        Ok(index) => Ok(index),
        Err(index) => {
            let source = CatalogRows::new(read, predicate, demand, memory, counters)?;
            counters.charge_work(rows.len() as u128, memory.limits, memory.location)?;
            memory.reserve(rows, 1)?;
            rows.insert(index, source);
            Ok(index)
        }
    }
}

pub(crate) fn owner_limits(
    limits: &FormulaLimits,
    outer: u128,
    location: ProgramSite,
) -> Result<atom_interner::Limits, FormulaFailure> {
    ceiling(
        FormulaResource::SupportBytes,
        outer,
        limits.max_support_bytes as u128,
        location,
    )?;
    Ok(atom_interner::Limits {
        max_atoms: limits.theory.max_atoms,
        max_bytes: limits.max_support_bytes as u128 - outer,
    })
}

fn record_owner_peak(peak: u128, outer: u128, counters: &Counters) {
    counters.record(Event::SupportPeakBytes(peak.saturating_add(outer)));
}

pub(crate) fn atom_failure(
    error: atom_interner::Failure<FormulaFailure>,
    limits: &FormulaLimits,
    outer: u128,
    location: ProgramSite,
) -> FormulaFailure {
    match error {
        atom_interner::Failure::Stopped(error) => error,
        atom_interner::Failure::Catalog(error) => FormulaFailure::AtomCatalog { error, location },
        atom_interner::Failure::Allocation(error) => {
            FormulaFailure::AtomAllocation { error, location }
        }
        atom_interner::Failure::Atoms { required, limit } => FormulaFailure::Limit {
            resource: FormulaResource::Atoms,
            observed: required as u128,
            limit: limit as u128,
            location,
        },
        atom_interner::Failure::Bytes { required, .. } => FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            observed: required.saturating_add(outer),
            limit: limits.max_support_bytes as u128,
            location,
        },
        atom_interner::Failure::Overflow => failure(Failure::Overflow, location),
    }
}

impl CatalogRows {
    /// Admit the empty relation and its posting lanes before catalog insertion.
    fn new(
        read: CatalogRead<'_>,
        predicate: PredicateRef<'_>,
        demand: Option<&super::demand::Demand>,
        memory: &mut Memory<'_>,
        counters: &mut Counters,
    ) -> Result<Self, FormulaFailure> {
        let limits = memory.limits;
        let location = memory.location;
        counters.work(limits, location)?;
        let declared = read
            .declare_existing(predicate)
            .map_err(|_| failure(Failure::Owner, location))?;
        let outer_bytes = memory.outside(0)?;
        let catalog = Catalog::new(
            read,
            declared,
            relation_limits(limits, counters, predicate.arity(), memory.remaining()?),
        )
        .map_err(|error| catalog_failure(error, limits, counters, outer_bytes, location))?;
        counters.record(Event::SupportPeakBytes(
            memory.live_bytes() + catalog.construction().peak_construction_bytes as u128,
        ));
        counters.charge_work(catalog.construction().construction_work, limits, location)?;
        memory.add(catalog.retained_bytes() - size_of::<Catalog>())?;
        // Without installed demand every column keeps postings, as before.
        let demanded = match demand {
            Some(demand) => demand.columns(predicate, limits, counters, location)?,
            None => None,
        };
        let mut columns = Vec::new();
        memory.reserve(&mut columns, predicate.arity())?;
        for column in 0..predicate.arity() {
            counters.work(limits, location)?;
            let indexed = match (demand, demanded) {
                (None, _) => true,
                (Some(_), Some(demanded)) => demanded[column],
                (Some(_), None) => false,
            };
            columns.push(indexed.then(BTreeMap::new));
        }
        Ok(Self {
            catalog,
            columns,
            old_rows: 0,
        })
    }

    /// The number of columns keeping postings.
    fn indexed(&self) -> usize {
        self.columns
            .iter()
            .filter(|column| column.is_some())
            .count()
    }

    fn append_postings(
        &mut self,
        read: CatalogRead<'_>,
        row: usize,
        memory: &mut Memory<'_>,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        let view = self
            .catalog
            .view(read)
            .map_err(|error| failure(error, memory.location))?;
        memory.add(size_of::<Relation<'_>>())?;
        for (column, postings) in self
            .columns
            .iter_mut()
            .enumerate()
            .filter_map(|(column, postings)| postings.as_mut().map(|postings| (column, postings)))
        {
            counters.work(memory.limits, memory.location)?;
            let id = view.column(column).expect("checked column")[row];
            let posting = match postings.entry(id) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    memory.add(size_of::<(u32, Vec<usize>)>())?;
                    entry.insert(Vec::new())
                }
            };
            if posting.len() == posting.capacity() {
                counters.charge_work(posting.len() as u128, memory.limits, memory.location)?;
            }
            memory.reserve(posting, 1)?;
            posting.push(row);
            counters.record(Event::SupportIndexEntry);
        }
        memory.release(size_of::<Relation<'_>>());
        Ok(())
    }
}

/// Preserve cumulative work on a failed append, including completed charged
/// comparisons before the failed admission. Build failure consumes this owner.
fn catalog_failure(
    error: CatalogFailure,
    limits: &FormulaLimits,
    counters: &mut Counters,
    outer_bytes: usize,
    location: ProgramSite,
) -> FormulaFailure {
    counters.record(Event::SupportPeakBytes(
        outer_bytes as u128 + error.peak_construction_bytes as u128,
    ));
    let failure = relation_failure(
        error.error,
        limits,
        counters.accounting.work,
        outer_bytes,
        location,
    );
    match counters.charge_work(error.work, limits, location) {
        Ok(()) => failure,
        Err(charge) => charge,
    }
}

/// Immutable possible support, including snapshots between growth rounds.
/// This view alone establishes neither completion nor current-world truth.
#[derive(Default)]
pub(crate) struct Relations<'source> {
    rows: Vec<RelationRows<'source>>,
    components: Option<TemplateComponentsRef<'source>>,
    read: Option<CatalogRead<'source>>,
    growth: Option<&'source AtomicUsize>,
    pub(super) entries: usize,
    pub(super) bytes: usize,
}

pub(super) struct RelationRows<'source> {
    pub(super) relation: Relation<'source>,
    columns: &'source [Option<BTreeMap<u32, Vec<usize>>>],
    catalog: &'source Catalog,
    old_rows: usize,
    pub(super) atoms: Atoms<'source>,
}

impl<'source> RelationRows<'source> {
    /// The postings of `column`, through which every reader goes: a column
    /// no join binds keeps none, and is never read as an empty one.
    pub(super) fn postings(&self, column: usize) -> Postings<'source> {
        self.columns[column]
            .as_ref()
            .map_or(Postings::Unindexed, Postings::Indexed)
    }

    /// The number of columns.
    #[cfg(test)]
    pub(super) fn column_count(&self) -> usize {
        self.columns.len()
    }

    pub(super) fn row(&self, position: usize) -> Option<Row<'_, 'source>> {
        self.relation.row(position)
    }

    pub(super) fn row_count(&self) -> usize {
        self.relation.row_count()
    }

    pub(super) fn old_rows(&self) -> usize {
        self.old_rows
    }
}

impl<'source> Relations<'source> {
    pub(super) fn components(&self) -> Option<TemplateComponentsRef<'source>> {
        self.components
    }
    pub(super) fn current_bytes(&self) -> usize {
        self.bytes
            + self
                .growth
                .map_or(0, |growth| growth.load(Ordering::Relaxed))
    }

    fn find(&self, predicate: PredicateRef<'_>) -> Option<&RelationRows<'source>> {
        self.rows
            .binary_search_by(|rows| rows.relation.predicate().cmp(&predicate))
            .ok()
            .map(|index| &self.rows[index])
    }

    pub(super) fn find_with(
        &self,
        predicate: PredicateRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<&RelationRows<'source>>, FormulaFailure> {
        self.find_checked(predicate, || counters.work(limits, location))
    }

    fn find_checked<E>(
        &self,
        predicate: PredicateRef<'_>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<&RelationRows<'source>>, E> {
        let mut start = 0;
        let mut end = self.rows.len();
        while start < end {
            before()?;
            let middle = start + (end - start) / 2;
            match self.rows[middle]
                .relation
                .predicate()
                .compare_ref_with(predicate, &mut before)?
            {
                std::cmp::Ordering::Less => start = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => return Ok(Some(&self.rows[middle])),
            }
        }
        Ok(None)
    }

    /// One checked directory lookup for the two source-round populations.
    /// The caller supplies its own work boundary; no semantic ID ordering is used.
    pub(super) fn row_counts_with<E>(
        &self,
        predicate: PredicateRef<'_>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(usize, usize), E> {
        let Some(rows) = self.find_checked(predicate, &mut before)? else {
            return Ok((0, 0));
        };
        before()?;
        Ok((rows.old_rows, rows.relation.row_count()))
    }

    pub(super) fn relation_with<'predicate>(
        &self,
        predicate: impl Into<PredicateRef<'predicate>>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<&Relation<'_>>, FormulaFailure> {
        self.find_with(predicate.into(), limits, counters, location)
            .map(|rows| rows.map(|rows| &rows.relation))
    }
    pub(crate) fn predicates(&self) -> impl Iterator<Item = PredicateRef<'source>> + '_ {
        self.rows.iter().map(|rows| rows.relation.predicate())
    }
    pub(super) fn source_atoms(
        &self,
    ) -> impl Iterator<Item = (PredicateRef<'source>, Atoms<'source>)> + '_ {
        self.rows
            .iter()
            .map(|rows| (rows.relation.predicate(), rows.atoms))
    }
    pub(super) fn old_rows<'predicate>(
        &self,
        predicate: impl Into<PredicateRef<'predicate>>,
    ) -> usize {
        self.find(predicate.into()).map_or(0, |rows| rows.old_rows)
    }
    pub(super) fn row_count<'predicate>(
        &self,
        predicate: impl Into<PredicateRef<'predicate>>,
    ) -> usize {
        self.find(predicate.into())
            .map_or(0, |rows| rows.relation.row_count())
    }
    pub(crate) fn rows<'predicate>(
        &self,
        predicate: impl Into<PredicateRef<'predicate>>,
    ) -> impl Iterator<Item = Row<'_, '_>> {
        self.find(predicate.into()).into_iter().flat_map(|rows| {
            (0..rows.relation.row_count()).map(|row| rows.relation.row(row).expect("bounded row"))
        })
    }
    #[cfg(test)]
    pub(super) fn row<'predicate>(
        &self,
        predicate: impl Into<PredicateRef<'predicate>>,
        position: usize,
    ) -> Option<Row<'_, '_>> {
        self.find(predicate.into())?.relation.row(position)
    }

    pub(super) fn contains(
        &self,
        key: &AtomKey<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        let Some(rows) = self.find_with(key.predicate(), limits, counters, location)? else {
            return Ok(false);
        };
        let owner_bytes = rows.catalog.retained_bytes();
        let mut checked = relation_limits(limits, counters, key.predicate().arity(), owner_bytes);
        checked.max_values = usize::MAX;
        let receipt = rows
            .catalog
            .lookup_key(self.read.expect("nonempty snapshot"), key, checked)
            .map_err(|error| {
                catalog_failure(
                    error,
                    limits,
                    counters,
                    self.current_bytes() - owner_bytes,
                    location,
                )
            })?;
        counters.charge_work(receipt.storage.construction_work, limits, location)?;
        Ok(receipt.row.is_some())
    }

    #[cfg(test)]
    pub(super) fn probe(
        &self,
        pattern: &AtomPattern,
        values: &[Option<Value>],
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<&[usize]>, FormulaFailure> {
        let rows = self.find_with(pattern.predicate().into(), limits, counters, location)?;
        self.probe_at(
            rows,
            pattern.into(),
            values.into(),
            0,
            GroundingWork::new(limits, counters, location),
        )
    }

    /// The caller resolves this immutable snapshot's relation once, before its
    /// cursor visits rows. Equalities are folded immediately; no key or query
    /// vector outlives an individual dictionary lookup.
    pub(super) fn probe_at<'rows>(
        &self,
        rows: Option<&'rows RelationRows<'source>>,
        pattern: PatternRef<'_>,
        values: BindingView<'_>,
        outer_bytes: usize,
        work: GroundingWork<'_>,
    ) -> Result<Option<&'rows [usize]>, FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        counters.record(Event::JoinProbe);
        let Some(rows) = rows else {
            return Ok(Some(&[]));
        };
        let mut memory = Memory::new(
            self.current_bytes(),
            outer_bytes as u128,
            limits,
            counters,
            location,
        );
        memory.add(0)?;
        let mut selected: Option<&[usize]> = None;
        let mut possible = true;
        let terms = pattern.terms();
        for column in 0..terms.len() {
            counters.work(limits, location)?;
            let term = terms.at(column).expect("checked pattern arity");
            let value = match term {
                TemplateTerm::Constant(value) => Some(value),
                TemplateTerm::Variable(variable) => {
                    if variable >= values.len() {
                        return Err(FormulaFailure::UnsafeVariable { variable, location });
                    }
                    values.get(variable)
                }
            };
            if let Some(value) = value {
                let outside = memory.outside(rows.relation.storage().retained_bytes)?;
                let base_work = counters.accounting.work;
                let checked = relation_limits(
                    limits,
                    counters,
                    pattern.predicate().arity(),
                    rows.relation.storage().retained_bytes + memory.remaining()?,
                );
                let attempt = rows
                    .relation
                    .equality_attempt_with(column, value, checked, || {
                        counters.work(limits, location)
                    });
                counters.record(Event::SupportPeakBytes(
                    outside as u128 + attempt.peak_bytes as u128,
                ));
                let equality = attempt.result.map_err(|error| match error {
                    zetesis_core::relation::QueryFailure::Relation(error) => {
                        relation_failure(error, limits, base_work, outside, location)
                    }
                    zetesis_core::relation::QueryFailure::Stopped(error) => error,
                })?;
                if let Some(equality) = equality {
                    match rows.postings(equality.column()) {
                        Postings::Indexed(postings) => {
                            let posting = postings
                                .get(&equality.value_id())
                                .map_or(&[][..], Vec::as_slice);
                            if selected.is_none_or(|previous| posting.len() < previous.len()) {
                                selected = Some(posting);
                            }
                        }
                        // The value is present, so rows may match; with no
                        // posting this column narrows nothing, and the
                        // full matcher still checks every offered row.
                        Postings::Unindexed => {
                            counters.record(Event::UnindexedProbe);
                            debug_assert!(
                                false,
                                "demand covers every column a join binds: {}/{} column {}",
                                pattern.predicate().name(),
                                pattern.predicate().arity(),
                                equality.column()
                            );
                        }
                    }
                } else {
                    possible = false;
                }
            }
        }
        if !possible {
            selected = Some(&[]);
        }
        #[cfg(test)]
        super::postings::observe(rows, pattern, values, selected);
        Ok(selected)
    }
}

fn relation_limits(
    limits: &FormulaLimits,
    counters: &Counters,
    arity: usize,
    bytes: usize,
) -> Limits {
    Limits {
        max_rows: limits.theory.max_atoms,
        max_columns: arity,
        max_values: limits.max_support_index_entries,
        max_bytes: bytes,
        max_work: limits.max_work.saturating_sub(counters.accounting.work),
    }
}

fn failure(error: Failure, location: ProgramSite) -> FormulaFailure {
    FormulaFailure::SupportRelation { error, location }
}

/// Shared ceilings retain their formula-wide resource and cumulative amount.
/// Other core refusals retain their typed cause at the source location.
pub(super) fn relation_failure(
    error: Failure,
    limits: &FormulaLimits,
    base_work: u64,
    outer_bytes: usize,
    location: ProgramSite,
) -> FormulaFailure {
    let (resource, base, observed, limit) = match error {
        Failure::Limit {
            resource: Resource::Work,
            observed,
            ..
        } => (
            FormulaResource::Work,
            u128::from(base_work),
            observed,
            u128::from(limits.max_work),
        ),
        Failure::Limit {
            resource: Resource::Bytes,
            observed,
            ..
        } => (
            FormulaResource::SupportBytes,
            outer_bytes as u128,
            observed,
            limits.max_support_bytes as u128,
        ),
        other => return failure(other, location),
    };
    observed.checked_add(base).map_or_else(
        || failure(Failure::Overflow, location),
        |observed| FormulaFailure::Limit {
            resource,
            observed,
            limit,
            location,
        },
    )
}

/// Named canonical authority, metadata, snapshot and index capacities.
/// Allocator/tree overhead is excluded; this is not total RSS.
pub(super) struct Memory<'limits> {
    // The accounted span retained by this caller after its scratch is released.
    // Other simultaneously live owners must never enter this receipt.
    pub(super) bytes: usize,
    external_bytes: u128,
    limits: &'limits FormulaLimits,
    location: ProgramSite,
    observed: crate::grounding_observer::Work,
}

impl<'limits> Memory<'limits> {
    pub(super) fn new(
        bytes: usize,
        external_bytes: u128,
        limits: &'limits FormulaLimits,
        counters: &Counters,
        location: ProgramSite,
    ) -> Self {
        Self {
            bytes,
            external_bytes,
            limits,
            location,
            observed: counters.observed.clone(),
        }
    }

    /// Compose the current span with other live owners exactly once. The
    /// external amount is a bounded sum of named usize capacities; keeping it
    /// wide preserves an over-limit requirement before narrowing any receipt.
    fn live_bytes(&self) -> u128 {
        self.external_bytes + self.bytes as u128
    }

    /// Give a child allocator all live bytes outside its already counted span.
    fn outside(&self, included: usize) -> Result<usize, FormulaFailure> {
        self.live_bytes()
            .checked_sub(included as u128)
            .and_then(|bytes| usize::try_from(bytes).ok())
            .ok_or_else(|| failure(Failure::Overflow, self.location))
    }

    pub(super) fn add(&mut self, amount: usize) -> Result<(), FormulaFailure> {
        let next = self.bytes as u128 + amount as u128;
        let live = self.external_bytes + next;
        ceiling(
            FormulaResource::SupportBytes,
            live,
            self.limits.max_support_bytes as u128,
            self.location,
        )?;
        self.bytes =
            usize::try_from(next).map_err(|_| failure(Failure::Overflow, self.location))?;
        self.observed.record(Event::SupportPeakBytes(live));
        Ok(())
    }

    fn remaining(&self) -> Result<usize, FormulaFailure> {
        ceiling(
            FormulaResource::SupportBytes,
            self.live_bytes(),
            self.limits.max_support_bytes as u128,
            self.location,
        )?;
        Ok(
            usize::try_from(self.limits.max_support_bytes as u128 - self.live_bytes())
                .expect("admitted remaining bytes fit the usize ceiling"),
        )
    }

    pub(super) fn release(&mut self, amount: usize) {
        self.bytes = self.bytes.checked_sub(amount).expect("charged allocation");
    }

    pub(super) fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Result<(), FormulaFailure> {
        let needed = values
            .len()
            .checked_add(additional)
            .ok_or_else(|| failure(Failure::Overflow, self.location))?;
        if needed <= values.capacity() {
            return Ok(());
        }
        let proposed = needed.max(values.capacity().saturating_mul(2));
        // Growth admits old and replacement buffers simultaneously. Retained
        // bytes become only the final capacity after the allocation succeeds.
        let replacement = proposed
            .checked_mul(size_of::<T>())
            .ok_or_else(|| failure(Failure::Overflow, self.location))?;
        ceiling(
            FormulaResource::SupportBytes,
            self.live_bytes() + replacement as u128,
            self.limits.max_support_bytes as u128,
            self.location,
        )?;
        let previous = values.capacity();
        values
            .try_reserve_exact(proposed - values.len())
            .map_err(|_| failure(Failure::Allocation, self.location))?;
        let actual = values
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or_else(|| failure(Failure::Overflow, self.location))?;
        let peak = self.live_bytes() + actual as u128;
        self.observed.record(Event::SupportPeakBytes(peak));
        ceiling(
            FormulaResource::SupportBytes,
            peak,
            self.limits.max_support_bytes as u128,
            self.location,
        )?;
        self.add(
            (values.capacity() - previous)
                .checked_mul(size_of::<T>())
                .ok_or_else(|| failure(Failure::Overflow, self.location))?,
        )
    }
}
