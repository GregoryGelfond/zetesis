//! Ordered tuple access over borrowed source snapshots or retained catalogs.
//!
//! Catalog calls receive the remaining work quota. Control is observed before
//! and after each whole catalog operation, rather than within its comparisons,
//! reservations and index planning. A stopped operation never returns a closure.

use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    mem::size_of,
};

mod storage;
pub(super) use storage::atom_bytes;

use zetesis_core::{
    Atom, AtomKey, Model, Predicate,
    relation::{
        Catalog, CatalogFailure, Failure, Insertion, Limits, OrderedRows, Resource, Storage,
    },
};

use super::{Relations, Work};
use crate::Stop;

/// Both views use complete tuple storage order. Their positions are access
/// positions, not persistent equality IDs or global source occurrence IDs.
#[derive(Clone, Copy)]
pub(super) enum Rows<'a> {
    Borrowed(&'a [&'a Atom]),
    Catalog(OrderedRows<'a>),
}

impl<'a> Rows<'a> {
    pub(super) fn len(self) -> usize {
        match self {
            Self::Borrowed(rows) => rows.len(),
            Self::Catalog(rows) => rows.len(),
        }
    }

    pub(super) fn get(self, position: usize) -> Option<&'a Atom> {
        match self {
            Self::Borrowed(rows) => rows.get(position).copied(),
            Self::Catalog(rows) => rows.get(position),
        }
    }
}

pub(super) trait Relational {
    fn rows(&self, predicate: &Predicate) -> Rows<'_>;
}

impl Relational for Relations<'_> {
    fn rows(&self, predicate: &Predicate) -> Rows<'_> {
        Rows::Borrowed(self.get(predicate).map_or(&[], Vec::as_slice))
    }
}

/// The scalar closure owns each atom once until final interpretation assembly.
/// Views borrow the retained row index; rounds do not rebuild tuple snapshots.
pub(super) struct Catalogs {
    relations: BTreeMap<Predicate, Catalog>,
    atoms: usize,
    bytes: u128,
}

impl Default for Catalogs {
    fn default() -> Self {
        Self {
            relations: BTreeMap::new(),
            atoms: 0,
            bytes: (size_of::<Self>() + size_of::<BTreeSet<Atom>>()) as u128,
        }
    }
}

impl Relational for Catalogs {
    fn rows(&self, predicate: &Predicate) -> Rows<'_> {
        self.relations
            .get(predicate)
            .map_or(Rows::Borrowed(&[]), |catalog| {
                Rows::Catalog(
                    catalog
                        .ordered()
                        .expect("round prepared its published extent"),
                )
            })
    }
}

impl Catalogs {
    pub(super) const fn len(&self) -> usize {
        self.atoms
    }

    pub(super) fn prepare(&mut self, work: &mut Work<'_>) -> Result<(), Stop> {
        storage::admit(work, self.bytes)?;
        storage::record(work, self.bytes)?;
        for catalog in self.relations.values_mut() {
            work.control.poll()?;
            let old = catalog.retained_bytes() as u128;
            let other = self.bytes.checked_sub(old).ok_or(Stop::InvalidProgram)?;
            let result = catalog
                .prepare_ordered(limits(work, other)?)
                .map(OrderedRows::storage);
            self.bytes = other
                .checked_add(catalog.retained_bytes() as u128)
                .ok_or(Stop::StorageLimit)?;
            let receipt = completed(result, other, work)?;
            account_storage(work, other, receipt)?;
        }
        Ok(())
    }

    pub(super) fn contains(
        &self,
        key: &AtomKey<'_>,
        pending: u128,
        work: &mut Work<'_>,
    ) -> Result<bool, Stop> {
        work.control.poll()?;
        let Some(catalog) = self.relations.get(key.predicate()) else {
            return Ok(false);
        };
        let other = self
            .bytes
            .checked_sub(catalog.retained_bytes() as u128)
            .and_then(|bytes| bytes.checked_add(pending))
            .ok_or(Stop::StorageLimit)?;
        let lookup = completed(catalog.lookup_key(key, limits(work, other)?), other, work)?;
        account_storage(work, other, lookup.storage)?;
        Ok(lookup.row.is_some())
    }

    pub(super) fn pending(
        &self,
        key: AtomKey<'_>,
        pending: u128,
        work: &mut Work<'_>,
    ) -> Result<(Atom, u128), Stop> {
        storage::pending(
            key,
            self.bytes.checked_add(pending).ok_or(Stop::StorageLimit)?,
            work,
        )
    }

    pub(super) fn insert(
        &mut self,
        atom: Atom,
        pending: u128,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        work.control.poll()?;
        let input = atom_bytes(&atom, work)?;
        let held = pending.checked_add(input).ok_or(Stop::StorageLimit)?;
        storage::admit(
            work,
            self.bytes.checked_add(held).ok_or(Stop::StorageLimit)?,
        )?;
        storage::record(
            work,
            self.bytes.checked_add(held).ok_or(Stop::StorageLimit)?,
        )?;
        if !self.relations.contains_key(atom.predicate()) {
            self.create(atom.predicate(), held, work)?;
        }
        let catalog = self
            .relations
            .get_mut(atom.predicate())
            .ok_or(Stop::InvalidProgram)?;
        let old = catalog.retained_bytes() as u128;
        let other = self
            .bytes
            .checked_sub(old)
            .and_then(|bytes| bytes.checked_add(held))
            .ok_or(Stop::StorageLimit)?;
        let result = catalog.insert(atom, limits(work, other)?);
        self.bytes = self
            .bytes
            .checked_sub(old)
            .and_then(|bytes| bytes.checked_add(catalog.retained_bytes() as u128))
            .ok_or(Stop::StorageLimit)?;
        if let Ok(insertion) = &result
            && insertion.inserted
        {
            self.bytes = self
                .bytes
                .checked_add(input - size_of::<Atom>() as u128)
                .ok_or(Stop::StorageLimit)?;
        }
        let insertion = completed(result, other, work)?;
        storage::record(
            work,
            other
                .checked_add(insertion.storage.peak_construction_bytes as u128)
                .ok_or(Stop::StorageLimit)?,
        )?;
        self.publish(insertion, work)
    }

    fn create(
        &mut self,
        predicate: &Predicate,
        held: u128,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        let base = self
            .bytes
            .checked_add(held)
            .and_then(|bytes| bytes.checked_add(size_of::<Predicate>() as u128))
            .ok_or(Stop::StorageLimit)?;
        let headers = base
            .checked_add(size_of::<Catalog>() as u128)
            .ok_or(Stop::StorageLimit)?;
        let key = storage::predicate(predicate, headers, work)?;
        let key_bytes = key.payload_capacity_bytes() as u128;
        let signature = storage::predicate(
            predicate,
            headers.checked_add(key_bytes).ok_or(Stop::StorageLimit)?,
            work,
        )?;
        let names = key_bytes
            .checked_add(signature.payload_capacity_bytes() as u128)
            .ok_or(Stop::StorageLimit)?;
        let other = base.checked_add(names).ok_or(Stop::StorageLimit)?;
        let catalog = completed(Catalog::new(signature, limits(work, other)?), other, work)?;
        account_storage(work, other, catalog.construction())?;
        self.bytes = self
            .bytes
            .checked_add(size_of::<Predicate>() as u128)
            .and_then(|bytes| bytes.checked_add(names))
            .and_then(|bytes| bytes.checked_add(catalog.retained_bytes() as u128))
            .ok_or(Stop::StorageLimit)?;
        match self.relations.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(catalog);
            }
            Entry::Occupied(_) => unreachable!("exclusive absent predicate remains absent"),
        }
        Ok(())
    }

    fn publish(&mut self, insertion: Insertion, work: &mut Work<'_>) -> Result<(), Stop> {
        // The catalog has published its tuple. Keep the enclosing count coherent
        // before accounting observes a cancellation or deadline.
        if insertion.inserted {
            self.atoms += 1;
        }
        account(work, insertion.storage.construction_work)
    }

    pub(super) fn into_model(self) -> Model {
        // Move the completed closure into its shared model catalog. No second
        // live tuple owner is kept. Model's final O(n log n) canonicalization
        // and catalog/selection allocation are not in
        // the source operation schedule; input atom count is already bounded.
        Model::new(self.relations.into_values().flat_map(Catalog::into_atoms))
    }
}

fn limits(work: &Work<'_>, other: u128) -> Result<Limits, Stop> {
    let remaining = (work.limits.max_closure_bytes as u128)
        .checked_sub(other)
        .ok_or(Stop::StorageLimit)?;
    Ok(Limits {
        max_rows: work.limits.max_derived_atoms,
        max_columns: usize::MAX,
        max_values: usize::MAX,
        max_bytes: usize::try_from(remaining).map_err(|_| Stop::StorageLimit)?,
        max_work: work.limits.max_work - work.statistics.work,
    })
}

fn account_storage(work: &mut Work<'_>, other: u128, receipt: Storage) -> Result<(), Stop> {
    storage::record(
        work,
        other
            .checked_add(receipt.peak_construction_bytes as u128)
            .ok_or(Stop::StorageLimit)?,
    )?;
    account(work, receipt.construction_work)
}

fn account(work: &mut Work<'_>, amount: u128) -> Result<(), Stop> {
    let amount = u64::try_from(amount).map_err(|_| Stop::InvalidProgram)?;
    work.statistics.work = work
        .statistics
        .work
        .checked_add(amount)
        .ok_or(Stop::InvalidProgram)?;
    if work.statistics.work > work.limits.max_work {
        return Err(Stop::InvalidProgram);
    }
    work.statistics.catalog_work = work
        .statistics
        .catalog_work
        .checked_add(amount)
        .ok_or(Stop::InvalidProgram)?;
    work.control.poll()
}

fn completed<T>(
    result: Result<T, CatalogFailure>,
    other: u128,
    work: &mut Work<'_>,
) -> Result<T, Stop> {
    match result {
        Ok(value) => Ok(value),
        Err(failure) => {
            storage::record(
                work,
                other
                    .checked_add(failure.peak_construction_bytes as u128)
                    .ok_or(Stop::StorageLimit)?,
            )?;
            account(work, failure.work)?;
            Err(match failure.error {
                Failure::Limit {
                    resource: Resource::Work,
                    ..
                } => Stop::WorkLimit,
                Failure::Limit {
                    resource: Resource::Rows,
                    ..
                } => Stop::DerivedAtomLimit,
                Failure::Limit {
                    resource: Resource::Bytes,
                    ..
                } => Stop::StorageLimit,
                Failure::Allocation | Failure::Overflow | Failure::Limit { .. } => Stop::Allocation,
                _ => Stop::InvalidProgram,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Control;
    use zetesis_core::Value;

    fn atom(value: Value) -> Atom {
        Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap()
    }

    #[test]
    fn retained_views_preserve_complete_tuple_order() {
        let predicate = Predicate::new("p", 1).unwrap();
        let control = Control::default();
        let mut work = Work::source(&control, u64::MAX);
        work.limits.max_derived_atoms = 3;
        let mut catalogs = Catalogs::default();
        let mut expected = [
            atom(Value::Symbol("a".into())),
            atom(Value::Number(2)),
            atom(Value::String("a".into())),
        ];
        for atom in &expected {
            catalogs.insert(atom.clone(), 0, &mut work).unwrap();
        }
        catalogs.prepare(&mut work).unwrap();
        expected.sort();
        let rows = catalogs.rows(&predicate);
        for (index, expected) in expected.iter().enumerate() {
            assert_eq!(rows.get(index), Some(expected));
        }
        assert!(rows.get(expected.len()).is_none());
    }

    #[test]
    fn ordered_rows_borrow_the_owning_tuple() {
        let predicate = Predicate::new("p", 1).unwrap();
        let control = Control::default();
        let mut work = Work::source(&control, u64::MAX);
        work.limits.max_derived_atoms = 1;
        let mut catalogs = Catalogs::default();
        catalogs
            .insert(atom(Value::String("payload".into())), 0, &mut work)
            .unwrap();
        catalogs.prepare(&mut work).unwrap();
        let original = &catalogs.relations[&predicate].atoms()[0];
        let row = catalogs.rows(&predicate).get(0).unwrap();
        assert!(std::ptr::eq(original, row));
    }

    #[test]
    fn cancelled_receipt_preserves_catalog_count() {
        let control = Control::default();
        let mut work = Work::source(&control, u64::MAX);
        work.limits.max_derived_atoms = 1;
        let mut catalogs = Catalogs::default();
        let tuple = atom(Value::Number(1));
        let predicate = tuple.predicate().clone();
        let mut catalog = Catalog::new(predicate.clone(), limits(&work, 0).unwrap()).unwrap();
        let receipt = catalog.insert(tuple, limits(&work, 0).unwrap()).unwrap();
        catalogs.relations.insert(predicate, catalog);
        control.cancel();
        assert_eq!(catalogs.publish(receipt, &mut work), Err(Stop::Cancelled));
        assert_eq!(
            catalogs.len(),
            catalogs
                .relations
                .values()
                .map(|catalog| catalog.atoms().len())
                .sum::<usize>()
        );
    }
}
