//! One atom owner and immutable column views between support-growth rounds.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::mem::size_of;

use themelios_base::span::Location;
use zetesis_core::relation::{Failure, Limits, Relation, Resource, Row};
use zetesis_core::{Atom, AtomPattern, Predicate, Term, Value};

use super::Counters;
use crate::formula::ceiling;
use crate::grounding_observer::Event;
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

#[cfg(test)]
mod tests;

#[derive(Default)]
struct CatalogRows {
    atoms: Vec<Atom>,
    ordered: Vec<usize>,
}

/// The sole owner of possible atoms. Row IDs survive vector reallocation.
#[derive(Default)]
pub(crate) struct SupportCatalog {
    rows: BTreeMap<Predicate, CatalogRows>,
    atoms: usize,
    entries: usize,
    index_bytes: usize,
}

impl SupportCatalog {
    pub(super) fn insert(
        &mut self,
        atom: Atom,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::SupportIndexEntries,
            self.entries as u128 + atom.values().len() as u128,
            limits.max_support_index_entries as u128,
            location,
        )?;
        let is_new = !self.rows.contains_key(atom.predicate());
        let mut memory = Memory::new(self.index_bytes, limits, location);
        if is_new {
            memory.add(size_of::<Vec<usize>>())?;
        }
        let rows = self.rows.entry(atom.predicate().clone()).or_default();
        let Err(insertion) = rows
            .ordered
            .binary_search_by(|&row| rows.atoms[row].cmp(&atom))
        else {
            return Ok(());
        };
        memory.reserve(&mut rows.ordered, 1)?;
        rows.atoms
            .try_reserve(1)
            .map_err(|_| failure(Failure::Allocation, location))?;
        // Insertion shifts only row identifiers; the authoritative append order
        // and all prior local row positions remain unchanged.
        counters.charge_work(
            (rows.ordered.len() - insertion + 1) as u128,
            limits,
            location,
        )?;
        rows.ordered.insert(insertion, rows.atoms.len());
        self.entries += atom.values().len();
        rows.atoms.push(atom);
        self.atoms += 1;
        self.index_bytes = memory.bytes;
        counters.record(Event::SupportAtom);
        Ok(())
    }

    /// Snapshot borrows prevent catalog growth until every join has ended.
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
            // The relation's own accounting includes its object. Other row
            // metadata is charged here and transferred into the table entry;
            // there is no separately reserved destination Relation object.
            memory.add(
                size_of::<RelationRows<'_>>() - size_of::<Relation<'_>>() + size_of::<&Predicate>(),
            )?;
            let relation = Relation::from_atoms(
                predicate,
                &source.atoms,
                relation_limits(limits, counters, predicate, memory.remaining()?),
            )
            .map_err(|error| relation_failure(error, limits, counters, memory.bytes, location))?;
            counters.charge_work(relation.storage().construction_work, limits, location)?;
            memory.add(relation.storage().retained_bytes)?;
            let mut columns = Vec::new();
            memory.reserve(&mut columns, predicate.arity())?;
            for column in 0..predicate.arity() {
                // Reserved column slots coexist with this local map header
                // until the completed map moves into its slot.
                memory.add(size_of::<BTreeMap<u32, Vec<usize>>>())?;
                let mut postings = BTreeMap::<u32, Vec<usize>>::new();
                for (row, &id) in relation
                    .column(column)
                    .expect("checked column")
                    .iter()
                    .enumerate()
                {
                    counters.work(limits, location)?;
                    if !postings.contains_key(&id) {
                        memory.add(size_of::<(u32, Vec<usize>)>())?;
                    }
                    memory.reserve(postings.entry(id).or_default(), 1)?;
                    postings.get_mut(&id).expect("posting inserted").push(row);
                    counters.record(Event::SupportIndexEntry);
                }
                columns.push(postings);
                memory.release(size_of::<BTreeMap<u32, Vec<usize>>>());
            }
            rows.insert(
                predicate,
                RelationRows {
                    relation,
                    columns,
                    ordered: &source.ordered,
                    #[cfg(test)]
                    atoms: &source.atoms,
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

/// Immutable possible support; it does not establish current-world truth.
#[derive(Default)]
pub(crate) struct Support<'source> {
    rows: BTreeMap<&'source Predicate, RelationRows<'source>>,
    atoms: usize,
    bytes: usize,
}

pub(super) struct RelationRows<'source> {
    pub(super) relation: Relation<'source>,
    pub(super) columns: Vec<BTreeMap<u32, Vec<usize>>>,
    ordered: &'source [usize],
    #[cfg(test)]
    pub(super) atoms: &'source [Atom],
}

impl Support<'_> {
    pub(super) fn len(&self) -> usize {
        self.atoms
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

    pub(super) fn contains(&self, atom: &Atom) -> bool {
        self.rows.get(atom.predicate()).is_some_and(|rows| {
            rows.ordered
                .binary_search_by(|&position| {
                    let row = rows.relation.row(position).expect("catalog row index");
                    for (column, value) in atom.values().iter().enumerate() {
                        let order = row.value(column).expect("checked arity").cmp(value);
                        if order != Ordering::Equal {
                            return order;
                        }
                    }
                    Ordering::Equal
                })
                .is_ok()
        })
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

/// Authored snapshot/index capacity. Source atoms and allocator/tree overhead
/// retain separate bounds; this is not a total grounder-memory measurement.
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
