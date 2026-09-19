//! Ordered tuple access over borrowed source snapshots or retained catalogs.
//!
//! Catalog calls receive the remaining work quota. Control is observed before
//! and after each whole catalog operation, rather than within its comparisons,
//! reservations and index planning. A stopped operation never returns a closure.

use std::{collections::BTreeSet, mem::size_of};

mod partition;
use partition::Partition;

mod dense;
pub(in crate::oracle) use dense::{Block, Dense, Layout, Layouts, PendingRows};

pub(super) mod storage;
pub(super) use storage::atom_bytes;

use zetesis_core::{
    Atom, AtomKey, Model, Predicate, Value,
    relation::{Catalog, CatalogFailure, Failure, Insertion, Limits, Resource, Storage},
};

use super::Work;
use crate::Stop;

/// A view is one or more runs, each in complete tuple storage order. A row is
/// addressed by its run and its position within the run; positions are access
/// positions, not persistent equality IDs or global source occurrence IDs. A
/// borrowed snapshot is one run; a catalog view is its levels and its tail,
/// any of which a selection may leave out; a dense view is one run of bit
/// positions, of which only the set ones are rows.
#[derive(Clone, Copy)]
pub(super) enum Rows<'a> {
    Borrowed(&'a [&'a Atom]),
    Runs {
        atoms: &'a [Atom],
        levels: &'a [Vec<usize>],
        tail: &'a [usize],
    },
    Dense {
        relation: &'a Dense,
        set: RowSet,
    },
}

/// One row of a view: an atom the view borrows, or a position in a dense
/// relation, whose values are read from the relation's layout.
#[derive(Clone, Copy)]
pub(super) enum Row<'a> {
    Atom(&'a Atom),
    Dense { layout: &'a Layout, position: usize },
}

impl<'a> Row<'a> {
    /// The row's values in argument order.
    pub(super) fn values(self) -> impl Iterator<Item = &'a Value> {
        let (atom, dense) = match self {
            Self::Atom(atom) => (Some(atom.values().iter()), None),
            Self::Dense { layout, position } => (
                None,
                Some(
                    (0..layout.predicate().arity())
                        .map(move |argument| layout.value(argument, position)),
                ),
            ),
        };
        atom.into_iter()
            .flatten()
            .chain(dense.into_iter().flatten())
    }

    /// The borrowed atom of a tree row; a dense row has none.
    #[cfg(test)]
    pub(super) fn atom(self) -> Option<&'a Atom> {
        match self {
            Self::Atom(atom) => Some(atom),
            Self::Dense { .. } => None,
        }
    }
}

impl<'a> Rows<'a> {
    /// Number of runs, counting an empty tail as none.
    pub(super) fn runs(self) -> usize {
        match self {
            Self::Borrowed(_) | Self::Dense { .. } => 1,
            Self::Runs { levels, tail, .. } => levels.len() + usize::from(!tail.is_empty()),
        }
    }

    /// The positions of a run; every one is a row except in a dense view,
    /// where a window yields only the set positions.
    pub(super) fn run_len(self, run: usize) -> usize {
        match self {
            Self::Borrowed(rows) => rows.len(),
            Self::Runs { levels, tail, .. } => levels.get(run).map_or(tail.len(), Vec::len),
            Self::Dense { relation, .. } => relation.layout().positions(),
        }
    }

    /// Every row of every run, run by run. Runs are each in canonical order;
    /// the sequence across runs is not.
    #[cfg(test)]
    pub(super) fn all(self) -> Vec<Row<'a>> {
        (0..self.runs())
            .flat_map(|run| (0..self.run_len(run)).map(move |position| (run, position)))
            .filter(|&(_, position)| match self {
                Self::Dense { relation, set } => relation.holds(set, position),
                Self::Borrowed(_) | Self::Runs { .. } => true,
            })
            .map(|(run, position)| self.get(run, position).expect("in range"))
            .collect()
    }

    /// The row at a position a window yielded.
    pub(super) fn get(self, run: usize, position: usize) -> Option<Row<'a>> {
        match self {
            Self::Borrowed(rows) => rows.get(position).copied().map(Row::Atom),
            Self::Runs {
                atoms,
                levels,
                tail,
            } => levels
                .get(run)
                .map_or(tail, Vec::as_slice)
                .get(position)
                .and_then(|&id| atoms.get(id))
                .map(Row::Atom),
            Self::Dense { relation, .. } => {
                (position < relation.layout().positions()).then_some(Row::Dense {
                    layout: relation.layout(),
                    position,
                })
            }
        }
    }

    /// The next row's position in a run at or after the range's start,
    /// advancing the range past it; `None` when the range holds no row.
    pub(super) fn next_row(
        self,
        range: &mut std::ops::Range<usize>,
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, Stop> {
        match self {
            Self::Borrowed(_) | Self::Runs { .. } => Ok(range.next()),
            Self::Dense { relation, set } => relation.next_row(set, range, work),
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

/// One predicate's rows: a typed catalog with its round partition, or a
/// dense bit array when the predicate's arguments are bounded. The catalog
/// is boxed so that the two arms are of a size.
pub(super) enum Relation {
    Tree {
        catalog: Box<Catalog>,
        partition: Partition,
    },
    Dense(Dense),
}

impl Relation {
    fn predicate(&self) -> &Predicate {
        match self {
            Self::Tree { catalog, .. } => catalog.predicate(),
            Self::Dense(dense) => dense.predicate(),
        }
    }

    #[cfg(test)]
    pub(super) fn catalog(&self) -> &Catalog {
        match self {
            Self::Tree { catalog, .. } => catalog,
            Self::Dense(_) => panic!("a dense relation has no catalog"),
        }
    }

    #[cfg(test)]
    pub(super) fn partition(&self) -> &Partition {
        match self {
            Self::Tree { partition, .. } => partition,
            Self::Dense(_) => panic!("a dense relation has no partition"),
        }
    }

    #[cfg(test)]
    pub(super) fn is_dense(&self) -> bool {
        matches!(self, Self::Dense(_))
    }
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

/// Charge relation work, which counts as catalog work.
pub(super) fn charge(work: &mut Work<'_>, amount: usize) -> Result<(), Stop> {
    let before = work.statistics.work;
    let result = work.charge(amount);
    let charged = work.statistics.work - before;
    work.statistics.catalog_work = work
        .statistics
        .catalog_work
        .checked_add(charged)
        .ok_or(Stop::InvalidProgram)?;
    result
}

#[cfg(test)]
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

/// The scalar closure's relations, of two kinds: a tree relation owns each
/// of its atoms once until final interpretation assembly, and a dense
/// relation owns no atom, holding its tuples as bits until the assembly
/// builds them. Views borrow the retained row index; rounds do not rebuild
/// tuple snapshots. Relations are kept in predicate order, so the final
/// model is their concatenation and a handle is a position in this vector.
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
        match self.relations.get(handle).ok_or(Stop::InvalidProgram)? {
            Relation::Tree { catalog, partition } => partition.rows(catalog, set),
            Relation::Dense(relation) => Ok(Rows::Dense { relation, set }),
        }
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
            .binary_search_by(|relation| relation.predicate().cmp(predicate))
    }

    pub(super) fn prepare(&mut self, work: &mut Work<'_>) -> Result<(), Stop> {
        storage::admit(work, self.bytes)?;
        storage::record(work, self.bytes)?;
        for relation in &mut self.relations {
            let Relation::Tree { catalog, .. } = relation else {
                continue;
            };
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
            if let Relation::Tree { catalog, partition } = relation {
                partition.confirm(catalog, work)?;
            }
        }
        Ok(())
    }

    /// Advance old truth only after every source partition has completed, and
    /// before appending the new heads. No borrowed round view is still live.
    pub(super) fn advance(&mut self, work: &mut Work<'_>) -> Result<(), Stop> {
        for relation in &mut self.relations {
            match relation {
                Relation::Tree { catalog, partition } => {
                    partition.advance(catalog.atoms().len(), work)?;
                }
                Relation::Dense(dense) => dense.advance(work)?,
            }
        }
        Ok(())
    }

    fn relation_has_new(relation: &Relation) -> bool {
        match relation {
            Relation::Tree { catalog, partition } => partition.has_new(catalog.atoms().len()),
            Relation::Dense(dense) => dense.has_new(),
        }
    }

    /// Predicates whose relation gained rows since the cutoff advanced, in
    /// canonical predicate order.
    pub(super) fn predicates_with_new(&self) -> impl Iterator<Item = &Predicate> {
        self.relations
            .iter()
            .filter(|relation| Self::relation_has_new(relation))
            .map(Relation::predicate)
    }

    pub(super) fn has_new(&self, predicate: &Predicate, work: &mut Work<'_>) -> Result<bool, Stop> {
        work.tick()?;
        Ok(self
            .find(predicate)
            .ok()
            .is_some_and(|handle| Self::relation_has_new(&self.relations[handle])))
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
        let catalog = match &self.relations[handle] {
            Relation::Tree { catalog, .. } => catalog,
            // A dense relation is asked through its layout by the round,
            // never through the catalogs.
            Relation::Dense(_) => return Err(Stop::InvalidProgram),
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

    /// Insert a derived atom of a tree relation, creating the relation on
    /// first use. A dense relation takes its rows from the round's pending
    /// rows, never as atoms, so an atom of one is an invariant violation.
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
        let Relation::Tree { catalog, .. } = &mut self.relations[handle] else {
            return Err(Stop::InvalidProgram);
        };
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
            .and_then(|bytes| bytes.checked_add(size_of::<Relation>() as u128))
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
            .checked_add(size_of::<Relation>() as u128)
            .and_then(|bytes| bytes.checked_add(names))
            .and_then(|bytes| bytes.checked_add(catalog.retained_bytes() as u128))
            .ok_or(Stop::StorageLimit)?;
        self.relations
            .try_reserve(1)
            .map_err(|_| Stop::Allocation)?;
        work.charge(self.relations.len() - position)?;
        self.relations.insert(
            position,
            Relation::Tree {
                catalog: Box::new(catalog),
                partition: Partition::default(),
            },
        );
        Ok(())
    }

    /// Create the dense relation of every layout that lacks one, so that a
    /// round can test and mark a dense head without changing the catalogs.
    /// A reused workspace already holds them and pays one search each.
    pub(super) fn create_dense_relations(
        &mut self,
        layouts: &Layouts,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        for layout in layouts.iter() {
            work.tick()?;
            if let Err(position) = self.find(layout.predicate()) {
                self.create_dense(position, layout, 0, work)?;
            }
        }
        Ok(())
    }

    /// The dense relation of a predicate, if it has one.
    pub(super) fn dense(&self, predicate: &Predicate) -> Option<&Dense> {
        match &self.relations[self.find(predicate).ok()?] {
            Relation::Dense(dense) => Some(dense),
            Relation::Tree { .. } => None,
        }
    }

    /// Insert a round's pending rows into their relations as new rows,
    /// leaving the pending rows empty. Called where the round's atoms are
    /// inserted: after the cutoff advanced, with no borrowed view live.
    pub(super) fn absorb(
        &mut self,
        pending: &mut PendingRows,
        layouts: &Layouts,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        if pending.is_empty() {
            return Ok(());
        }
        for (slot, layout) in layouts.iter().enumerate() {
            work.control.poll()?;
            let handle = self
                .find(layout.predicate())
                .map_err(|_| Stop::InvalidProgram)?;
            let Relation::Dense(dense) = &mut self.relations[handle] else {
                return Err(Stop::InvalidProgram);
            };
            self.atoms += pending.absorb_into(slot, dense, work)?;
        }
        Ok(())
    }

    /// Create a dense relation at its sorted position. Its variable storage
    /// is the two word vectors; the layout is shared with the preparation
    /// that chose it and counted there.
    fn create_dense(
        &mut self,
        position: usize,
        layout: &std::sync::Arc<Layout>,
        held: u128,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        let added = (size_of::<Relation>() as u128)
            .checked_add(Dense::word_bytes(layout))
            .ok_or(Stop::StorageLimit)?;
        let total = self
            .bytes
            .checked_add(held)
            .and_then(|bytes| bytes.checked_add(added))
            .ok_or(Stop::StorageLimit)?;
        storage::admit(work, total)?;
        let dense = Dense::new(layout.clone())?;
        storage::after_reservation(work, total)?;
        self.bytes = self.bytes.checked_add(added).ok_or(Stop::StorageLimit)?;
        self.relations
            .try_reserve(1)
            .map_err(|_| Stop::Allocation)?;
        work.charge(self.relations.len() - position)?;
        self.relations.insert(position, Relation::Dense(dense));
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
            let (catalog, partition) = match relation {
                Relation::Tree { catalog, partition } => (catalog, partition),
                Relation::Dense(dense) => {
                    work.tick()?;
                    dense.take_atoms(&mut atoms, self.bytes, work)?;
                    continue;
                }
            };
            partition.reset(work)?;
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
        let all: Vec<&Atom> = rows
            .all()
            .into_iter()
            .map(|row| row.atom().unwrap())
            .collect();
        assert_eq!(all, expected.iter().collect::<Vec<_>>());
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
        let original = &catalogs.relation(&predicate).catalog().atoms()[0];
        let row = catalogs.rows(&predicate).get(0, 0).unwrap();
        assert!(std::ptr::eq(original, row.atom().unwrap()));
    }

    /// The work of the dense-relation propositions: a generous ceiling.
    fn dense_work(control: &Control) -> Work<'_> {
        let mut work = Work::source(control, u64::MAX);
        work.limits.max_derived_atoms = 8;
        work.limits.max_closure_bytes = 1 << 20;
        work
    }

    /// `p` laid out over the values 1, 2 and 3, with the positions of 3 and
    /// 1 marked and absorbed: the predicate, the layouts, the catalogs and
    /// the pending rows.
    fn laid_out(work: &mut Work<'_>) -> (Predicate, Layouts, Catalogs, PendingRows) {
        use crate::oracle::bounds::Bound;
        let predicate = Predicate::new("p", 1).unwrap();
        let mut layouts = Layouts::default();
        layouts.push(
            Layout::new(
                &predicate,
                &[Bound::Finite(vec![
                    Value::Number(1),
                    Value::Number(2),
                    Value::Number(3),
                ])],
                64,
            )
            .unwrap(),
        );
        let mut catalogs = Catalogs::default();
        catalogs.create_dense_relations(&layouts, work).unwrap();
        let mut pending = PendingRows::default();
        pending.prepare(&layouts, &mut 0, work).unwrap();
        // Positions 2 and 0 are the values 3 and 1; a repeated mark is not a
        // second atom.
        assert!(pending.mark(0, 2));
        assert!(pending.mark(0, 0));
        assert!(!pending.mark(0, 2));
        catalogs.absorb(&mut pending, &layouts, work).unwrap();
        (predicate, layouts, catalogs, pending)
    }

    #[test]
    fn a_laid_out_predicate_is_a_dense_relation() {
        let control = Control::default();
        let mut work = dense_work(&control);
        let (predicate, _, catalogs, _) = laid_out(&mut work);
        assert!(catalogs.relation(&predicate).is_dense());
        assert_eq!(catalogs.len(), 2);
    }

    #[test]
    fn a_dense_relation_takes_no_atom() {
        let control = Control::default();
        let mut work = dense_work(&control);
        let (_, _, mut catalogs, _) = laid_out(&mut work);
        assert_eq!(
            catalogs.insert(atom(Value::Number(2)), 0, &mut work),
            Err(Stop::InvalidProgram)
        );
    }

    #[test]
    fn a_dense_relation_answers_membership_by_position() {
        let control = Control::default();
        let mut work = dense_work(&control);
        let (predicate, _, mut catalogs, _) = laid_out(&mut work);
        catalogs.prepare_delta(&mut work).unwrap();
        let Relation::Dense(dense) = catalogs.relation(&predicate) else {
            unreachable!("the predicate is laid out");
        };
        for (value, expected) in [(3, true), (2, false), (9, false)] {
            let value = Value::Number(value);
            let pattern = zetesis_core::AtomPattern::new(
                predicate.clone(),
                vec![zetesis_core::Term::Variable(0)],
            )
            .unwrap();
            let assignment = [Some(&value)];
            let key = pattern.key(&assignment[..]).unwrap();
            assert_eq!(
                dense
                    .position(&key)
                    .is_some_and(|position| dense.contains(position)),
                expected
            );
        }
    }

    #[test]
    fn the_catalogs_answer_membership_for_trees_alone() {
        // A dense relation asked through the catalogs is an invariant
        // violation: the round asks it through its layout.
        let control = Control::default();
        let mut work = dense_work(&control);
        let (predicate, _, mut catalogs, _) = laid_out(&mut work);
        catalogs.prepare_delta(&mut work).unwrap();
        let value = Value::Number(3);
        let pattern = zetesis_core::AtomPattern::new(
            predicate.clone(),
            vec![zetesis_core::Term::Variable(0)],
        )
        .unwrap();
        let assignment = [Some(&value)];
        let key = pattern.key(&assignment[..]).unwrap();
        assert_eq!(
            catalogs.contains(&key, 0, &mut work),
            Err(Stop::InvalidProgram)
        );
    }

    #[test]
    fn a_dense_relations_rows_are_its_positions_new_until_advanced() {
        let control = Control::default();
        let mut work = dense_work(&control);
        let (predicate, _, mut catalogs, _) = laid_out(&mut work);
        catalogs.prepare_delta(&mut work).unwrap();
        let positions: Vec<usize> = catalogs
            .rows(&predicate)
            .all()
            .into_iter()
            .map(|row| match row {
                Row::Dense { position, .. } => position,
                Row::Atom(_) => unreachable!(),
            })
            .collect();
        assert_eq!(positions, vec![0, 2]);
        assert!(catalogs.has_new(&predicate, &mut work).unwrap());
        catalogs.advance(&mut work).unwrap();
        assert!(!catalogs.has_new(&predicate, &mut work).unwrap());
    }

    #[test]
    fn the_model_of_a_dense_relation_is_its_atoms_in_order() {
        let control = Control::default();
        let mut work = dense_work(&control);
        let (_, _, mut catalogs, _) = laid_out(&mut work);
        catalogs.prepare_delta(&mut work).unwrap();
        catalogs.advance(&mut work).unwrap();
        let model = catalogs.take_model(&mut work).unwrap();
        assert_eq!(
            model,
            Model::new([atom(Value::Number(1)), atom(Value::Number(3))])
        );
    }

    #[test]
    fn an_emptied_dense_relation_is_reused_as_a_dense_one() {
        let control = Control::default();
        let mut work = dense_work(&control);
        let (predicate, layouts, mut catalogs, mut pending) = laid_out(&mut work);
        catalogs.prepare_delta(&mut work).unwrap();
        catalogs.advance(&mut work).unwrap();
        catalogs.take_model(&mut work).unwrap();
        assert!(pending.mark(0, 1));
        catalogs.absorb(&mut pending, &layouts, &mut work).unwrap();
        assert!(catalogs.relation(&predicate).is_dense());
        assert_eq!(catalogs.len(), 1);
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
            Relation::Tree {
                catalog: Box::new(catalog),
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
                .map(|relation| relation.catalog().atoms().len())
                .sum::<usize>()
        );
    }
}
