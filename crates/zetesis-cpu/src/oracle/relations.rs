//! Ordered tuple access over borrowed source snapshots or retained catalogs.
//!
//! Catalog calls receive the remaining work quota. Control is observed before
//! and after each whole catalog operation, rather than within its comparisons,
//! reservations and index planning. A stopped operation never returns a closure.

use std::{collections::BTreeSet, mem::size_of};

mod partition;
use partition::Partition;

pub(super) mod storage;
pub(super) use storage::atom_bytes;

use zetesis_core::{
    Atom, AtomKey, Model, Predicate,
    relation::{Catalog, CatalogFailure, Failure, Insertion, Limits, Resource, Storage},
};

use super::Work;
use crate::Stop;

/// A view is one or more runs, each in complete tuple storage order. A row is
/// addressed by its run and its position within the run; positions are access
/// positions, not persistent equality IDs or global source occurrence IDs. A
/// borrowed snapshot is one run; a catalog view is its levels and its tail,
/// any of which a selection may leave out.
#[derive(Clone, Copy)]
pub(super) enum Rows<'a> {
    Borrowed(&'a [&'a Atom]),
    Runs {
        atoms: &'a [Atom],
        levels: &'a [Vec<usize>],
        tail: &'a [usize],
    },
}

impl<'a> Rows<'a> {
    /// Number of runs, counting an empty tail as none.
    pub(super) fn runs(self) -> usize {
        match self {
            Self::Borrowed(_) => 1,
            Self::Runs { levels, tail, .. } => levels.len() + usize::from(!tail.is_empty()),
        }
    }

    pub(super) fn run_len(self, run: usize) -> usize {
        match self {
            Self::Borrowed(rows) => rows.len(),
            Self::Runs { levels, tail, .. } => levels.get(run).map_or(tail.len(), Vec::len),
        }
    }

    /// Every row of every run, run by run. Runs are each in canonical order;
    /// the sequence across runs is not.
    #[cfg(test)]
    pub(super) fn all(self) -> Vec<&'a Atom> {
        (0..self.runs())
            .flat_map(|run| (0..self.run_len(run)).map(move |position| (run, position)))
            .map(|(run, position)| self.get(run, position).expect("in range"))
            .collect()
    }

    pub(super) fn get(self, run: usize, position: usize) -> Option<&'a Atom> {
        match self {
            Self::Borrowed(rows) => rows.get(position).copied(),
            Self::Runs {
                atoms,
                levels,
                tail,
            } => levels
                .get(run)
                .map_or(tail, Vec::as_slice)
                .get(position)
                .and_then(|&id| atoms.get(id)),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RowSet {
    Current,
    Old,
    New,
}

/// A source of rows per predicate. A join resolves a predicate to a handle
/// once, when it enters a depth, and reads rows by handle on every probe
/// after that; the handle is valid until the source gains a relation, which
/// never happens inside one round.
pub(super) trait Relational {
    /// The handle of the predicate's relation, or `None` when it has no rows.
    fn resolve(&self, predicate: &Predicate) -> Option<usize>;

    /// The rows of a resolved relation in the requested set.
    fn rows_at(&self, handle: usize, set: RowSet) -> Result<Rows<'_>, Stop>;

    #[cfg(test)]
    fn rows(&self, predicate: &Predicate) -> Rows<'_> {
        self.resolve(predicate)
            .and_then(|handle| self.rows_at(handle, RowSet::Current).ok())
            .unwrap_or(Rows::Borrowed(&[]))
    }

    #[cfg(test)]
    fn selected(&self, predicate: &Predicate, set: RowSet) -> Result<Rows<'_>, Stop> {
        match self.resolve(predicate) {
            Some(handle) => self.rows_at(handle, set),
            None => Ok(Rows::Borrowed(&[])),
        }
    }
}

/// A handle resolved for one join depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Slot {
    /// The depth has not been entered in this visit.
    Unresolved,
    /// The source has no relation for the depth's predicate.
    Absent,
    /// The relation's handle in the source.
    At(usize),
}

pub(super) struct Relation {
    catalog: Catalog,
    partition: Partition,
}

/// Borrowed rows grouped by predicate, in canonical atom order: the groups are
/// in predicate order and each group's rows in storage order, as a model's
/// atoms arrive. One run per predicate.
pub(super) struct Relations<'a>(Vec<(&'a Predicate, Vec<&'a Atom>)>);

impl<'a> Relations<'a> {
    pub(super) const fn new() -> Self {
        Self(Vec::new())
    }

    /// Append the next atom of a canonically ordered sequence.
    pub(super) fn push(&mut self, atom: &'a Atom) {
        match self.0.last_mut() {
            Some((predicate, rows)) if *predicate == atom.predicate() => rows.push(atom),
            _ => self.0.push((atom.predicate(), vec![atom])),
        }
    }
}

impl<'a, const N: usize> From<[(&'a Predicate, Vec<&'a Atom>); N]> for Relations<'a> {
    fn from(groups: [(&'a Predicate, Vec<&'a Atom>); N]) -> Self {
        let mut groups = groups.to_vec();
        groups.sort_by(|left, right| left.0.cmp(right.0));
        Self(groups)
    }
}

impl Relational for Relations<'_> {
    fn resolve(&self, predicate: &Predicate) -> Option<usize> {
        self.0
            .binary_search_by(|(candidate, _)| (*candidate).cmp(predicate))
            .ok()
    }

    fn rows_at(&self, handle: usize, set: RowSet) -> Result<Rows<'_>, Stop> {
        if set != RowSet::Current {
            return Err(Stop::InvalidProgram);
        }
        Ok(Rows::Borrowed(
            self.0.get(handle).map_or(&[], |(_, rows)| rows),
        ))
    }
}

/// The scalar closure owns each atom once until final interpretation assembly.
/// Views borrow the retained row index; rounds do not rebuild tuple snapshots.
/// Relations are kept in predicate order, so the final model is their
/// concatenation and a handle is a position in this vector.
pub(super) struct Catalogs {
    relations: Vec<Relation>,
    atoms: usize,
    bytes: u128,
    overhead: u128,
}

impl Default for Catalogs {
    fn default() -> Self {
        Self {
            relations: Vec::new(),
            atoms: 0,
            bytes: (size_of::<Self>() + size_of::<BTreeSet<Atom>>()) as u128,
            overhead: 0,
        }
    }
}

impl Relational for Catalogs {
    fn resolve(&self, predicate: &Predicate) -> Option<usize> {
        self.find(predicate).ok()
    }

    fn rows_at(&self, handle: usize, set: RowSet) -> Result<Rows<'_>, Stop> {
        let relation = self.relations.get(handle).ok_or(Stop::InvalidProgram)?;
        relation.partition.rows(&relation.catalog, set)
    }
}

impl Catalogs {
    pub(super) const fn len(&self) -> usize {
        self.atoms
    }

    #[cfg(test)]
    pub(super) fn relation(&self, predicate: &Predicate) -> &Relation {
        &self.relations[self.find(predicate).expect("relation exists")]
    }

    /// The relation's position, or where one for the predicate would go.
    fn find(&self, predicate: &Predicate) -> Result<usize, usize> {
        self.relations
            .binary_search_by(|relation| relation.catalog.predicate().cmp(predicate))
    }

    pub(super) fn prepare(&mut self, work: &mut Work<'_>) -> Result<(), Stop> {
        storage::admit(work, self.bytes)?;
        storage::record(work, self.bytes)?;
        for relation in &mut self.relations {
            let catalog = &mut relation.catalog;
            work.control.poll()?;
            let old = catalog.retained_bytes() as u128;
            let other = self.bytes.checked_sub(old).ok_or(Stop::InvalidProgram)?;
            let result = catalog
                .prepare_ordered(limits(work, other)?)
                .map(|prepared| prepared.storage);
            self.bytes = other
                .checked_add(catalog.retained_bytes() as u128)
                .ok_or(Stop::StorageLimit)?;
            let receipt = completed(result, other, work)?;
            account_storage(work, other, receipt)?;
        }
        Ok(())
    }

    pub(super) fn prepare_delta(&mut self, work: &mut Work<'_>) -> Result<(), Stop> {
        self.prepare(work)?;
        for relation in &self.relations {
            relation.partition.prepare(&relation.catalog, work)?;
        }
        Ok(())
    }

    /// Advance old truth only after every source partition has completed, and
    /// before appending the new heads. No borrowed round view is still live.
    pub(super) fn advance(&mut self, work: &mut Work<'_>) -> Result<(), Stop> {
        for relation in &mut self.relations {
            relation
                .partition
                .advance(relation.catalog.atoms().len(), work)?;
        }
        Ok(())
    }

    /// Predicates whose relation gained rows since the cutoff advanced, in
    /// canonical predicate order.
    pub(super) fn predicates_with_new(&self) -> impl Iterator<Item = &Predicate> {
        self.relations
            .iter()
            .filter(|relation| relation.partition.has_new(relation.catalog.atoms().len()))
            .map(|relation| relation.catalog.predicate())
    }

    pub(super) fn has_new(&self, predicate: &Predicate, work: &mut Work<'_>) -> Result<bool, Stop> {
        work.tick()?;
        Ok(self.find(predicate).ok().is_some_and(|handle| {
            let relation = &self.relations[handle];
            relation.partition.has_new(relation.catalog.atoms().len())
        }))
    }

    pub(super) fn contains(
        &self,
        key: &AtomKey<'_>,
        pending: u128,
        work: &mut Work<'_>,
    ) -> Result<bool, Stop> {
        work.control.poll()?;
        let Ok(handle) = self.find(key.predicate()) else {
            return Ok(false);
        };
        let catalog = &self.relations[handle].catalog;
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
        let handle = match self.find(atom.predicate()) {
            Ok(handle) => handle,
            Err(position) => {
                self.create(position, atom.predicate(), held, work)?;
                position
            }
        };
        let catalog = &mut self.relations[handle].catalog;
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

    /// Create the relation at its sorted position. The catalog's signature is
    /// the relation's only copy of the predicate.
    fn create(
        &mut self,
        position: usize,
        predicate: &Predicate,
        held: u128,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        let base = self
            .bytes
            .checked_add(held)
            .and_then(|bytes| {
                bytes.checked_add((size_of::<Relation>() - size_of::<Catalog>()) as u128)
            })
            .ok_or(Stop::StorageLimit)?;
        let headers = base
            .checked_add(size_of::<Catalog>() as u128)
            .ok_or(Stop::StorageLimit)?;
        let signature = storage::predicate(predicate, headers, work)?;
        let names = signature.payload_capacity_bytes() as u128;
        let other = base.checked_add(names).ok_or(Stop::StorageLimit)?;
        let catalog = completed(Catalog::new(signature, limits(work, other)?), other, work)?;
        account_storage(work, other, catalog.construction())?;
        self.bytes = self
            .bytes
            .checked_add((size_of::<Relation>() - size_of::<Catalog>()) as u128)
            .and_then(|bytes| bytes.checked_add(names))
            .and_then(|bytes| bytes.checked_add(catalog.retained_bytes() as u128))
            .ok_or(Stop::StorageLimit)?;
        self.relations
            .try_reserve(1)
            .map_err(|_| Stop::Allocation)?;
        work.charge(self.relations.len() - position)?;
        self.relations.insert(
            position,
            Relation {
                catalog,
                partition: Partition::default(),
            },
        );
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

    pub(super) fn owned_bytes(&self) -> u128 {
        self.bytes - self.overhead
    }

    pub(super) fn set_overhead(&mut self, bytes: u128, work: &mut Work<'_>) -> Result<(), Stop> {
        let total = self
            .owned_bytes()
            .checked_add(bytes)
            .ok_or(Stop::StorageLimit)?;
        storage::admit(work, total)?;
        storage::record(work, total)?;
        self.bytes = total;
        self.overhead = bytes;
        Ok(())
    }

    pub(super) fn take_model(&mut self, work: &mut Work<'_>) -> Result<Model, Stop> {
        // Final interpretation assembly owns these headers; its allocation and
        // canonicalization remain separate from the closure capacity allowance.
        // Each payload is moved once. Empty indexes and predicate owners remain.
        let mut atoms = Vec::new();
        atoms
            .try_reserve_exact(self.atoms)
            .map_err(|_| Stop::Allocation)?;
        // Relations are in predicate order and each catalog knows its rows'
        // canonical order, so the model is their concatenation: no sort.
        for relation in &mut self.relations {
            relation.partition.reset(work)?;
            let catalog = &mut relation.catalog;
            work.tick()?;
            let mut payload = 0_u128;
            for atom in catalog.atoms() {
                payload = payload
                    .checked_add(atom_bytes(atom, work)? - size_of::<Atom>() as u128)
                    .ok_or(Stop::StorageLimit)?;
            }
            let old = catalog.retained_bytes() as u128;
            let other = self.bytes.checked_sub(old).ok_or(Stop::InvalidProgram)?;
            let canonical = completed(catalog.canonical(limits(work, other)?), other, work)?;
            account_storage(work, other, canonical.storage)?;
            let order = canonical.ids;
            let result = catalog.take_atoms(limits(work, other)?);
            self.bytes = other
                .checked_add(catalog.retained_bytes() as u128)
                .ok_or(Stop::StorageLimit)?;
            let extracted = completed(result, other, work)?;
            self.bytes = self
                .bytes
                .checked_sub(payload)
                .ok_or(Stop::InvalidProgram)?;
            account_storage(work, other, extracted.storage)?;
            // Move each atom to its canonical position; the slots and the
            // order are transient and released with this loop iteration.
            storage::admit(
                work,
                self.bytes
                    .checked_add((extracted.atoms.len() * size_of::<Atom>()) as u128)
                    .ok_or(Stop::StorageLimit)?,
            )?;
            work.charge(order.len())?;
            let mut slots: Vec<Option<Atom>> = extracted.atoms.into_iter().map(Some).collect();
            for id in order {
                atoms.push(
                    slots
                        .get_mut(id)
                        .and_then(Option::take)
                        .ok_or(Stop::InvalidProgram)?,
                );
            }
        }
        self.atoms = 0;
        Ok(Model::from_ordered(atoms))
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
        // The first preparation is one run, in canonical order.
        let rows = catalogs.rows(&predicate);
        assert_eq!(rows.runs(), 1);
        assert_eq!(rows.all(), expected.iter().collect::<Vec<_>>());
        assert!(rows.get(0, expected.len()).is_none());
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
        let original = &catalogs.relation(&predicate).catalog.atoms()[0];
        let row = catalogs.rows(&predicate).get(0, 0).unwrap();
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
        catalogs.relations.insert(
            0,
            Relation {
                catalog,
                partition: Partition::default(),
            },
        );
        control.cancel();
        assert_eq!(catalogs.publish(receipt, &mut work), Err(Stop::Cancelled));
        assert_eq!(
            catalogs.len(),
            catalogs
                .relations
                .iter()
                .map(|relation| relation.catalog.atoms().len())
                .sum::<usize>()
        );
    }
}
