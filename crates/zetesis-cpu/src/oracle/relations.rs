//! Ordered tuple access over borrowed source snapshots or retained catalogs.
//!
//! Catalog calls receive the remaining work quota. Control is observed before
//! and after each whole catalog operation, rather than within its comparisons,
//! reservations and ID shifts. A stopped operation never returns a closure.

use std::collections::{BTreeMap, btree_map::Entry};

use zetesis_core::{
    Atom, Model, Predicate,
    relation::{Catalog, CatalogFailure, Failure, Limits, Resource},
};

use super::{Relations, Work};
use crate::Stop;

/// Both views use complete tuple storage order. Their positions are access
/// positions, not persistent equality IDs or global source occurrence IDs.
#[derive(Clone, Copy)]
pub(super) enum Rows<'a> {
    Borrowed(&'a [&'a Atom]),
    Catalog(&'a Catalog),
}

impl<'a> Rows<'a> {
    pub(super) fn len(self) -> usize {
        match self {
            Self::Borrowed(rows) => rows.len(),
            Self::Catalog(catalog) => catalog.atoms().len(),
        }
    }

    pub(super) fn get(self, position: usize) -> Option<&'a Atom> {
        match self {
            Self::Borrowed(rows) => rows.get(position).copied(),
            Self::Catalog(catalog) => catalog.ordered_row(position),
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
#[derive(Default)]
pub(super) struct Catalogs {
    relations: BTreeMap<Predicate, Catalog>,
    atoms: usize,
}

impl Relational for Catalogs {
    fn rows(&self, predicate: &Predicate) -> Rows<'_> {
        self.relations
            .get(predicate)
            .map_or(Rows::Borrowed(&[]), Rows::Catalog)
    }
}

impl Catalogs {
    pub(super) const fn len(&self) -> usize {
        self.atoms
    }

    pub(super) fn contains(&self, atom: &Atom, work: &mut Work<'_>) -> Result<bool, Stop> {
        work.control.poll()?;
        let Some(catalog) = self.relations.get(atom.predicate()) else {
            return Ok(false);
        };
        let lookup = completed(catalog.lookup(atom, limits(work)), work)?;
        account(work, lookup.storage.construction_work)?;
        Ok(lookup.row.is_some())
    }

    pub(super) fn insert(&mut self, atom: Atom, work: &mut Work<'_>) -> Result<(), Stop> {
        work.control.poll()?;
        let catalog = match self.relations.entry(atom.predicate().clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let catalog = completed(Catalog::new(entry.key().clone(), limits(work)), work)?;
                account(work, catalog.construction().construction_work)?;
                entry.insert(catalog)
            }
        };
        let insertion = completed(catalog.insert(atom, limits(work)), work)?;
        account(work, insertion.storage.construction_work)?;
        if insertion.inserted {
            self.atoms += 1;
        }
        Ok(())
    }

    pub(super) fn into_model(self) -> Model {
        // Model's canonical BTreeSet is the final public interpretation. Moving
        // its atoms consumes the catalogs; no second live tuple owner is kept.
        // Model's final O(n log n) canonicalization/tree allocation is not in
        // the source operation schedule; input atom count is already bounded.
        Model::new(self.relations.into_values().flat_map(Catalog::into_atoms))
    }
}

fn limits(work: &Work<'_>) -> Limits {
    Limits {
        max_rows: work.limits.max_derived_atoms,
        max_columns: usize::MAX,
        max_values: usize::MAX,
        max_bytes: usize::MAX,
        max_work: work.limits.max_work - work.statistics.work,
    }
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

fn completed<T>(result: Result<T, CatalogFailure>, work: &mut Work<'_>) -> Result<T, Stop> {
    match result {
        Ok(value) => Ok(value),
        Err(failure) => {
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
            catalogs.insert(atom.clone(), &mut work).unwrap();
        }
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
            .insert(atom(Value::String("payload".into())), &mut work)
            .unwrap();
        let original = &catalogs.relations[&predicate].atoms()[0];
        let row = catalogs.rows(&predicate).get(0).unwrap();
        assert!(std::ptr::eq(original, row));
    }
}
