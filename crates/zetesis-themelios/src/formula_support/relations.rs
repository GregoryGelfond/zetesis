//! One atom owner and immutable column views between support-growth rounds.

use std::collections::{BTreeMap, btree_map::Entry};
use std::mem::size_of;

use themelios_base::span::Location;
use zetesis_core::relation::{Catalog, CatalogFailure, Failure, Limits, Relation, Resource, Row};
use zetesis_core::{Atom, AtomKey, AtomPattern, Predicate, Term, Value};

use super::Counters;
use crate::formula::ceiling;
use crate::grounding_observer::Event;
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

#[cfg(test)]
mod tests;

struct CatalogRows {
    catalog: Catalog,
    columns: Vec<BTreeMap<u32, Vec<usize>>>,
    old_rows: usize,
}

/// The sole owner of possible atoms and appendable equality postings.
#[derive(Default)]
pub(crate) struct SupportCatalog {
    rows: BTreeMap<Predicate, CatalogRows>,
    atoms: usize,
    entries: usize,
    index_bytes: usize,
}

impl SupportCatalog {
    /// Publish the complete owner only after both tuple and posting extension.
    /// A failed operation consumes its in-progress owner, so a partial index can
    /// never be reused by another support round.
    pub(super) fn insert(
        mut self,
        atom: Atom,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        let mut memory = Memory::new(self.index_bytes, limits, location);
        let source = match self.rows.entry(atom.predicate().clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                memory.add(
                    size_of::<CatalogRows>() - size_of::<Catalog>() + size_of::<Predicate>(),
                )?;
                let catalog = Catalog::new(
                    entry.key().clone(),
                    relation_limits(limits, counters, entry.key(), memory.remaining()?),
                )
                .map_err(|error| {
                    catalog_failure(error, limits, counters, memory.bytes, location)
                })?;
                counters.charge_work(catalog.construction().construction_work, limits, location)?;
                memory.add(catalog.retained_bytes())?;
                let mut columns = Vec::new();
                memory.reserve(&mut columns, entry.key().arity())?;
                for _ in 0..entry.key().arity() {
                    counters.work(limits, location)?;
                    columns.push(BTreeMap::new());
                }
                entry.insert(CatalogRows {
                    catalog,
                    columns,
                    old_rows: 0,
                })
            }
        };
        let old_bytes = source.catalog.retained_bytes();
        let outer_bytes = memory.bytes - old_bytes;
        let mut append_limits = relation_limits(
            limits,
            counters,
            source.catalog.predicate(),
            old_bytes + memory.remaining()?,
        );
        // Formula admission bounds retained column/row associations below.
        // A local dictionary-value ceiling is a different resource and must
        // not preempt that cumulative, duplicate-aware diagnostic.
        append_limits.max_values = usize::MAX;
        let receipt = source
            .catalog
            .insert(atom, append_limits)
            .map_err(|error| catalog_failure(error, limits, counters, outer_bytes, location))?;
        counters.charge_work(receipt.storage.construction_work, limits, location)?;
        memory.release(old_bytes);
        memory.add(receipt.storage.retained_bytes)?;
        if receipt.inserted {
            let arity = source.catalog.predicate().arity();
            ceiling(
                FormulaResource::SupportIndexEntries,
                self.entries as u128 + arity as u128,
                limits.max_support_index_entries as u128,
                location,
            )?;
            source.append_postings(receipt.row, &mut memory, counters)?;
            self.entries += arity;
            self.atoms += 1;
            counters.record(Event::SupportAtom);
        }
        self.index_bytes = memory.bytes;
        Ok(self)
    }

    /// Freeze the boundary before appending this completed round's new atoms.
    pub(super) fn advance(
        &mut self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        for rows in self.rows.values_mut() {
            counters.work(limits, location)?;
            rows.old_rows = rows.catalog.atoms().len();
        }
        Ok(())
    }

    /// Snapshot borrows prevent catalog growth until every join has ended.
    /// Existing dictionary, columns and postings are reused without a row scan.
    pub(crate) fn snapshot(
        &self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Support<'_>, FormulaFailure> {
        let mut memory = Memory::new(self.index_bytes, limits, location);
        memory.add(size_of::<Support<'_>>())?;
        let mut rows = BTreeMap::new();
        for (predicate, source) in &self.rows {
            counters.work(limits, location)?;
            memory.add(size_of::<RelationRows<'_>>() + size_of::<&Predicate>())?;
            rows.insert(
                predicate,
                RelationRows {
                    relation: source.catalog.view(),
                    columns: &source.columns,
                    catalog: &source.catalog,
                    old_rows: source.old_rows,
                    #[cfg(test)]
                    atoms: source.catalog.atoms(),
                },
            );
        }
        Ok(Support {
            rows,
            atoms: self.atoms,
            bytes: memory.bytes,
        })
    }
}

impl CatalogRows {
    fn append_postings(
        &mut self,
        row: usize,
        memory: &mut Memory<'_>,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        let view = self.catalog.view();
        memory.add(size_of::<Relation<'_>>())?;
        for (column, postings) in self.columns.iter_mut().enumerate() {
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
    location: Location,
) -> FormulaFailure {
    let failure = relation_failure(error.error, limits, counters, outer_bytes, location);
    match counters.charge_work(error.work, limits, location) {
        Ok(()) => failure,
        Err(charge) => charge,
    }
}

/// Immutable possible support, including snapshots between growth rounds.
/// This view alone establishes neither completion nor current-world truth.
#[derive(Default)]
pub(crate) struct Support<'source> {
    rows: BTreeMap<&'source Predicate, RelationRows<'source>>,
    atoms: usize,
    bytes: usize,
}

pub(super) struct RelationRows<'source> {
    pub(super) relation: Relation<'source>,
    pub(super) columns: &'source [BTreeMap<u32, Vec<usize>>],
    catalog: &'source Catalog,
    old_rows: usize,
    #[cfg(test)]
    pub(super) atoms: &'source [Atom],
}

impl Support<'_> {
    /// Predicates in this snapshot, borrowed from their sole atom owner.
    pub(crate) fn predicates(&self) -> impl Iterator<Item = &Predicate> {
        self.rows.keys().copied()
    }

    pub(super) fn len(&self) -> usize {
        self.atoms
    }

    pub(super) fn old_rows(&self, predicate: &Predicate) -> usize {
        self.rows.get(predicate).map_or(0, |rows| rows.old_rows)
    }

    pub(super) fn row_count(&self, predicate: &Predicate) -> usize {
        self.rows
            .get(predicate)
            .map_or(0, |rows| rows.relation.row_count())
    }

    pub(crate) fn rows(&self, predicate: &Predicate) -> impl Iterator<Item = Row<'_, '_>> {
        self.rows.get(predicate).into_iter().flat_map(|rows| {
            (0..rows.relation.row_count()).map(|row| rows.relation.row(row).expect("bounded row"))
        })
    }

    pub(super) fn row(&self, predicate: &Predicate, position: usize) -> Option<Row<'_, '_>> {
        self.rows.get(predicate)?.relation.row(position)
    }

    pub(super) fn contains(
        &self,
        key: &AtomKey<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        let Some(rows) = self.rows.get(key.predicate()) else {
            return Ok(false);
        };
        let owner_bytes = rows.catalog.retained_bytes();
        let mut checked = relation_limits(limits, counters, key.predicate(), owner_bytes);
        checked.max_values = usize::MAX;
        let receipt = rows.catalog.lookup_key(key, checked).map_err(|error| {
            catalog_failure(error, limits, counters, self.bytes - owner_bytes, location)
        })?;
        counters.charge_work(receipt.storage.construction_work, limits, location)?;
        Ok(receipt.row.is_some())
    }

    pub(super) fn probe(
        &self,
        pattern: &AtomPattern,
        values: &[Option<Value>],
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<&[usize]>, FormulaFailure> {
        counters.record(Event::JoinProbe);
        let Some(rows) = self.rows.get(pattern.predicate()) else {
            return Ok(Some(&[]));
        };
        let mut memory = Memory::new(self.bytes, limits, location);
        memory.add(size_of::<Vec<(usize, &Value)>>())?;
        let mut keys = Vec::new();
        for (column, term) in pattern.terms().iter().enumerate() {
            counters.work(limits, location)?;
            let value = match term {
                Term::Constant(value) => Some(value),
                Term::Variable(variable) => values[*variable].as_ref(),
            };
            if let Some(value) = value {
                memory.reserve(&mut keys, 1)?;
                keys.push((column, value));
            }
        }
        if keys.is_empty() {
            #[cfg(test)]
            super::postings::observe(rows, pattern, values, None);
            return Ok(None);
        }
        let query = rows
            .relation
            .query(
                &keys,
                relation_limits(
                    limits,
                    counters,
                    pattern.predicate(),
                    rows.relation.storage().retained_bytes + memory.remaining()?,
                ),
            )
            .map_err(|error| {
                relation_failure(
                    error,
                    limits,
                    counters,
                    memory.bytes - rows.relation.storage().retained_bytes,
                    location,
                )
            })?;
        counters.charge_work(query.work(), limits, location)?;
        memory.add(query.retained_bytes())?;
        let mut selected: Option<&[usize]> = None;
        if query.is_possible() {
            for equality in query.equalities() {
                let posting = rows.columns[equality.column()]
                    .get(&equality.value_id())
                    .map_or(&[][..], Vec::as_slice);
                if selected.is_none_or(|previous| posting.len() < previous.len()) {
                    selected = Some(posting);
                }
            }
        } else {
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
    predicate: &Predicate,
    bytes: usize,
) -> Limits {
    Limits {
        max_rows: limits.theory.max_atoms,
        max_columns: predicate.arity(),
        max_values: limits.max_support_index_entries,
        max_bytes: bytes,
        max_work: limits.max_work.saturating_sub(counters.work),
    }
}

fn failure(error: Failure, location: Location) -> FormulaFailure {
    FormulaFailure::SupportRelation { error, location }
}

/// Shared ceilings retain their formula-wide resource and cumulative amount.
/// Other core refusals retain their typed cause at the source location.
fn relation_failure(
    error: Failure,
    limits: &FormulaLimits,
    counters: &Counters,
    outer_bytes: usize,
    location: Location,
) -> FormulaFailure {
    let (resource, base, observed, limit) = match error {
        Failure::Limit {
            resource: Resource::Work,
            observed,
            ..
        } => (
            FormulaResource::Work,
            u128::from(counters.work),
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

/// Authored catalog/snapshot/index capacity. Nested atom payloads and
/// allocator/tree overhead retain separate bounds; this is not total RSS.
struct Memory<'limits> {
    bytes: usize,
    limits: &'limits FormulaLimits,
    location: Location,
}

impl<'limits> Memory<'limits> {
    fn new(bytes: usize, limits: &'limits FormulaLimits, location: Location) -> Self {
        Self {
            bytes,
            limits,
            location,
        }
    }

    fn add(&mut self, amount: usize) -> Result<(), FormulaFailure> {
        let next = self.bytes as u128 + amount as u128;
        ceiling(
            FormulaResource::SupportBytes,
            next,
            self.limits.max_support_bytes as u128,
            self.location,
        )?;
        self.bytes =
            usize::try_from(next).map_err(|_| failure(Failure::Overflow, self.location))?;
        Ok(())
    }

    fn remaining(&self) -> Result<usize, FormulaFailure> {
        ceiling(
            FormulaResource::SupportBytes,
            self.bytes as u128,
            self.limits.max_support_bytes as u128,
            self.location,
        )?;
        Ok(self.limits.max_support_bytes - self.bytes)
    }

    fn release(&mut self, amount: usize) {
        self.bytes = self.bytes.checked_sub(amount).expect("charged allocation");
    }

    fn reserve<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<(), FormulaFailure> {
        let needed = values
            .len()
            .checked_add(additional)
            .ok_or_else(|| failure(Failure::Overflow, self.location))?;
        if needed <= values.capacity() {
            return Ok(());
        }
        let proposed = needed.max(values.capacity().saturating_mul(2));
        let added = (proposed - values.capacity())
            .checked_mul(size_of::<T>())
            .ok_or_else(|| failure(Failure::Overflow, self.location))?;
        ceiling(
            FormulaResource::SupportBytes,
            self.bytes as u128 + added as u128,
            self.limits.max_support_bytes as u128,
            self.location,
        )?;
        let previous = values.capacity();
        values
            .try_reserve_exact(proposed - values.len())
            .map_err(|_| failure(Failure::Allocation, self.location))?;
        self.add(
            (values.capacity() - previous)
                .checked_mul(size_of::<T>())
                .ok_or_else(|| failure(Failure::Overflow, self.location))?,
        )
    }
}
