//! Ordered tuple access over borrowed source snapshots or retained catalogs.
//!
//! Catalog calls receive the remaining work quota. Cancellation is observed before
//! and after each whole catalog operation, rather than within its comparisons,
//! reservations and index planning. A stopped operation never returns a closure.

use std::mem::size_of;

mod partition;
use partition::Partition;

mod dense;
pub(in crate::oracle) use dense::{Block, Dense, Layout, Layouts, PendingMarks};

pub(super) mod storage;
mod pending;
#[cfg(test)]
pub(super) mod fixtures;
pub(super) use pending::Pending;

use zetesis_core::{
    AtomKey, Model, Program,
    atom_interner::{AtomAppender, AtomInterner},
    catalog::{AtomRef, Atoms, CatalogRead, DeclaredPredicate, PredicateRef, TermRef},
    relation::{Catalog, CatalogFailure, Failure, Insertion, Limits, Resource, Storage},
};

use super::Work;
use crate::Stop;
#[cfg(test)]
use zetesis_core::{Atom, Predicate};

/// A view is one or more runs, each in complete tuple storage order. A row is
/// addressed by its run and its position within the run; positions are access
/// positions, not persistent equality IDs or global source occurrence IDs. A
/// borrowed snapshot is one run; a catalog view is its levels and its tail,
/// any of which a selection may leave out; a dense view is one run of bit
/// positions, of which only the set ones are rows.
#[derive(Clone, Copy)]
pub(super) enum Rows<'a> {
    Borrowed(&'a [AtomRef<'a>]),
    Runs {
        atoms: Atoms<'a>,
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
    Atom(AtomRef<'a>),
    Dense { layout: &'a Layout, position: usize },
}

impl<'a> Row<'a> {
    /// The row's values in argument order.
    pub(super) fn values(self) -> impl Iterator<Item = TermRef<'a>> {
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
    pub(super) fn atom(self) -> Option<AtomRef<'a>> {
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
                .and_then(|&id| atoms.at(id))
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
    fn resolve(
        &self,
        predicate: PredicateRef<'_>,
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, Stop>;

    /// The rows of a resolved relation in the requested set.
    fn rows_at(&self, handle: usize, set: RowSet) -> Result<Rows<'_>, Stop>;

    #[cfg(test)]
    fn rows(&self, predicate: &Predicate) -> Rows<'_> {
        let cancellation = crate::Cancellation::default();
        let mut work = Work::source(&cancellation, u64::MAX);
        self.resolve(predicate.into(), &mut work)
            .unwrap()
            .and_then(|handle| self.rows_at(handle, RowSet::Current).ok())
            .unwrap_or(Rows::Borrowed(&[]))
    }

    #[cfg(test)]
    fn selected(&self, predicate: &Predicate, set: RowSet) -> Result<Rows<'_>, Stop> {
        let cancellation = crate::Cancellation::default();
        let mut work = Work::source(&cancellation, u64::MAX);
        match self.resolve(predicate.into(), &mut work)? {
            Some(handle) => self.rows_at(handle, set),
            None => Ok(Rows::Borrowed(&[])),
        }
    }
}

/// The resolution of one join depth's predicate to a relation handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Resolution {
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
    fn predicate<'a>(&'a self, read: CatalogRead<'a>) -> PredicateRef<'a> {
        match self {
            Self::Tree { catalog, .. } => catalog
                .predicate(read)
                .expect("reader covers retained relation"),
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
pub(super) struct Relations<'a>(Vec<(PredicateRef<'a>, Vec<AtomRef<'a>>)>);

impl<'a> Relations<'a> {
    pub(super) const fn new() -> Self {
        Self(Vec::new())
    }

    /// Named borrowed-row directory and row-buffer capacities. No canonical
    /// payload or shared Program is retained by these references.
    pub(super) fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + self.0.capacity() as u128 * size_of::<(PredicateRef<'_>, Vec<AtomRef<'_>>)>() as u128
            + self
                .0
                .iter()
                .map(|(_, rows)| rows.capacity() as u128 * size_of::<AtomRef<'_>>() as u128)
                .sum::<u128>()
    }

    /// Append one already ordered row, admitting metadata growth while its old
    /// buffer remains live. `other_bytes` names live caller allocations.
    pub(super) fn push_with(
        &mut self,
        atom: AtomRef<'a>,
        other_bytes: u128,
        limit: usize,
        peak: &mut u128,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        work.tick()?;
        let predicate = atom.predicate();
        let same = match self.0.last() {
            Some((previous, _)) => previous.equals_ref_with(predicate, || work.tick())?,
            None => false,
        };
        if same {
            let live = self.retained_bytes();
            let rows = &mut self.0.last_mut().ok_or(Stop::InvalidProgram)?.1;
            reserve_rows(rows, other_bytes + live, limit, peak, work)?;
            work.tick()?;
            rows.push(atom);
        } else {
            let live = self.retained_bytes();
            reserve_rows(&mut self.0, other_bytes + live, limit, peak, work)?;
            let mut rows = Vec::new();
            reserve_rows(
                &mut rows,
                other_bytes + self.retained_bytes(),
                limit,
                peak,
                work,
            )?;
            work.tick()?;
            rows.push(atom);
            work.tick()?;
            self.0.push((predicate, rows));
        }
        Ok(())
    }
}

fn reserve_rows<T>(
    rows: &mut Vec<T>,
    live: u128,
    limit: usize,
    peak: &mut u128,
    work: &mut Work<'_>,
) -> Result<(), Stop> {
    if rows.len() < rows.capacity() {
        return Ok(());
    }
    let target = rows
        .len()
        .checked_add(1)
        .ok_or(Stop::Allocation)?
        .max(rows.capacity().saturating_mul(2))
        .max(4);
    if live + target as u128 * size_of::<T>() as u128 > limit as u128 {
        return Err(Stop::StorageLimit);
    }
    *peak = (*peak).max(live + target as u128 * size_of::<T>() as u128);
    for _ in 0..rows.len().max(1) {
        work.tick()?;
    }
    rows.try_reserve_exact(target - rows.len())
        .map_err(|_| Stop::Allocation)?;
    let actual = live + rows.capacity() as u128 * size_of::<T>() as u128;
    *peak = (*peak).max(actual);
    if actual > limit as u128 {
        return Err(Stop::StorageLimit);
    }
    Ok(())
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
        let mut groups: Vec<_> = groups
            .into_iter()
            .map(|(predicate, atoms)| {
                (
                    PredicateRef::from(predicate),
                    atoms.into_iter().map(AtomRef::from).collect(),
                )
            })
            .collect();
        groups.sort_by(|left, right| left.0.cmp(&right.0));
        Self(groups)
    }
}

impl Relational for Relations<'_> {
    fn resolve(
        &self,
        predicate: PredicateRef<'_>,
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, Stop> {
        let (mut low, mut high) = (0, self.0.len());
        while low < high {
            let middle = low + (high - low) / 2;
            work.tick()?;
            match self.0[middle]
                .0
                .compare_ref_with(predicate, || work.tick())?
            {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return Ok(Some(middle)),
            }
        }
        Ok(None)
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

/// One Program's retained canonical authority and independently reset truth.
/// Payload survives candidate completion under the named storage ceiling;
/// relation rows, partitions and dense bits do not.
#[derive(Default)]
pub(super) struct Catalogs {
    authority: Option<AtomInterner>,
    program: Option<Program>,
    declarations: Vec<DeclaredPredicate>,
    relations: Vec<Relation>,
    atoms: usize,
    overhead: u128,
}

/// An immutable committed round, disjoint from its canonical append capability.
/// Every retained tree member was covered when the round was prepared.
pub(super) struct RoundRead<'a> {
    read: CatalogRead<'a>,
    relations: &'a [Relation],
    atoms: usize,
    base: u128,
}

impl Relational for RoundRead<'_> {
    fn resolve(
        &self,
        predicate: PredicateRef<'_>,
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, Stop> {
        Ok(self.find(predicate, work)?.ok())
    }
    fn rows_at(&self, handle: usize, set: RowSet) -> Result<Rows<'_>, Stop> {
        rows_at(self.relations, self.read, handle, set)
    }
}

fn rows_at<'a>(
    relations: &'a [Relation],
    read: CatalogRead<'a>,
    handle: usize,
    set: RowSet,
) -> Result<Rows<'a>, Stop> {
    match relations.get(handle).ok_or(Stop::InvalidProgram)? {
        Relation::Tree { catalog, partition } => partition.rows(catalog, read, set),
        Relation::Dense(relation) => Ok(Rows::Dense { relation, set }),
    }
}

fn find_relation(
    relations: &[Relation],
    read: CatalogRead<'_>,
    predicate: PredicateRef<'_>,
    work: &mut Work<'_>,
) -> Result<Result<usize, usize>, Stop> {
    let (mut left, mut right) = (0, relations.len());
    while left < right {
        charge(work, 1)?;
        let middle = left + (right - left) / 2;
        match relations[middle]
            .predicate(read)
            .compare_ref_with(predicate, || charge(work, 1))?
        {
            std::cmp::Ordering::Less => left = middle + 1,
            std::cmp::Ordering::Equal => return Ok(Ok(middle)),
            std::cmp::Ordering::Greater => right = middle,
        }
    }
    Ok(Err(left))
}

impl RoundRead<'_> {
    fn find(
        &self,
        predicate: PredicateRef<'_>,
        work: &mut Work<'_>,
    ) -> Result<Result<usize, usize>, Stop> {
        find_relation(self.relations, self.read, predicate, work)
    }
    pub(super) fn len(&self) -> usize {
        self.atoms
    }
    pub(super) fn base_bytes(&self) -> u128 {
        self.base
    }
    pub(super) fn dense(
        &self,
        predicate: PredicateRef<'_>,
        work: &mut Work<'_>,
    ) -> Result<Option<&Dense>, Stop> {
        let Ok(handle) = self.find(predicate, work)? else {
            return Ok(None);
        };
        Ok(match &self.relations[handle] {
            Relation::Dense(dense) => Some(dense),
            Relation::Tree { .. } => None,
        })
    }
    pub(super) fn predicates_with_new(&self) -> impl Iterator<Item = PredicateRef<'_>> {
        self.relations
            .iter()
            .filter(|relation| relation_has_new(relation))
            .map(|relation| relation.predicate(self.read))
    }
    pub(super) fn has_new(
        &self,
        predicate: PredicateRef<'_>,
        work: &mut Work<'_>,
    ) -> Result<bool, Stop> {
        work.tick()?;
        Ok(self
            .find(predicate, work)?
            .ok()
            .is_some_and(|handle| relation_has_new(&self.relations[handle])))
    }
    pub(super) fn contains(
        &self,
        key: &AtomKey<'_>,
        transient: u128,
        work: &mut Work<'_>,
    ) -> Result<bool, Stop> {
        work.cancellation.poll()?;
        let Ok(handle) = self.find(key.predicate(), work)? else {
            return Ok(false);
        };
        let Relation::Tree { catalog, .. } = &self.relations[handle] else {
            return Err(Stop::InvalidProgram);
        };
        let other = self
            .base
            .checked_add(transient)
            .and_then(|bytes| bytes.checked_sub(catalog.retained_bytes() as u128))
            .ok_or(Stop::StorageLimit)?;
        let lookup = completed(
            catalog.lookup_key(self.read, key, limits(work, other)?),
            other,
            work,
        )?;
        account_storage(work, other, lookup.storage)?;
        Ok(lookup.row.is_some())
    }
}

fn relation_has_new(relation: &Relation) -> bool {
    match relation {
        Relation::Tree { catalog, partition } => partition.has_new(catalog.len()),
        Relation::Dense(dense) => dense.has_new(),
    }
}

impl Relational for Catalogs {
    fn resolve(
        &self,
        predicate: PredicateRef<'_>,
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, Stop> {
        Ok(self.find(predicate, work)?.ok())
    }
    fn rows_at(&self, handle: usize, set: RowSet) -> Result<Rows<'_>, Stop> {
        let read = self.authority.as_ref().ok_or(Stop::InvalidProgram)?.read();
        rows_at(&self.relations, read, handle, set)
    }
}

impl Catalogs {
    /// Bind a workspace once. Reuse retains identity but starts each completed
    /// candidate with no truth; a different Program requires a new workspace.
    pub(super) fn bind_program(
        &mut self,
        program: &Program,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        if let Some(previous) = &self.program {
            if !previous.same_instance(program) {
                return Err(Stop::WrongProgram);
            }
            storage::admit(work, self.total_bytes())?;
            return storage::record(work, self.total_bytes());
        }
        let authority =
            AtomInterner::for_program(program, work.limits.max_closure_bytes).map_err(|error| {
                storage::atom_failure(zetesis_core::atom_interner::Failure::Catalog(error))
            })?;
        self.authority = Some(authority);
        self.program = Some(program.clone());
        let live = self.total_bytes();
        storage::reserve(
            &mut self.declarations,
            program.predicates().len(),
            live,
            work,
        )?;
        for predicate in program.predicates() {
            let other = self
                .metadata_bytes()
                .checked_add(self.overhead)
                .ok_or(Stop::StorageLimit)?;
            let allowance = storage::atom_limits(work, other)?;
            let authority = self.authority.as_mut().ok_or(Stop::InvalidProgram)?;
            authority.restart_storage_peak();
            let result = authority.declare_predicate_with(predicate, allowance, || charge(work, 1));
            storage::record(work, other + authority.storage_peak_bytes())?;
            self.declarations
                .push(result.map_err(storage::atom_failure)?);
        }
        self.commit(0, work)
    }

    #[cfg(test)]
    pub(super) const fn len(&self) -> usize {
        self.atoms
    }

    fn metadata_bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + self.declarations.capacity() as u128 * size_of::<DeclaredPredicate>() as u128
            + self.relations.capacity() as u128 * size_of::<Relation>() as u128
            + self
                .relations
                .iter()
                .map(|relation| match relation {
                    Relation::Tree { catalog, .. } => catalog.retained_bytes() as u128,
                    Relation::Dense(dense) => dense.retained_bytes(),
                })
                .sum::<u128>()
    }
    fn total_bytes(&self) -> u128 {
        self.owned_bytes() + self.overhead
    }
    pub(super) fn owned_bytes(&self) -> u128 {
        self.metadata_bytes()
            + self
                .authority
                .as_ref()
                .map_or(0, AtomInterner::storage_bytes)
    }
    pub(super) fn set_overhead(&mut self, bytes: u128, work: &mut Work<'_>) -> Result<(), Stop> {
        let total = self
            .owned_bytes()
            .checked_add(bytes)
            .ok_or(Stop::StorageLimit)?;
        storage::admit(work, total)?;
        storage::record(work, total)?;
        self.overhead = bytes;
        Ok(())
    }

    fn find(
        &self,
        predicate: PredicateRef<'_>,
        work: &mut Work<'_>,
    ) -> Result<Result<usize, usize>, Stop> {
        let Some(authority) = &self.authority else {
            return Ok(Err(0));
        };
        find_relation(&self.relations, authority.read(), predicate, work)
    }

    #[cfg(test)]
    pub(super) fn relation(&self, predicate: &Predicate) -> &Relation {
        let cancellation = crate::Cancellation::default();
        let mut work = Work::source(&cancellation, u64::MAX);
        &self.relations[self
            .find(predicate.into(), &mut work)
            .unwrap()
            .expect("relation exists")]
    }

    pub(super) fn split(&mut self) -> Result<(RoundRead<'_>, AtomAppender<'_>), Stop> {
        let base = self
            .metadata_bytes()
            .checked_add(self.overhead)
            .ok_or(Stop::StorageLimit)?;
        let (committed, appender) = self.authority.as_mut().ok_or(Stop::InvalidProgram)?.split();
        Ok((
            RoundRead {
                read: committed.read(),
                relations: &self.relations,
                atoms: self.atoms,
                base,
            },
            appender,
        ))
    }

    pub(super) fn commit(&mut self, pending: u128, work: &mut Work<'_>) -> Result<(), Stop> {
        let other = self
            .metadata_bytes()
            .checked_add(self.overhead)
            .and_then(|bytes| bytes.checked_add(pending))
            .ok_or(Stop::StorageLimit)?;
        let allowance = storage::atom_limits(work, other)?;
        let authority = self.authority.as_mut().ok_or(Stop::InvalidProgram)?;
        authority.restart_storage_peak();
        let result = authority.commit_with(allowance, || charge(work, 1));
        storage::record(work, other + authority.storage_peak_bytes())?;
        result.map_err(storage::atom_failure)
    }

    pub(super) fn prepare(&mut self, work: &mut Work<'_>) -> Result<(), Stop> {
        storage::admit(work, self.total_bytes())?;
        storage::record(work, self.total_bytes())?;
        for index in 0..self.relations.len() {
            let total = self.total_bytes();
            let Relation::Tree { catalog, .. } = &mut self.relations[index] else {
                continue;
            };
            work.cancellation.poll()?;
            let other = total
                .checked_sub(catalog.retained_bytes() as u128)
                .ok_or(Stop::InvalidProgram)?;
            let read = self.authority.as_ref().ok_or(Stop::InvalidProgram)?.read();
            let result = catalog
                .prepare_ordered(read, limits(work, other)?)
                .map(|prepared| prepared.storage);
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
    pub(super) fn advance(&mut self, work: &mut Work<'_>) -> Result<(), Stop> {
        for relation in &mut self.relations {
            match relation {
                Relation::Tree { catalog, partition } => partition.advance(catalog.len(), work)?,
                Relation::Dense(dense) => dense.advance(work)?,
            }
        }
        Ok(())
    }

    /// Publish one canonical discovery into truth after the source round closed.
    pub(super) fn insert(
        &mut self,
        id: usize,
        pending: u128,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        work.cancellation.poll()?;
        let (found, signature) = {
            let atom = self
                .authority
                .as_ref()
                .and_then(|authority| authority.get(id))
                .ok_or(Stop::InvalidProgram)?;
            let signature = self
                .program
                .as_ref()
                .ok_or(Stop::InvalidProgram)?
                .predicates()
                .binary_search_with(atom.predicate(), || charge(work, 1))?
                .map_err(|_| Stop::InvalidProgram)?;
            (self.find(atom.predicate(), work)?, signature)
        };
        let handle = match found {
            Ok(handle) => handle,
            Err(position) => {
                self.create(position, signature, pending, work)?;
                position
            }
        };
        let total = self
            .total_bytes()
            .checked_add(pending)
            .ok_or(Stop::StorageLimit)?;
        let Relation::Tree { catalog, .. } = &mut self.relations[handle] else {
            return Err(Stop::InvalidProgram);
        };
        let other = total
            .checked_sub(catalog.retained_bytes() as u128)
            .ok_or(Stop::InvalidProgram)?;
        let atom = self
            .authority
            .as_ref()
            .and_then(|authority| authority.get(id))
            .ok_or(Stop::InvalidProgram)?;
        let result = catalog.insert(atom, limits(work, other)?);
        let insertion = completed(result, other, work)?;
        self.publish(insertion, other, work)
    }

    // Truth is already published in the relation. Keep the aggregate count in
    // step with it even when cancellation is observed while charging its receipt.
    fn publish(
        &mut self,
        insertion: Insertion,
        other: u128,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        if insertion.inserted {
            self.atoms += 1;
        }
        account_storage(work, other, insertion.storage)
    }

    fn create(
        &mut self,
        position: usize,
        signature: usize,
        pending: u128,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        let live = self
            .total_bytes()
            .checked_add(pending)
            .ok_or(Stop::StorageLimit)?;
        storage::reserve(&mut self.relations, 1, live, work)?;
        let other = self
            .total_bytes()
            .checked_add(pending)
            .ok_or(Stop::StorageLimit)?;
        let declaration = self
            .declarations
            .get(signature)
            .ok_or(Stop::InvalidProgram)?
            .clone();
        let read = self.authority.as_ref().ok_or(Stop::InvalidProgram)?.read();
        let catalog = completed(
            Catalog::new(read, declaration, limits(work, other)?),
            other,
            work,
        )?;
        account_storage(work, other, catalog.construction())?;
        charge(work, self.relations.len() - position + 1)?;
        self.relations.insert(
            position,
            Relation::Tree {
                catalog: Box::new(catalog),
                partition: Partition::default(),
            },
        );
        Ok(())
    }

    pub(super) fn create_dense_relations(
        &mut self,
        layouts: &Layouts,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        for layout in layouts.iter() {
            work.tick()?;
            if let Err(position) = self.find(layout.predicate(), work)? {
                self.create_dense(position, layout, work)?;
            }
        }
        Ok(())
    }
    fn create_dense(
        &mut self,
        position: usize,
        layout: &std::sync::Arc<Layout>,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        let live = self.total_bytes();
        storage::reserve(&mut self.relations, 1, live, work)?;
        let total = self
            .total_bytes()
            .checked_add(Dense::word_bytes(layout))
            .ok_or(Stop::StorageLimit)?;
        storage::admit(work, total)?;
        let dense = Dense::new(layout.clone())?;
        storage::after_reservation(work, self.total_bytes() + dense.retained_bytes())?;
        charge(work, self.relations.len() - position + 1)?;
        self.relations.insert(position, Relation::Dense(dense));
        Ok(())
    }
    pub(super) fn absorb(
        &mut self,
        pending: &mut PendingMarks,
        layouts: &Layouts,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        if pending.is_empty() {
            return Ok(());
        }
        for (slot, layout) in layouts.iter().enumerate() {
            work.cancellation.poll()?;
            let handle = self
                .find(layout.predicate(), work)?
                .map_err(|_| Stop::InvalidProgram)?;
            let Relation::Dense(dense) = &mut self.relations[handle] else {
                return Err(Stop::InvalidProgram);
            };
            self.atoms += pending.absorb_into(slot, dense, work)?;
        }
        Ok(())
    }

    /// Resolve current truth rows to semantic-order discovery positions. Dense
    /// rows may discover atom identities here; neither path publishes a model.
    fn selected_positions(&mut self, work: &mut Work<'_>) -> Result<Vec<usize>, Stop> {
        let mut positions = Vec::new();
        let live = self
            .total_bytes()
            .checked_add(size_of::<Vec<usize>>() as u128)
            .ok_or(Stop::StorageLimit)?;
        storage::reserve(&mut positions, self.atoms, live, work)?;
        for index in 0..self.relations.len() {
            let selection_bytes = size_of::<Vec<usize>>() as u128
                + positions.capacity() as u128 * size_of::<usize>() as u128;
            let total = self
                .total_bytes()
                .checked_add(selection_bytes)
                .ok_or(Stop::StorageLimit)?;
            match &self.relations[index] {
                Relation::Tree { catalog, .. } => {
                    let authority = self.authority.as_ref().ok_or(Stop::InvalidProgram)?;
                    let other = total
                        .checked_sub(catalog.retained_bytes() as u128)
                        .ok_or(Stop::InvalidProgram)?;
                    let canonical = completed(
                        catalog.canonical(authority.read(), limits(work, other)?),
                        other,
                        work,
                    )?;
                    account_storage(work, other, canonical.storage)?;
                    let ordered_bytes = size_of::<Vec<usize>>() as u128
                        + canonical.ids.capacity() as u128 * size_of::<usize>() as u128;
                    let rows = catalog
                        .atoms(authority.read())
                        .map_err(|_| Stop::InvalidProgram)?;
                    let allowance = storage::atom_limits(
                        work,
                        self.metadata_bytes() + self.overhead + selection_bytes + ordered_bytes,
                    )?;
                    for row in canonical.ids {
                        let atom = rows.at(row).ok_or(Stop::InvalidProgram)?;
                        let id = authority
                            .find_atom_with(atom, allowance, || charge(work, 1))
                            .map_err(storage::atom_failure)?
                            .ok_or(Stop::InvalidProgram)?;
                        charge(work, 1)?;
                        positions.push(id);
                    }
                }
                Relation::Dense(dense) => {
                    let layout = dense.layout();
                    let mut coordinates = Vec::new();
                    storage::reserve(
                        &mut coordinates,
                        layout.predicate().arity(),
                        total + size_of::<Vec<usize>>() as u128,
                        work,
                    )?;
                    let coordinate_bytes = size_of::<Vec<usize>>() as u128
                        + coordinates.capacity() as u128 * size_of::<usize>() as u128;
                    let base =
                        self.metadata_bytes() + self.overhead + selection_bytes + coordinate_bytes;
                    let mut range = 0..layout.positions();
                    while let Some(position) = dense.next_row(RowSet::Current, &mut range, work)? {
                        coordinates.clear();
                        charge(work, layout.predicate().arity())?;
                        coordinates.extend(
                            (0..layout.predicate().arity())
                                .map(|argument| layout.coordinate(argument, position)),
                        );
                        let authority = self.authority.as_mut().ok_or(Stop::InvalidProgram)?;
                        let available = available(work, base + authority.storage_bytes())?;
                        let atom = layout
                            .program()
                            .carrier_atom_with(layout.signature(), &coordinates, available, || {
                                charge(work, 1)
                            })
                            .map_err(|error| carrier_failure(&error))?;
                        let other = base + atom.coordinate_bytes();
                        let allowance = storage::atom_limits(work, other)?;
                        authority.restart_storage_peak();
                        let result = authority
                            .entry_atom_with(atom.atom(), allowance, || charge(work, 1))
                            .and_then(|entry| entry.insert_with(allowance, || charge(work, 1)));
                        storage::record(work, other + authority.storage_peak_bytes())?;
                        let id = result.map_err(storage::atom_failure)?;
                        charge(work, 1)?;
                        positions.push(id);
                    }
                }
            }
        }
        Ok(positions)
    }

    /// Select the final prepared truth in semantic order and publish one shared
    /// prefix. The complete no-delta round has prepared every tree extent.
    /// Only ID/coordinate metadata is constructed; existing terms are not copied.
    pub(super) fn take_model(&mut self, work: &mut Work<'_>) -> Result<Model, Stop> {
        let positions = self.selected_positions(work)?;
        let selection_bytes = size_of::<Vec<usize>>() as u128
            + positions.capacity() as u128 * size_of::<usize>() as u128;
        let other = self.metadata_bytes() + self.overhead + selection_bytes;
        let allowance = storage::atom_limits(work, other)?;
        let authority = self.authority.as_mut().ok_or(Stop::InvalidProgram)?;
        authority.restart_storage_peak();
        let result = authority.publish_selection_with(&positions, allowance, || charge(work, 1));
        storage::record(work, other + authority.storage_peak_bytes())?;
        let catalog = result.map_err(storage::atom_failure)?;
        let publication = catalog.publication_bytes();
        let current = other + authority.storage_bytes() + publication;
        let result =
            Model::from_ordered_catalog_with(catalog, available(work, current)?, || work.tick());
        let model = result.map_err(|failure| match failure {
            zetesis_core::ModelFailure::Stopped(stop) => stop,
            zetesis_core::ModelFailure::Model(error) => Stop::model(&error),
        })?;
        storage::record(work, current + model.selection_bytes())?;
        drop(positions);
        let result_bytes = publication + model.selection_bytes();
        // Shared canonical storage is counted by the authority. Only the new
        // catalog/selection metadata remains alongside truth during reset.
        for index in 0..self.relations.len() {
            let total = self.total_bytes() + result_bytes;
            match &mut self.relations[index] {
                Relation::Tree { catalog, partition } => {
                    let other = total
                        .checked_sub(catalog.retained_bytes() as u128)
                        .ok_or(Stop::InvalidProgram)?;
                    let receipt = completed(catalog.clear(limits(work, other)?), other, work)?;
                    account_storage(work, other, receipt)?;
                    partition.reset(work)?;
                }
                Relation::Dense(dense) => dense.reset(work)?,
            }
        }
        self.atoms = 0;
        Ok(model)
    }
}

fn available(work: &Work<'_>, live: u128) -> Result<usize, Stop> {
    usize::try_from(
        (work.limits.max_closure_bytes as u128)
            .checked_sub(live)
            .ok_or(Stop::StorageLimit)?,
    )
    .map_err(|_| Stop::StorageLimit)
}

fn carrier_failure(failure: &zetesis_core::CarrierFailure<Stop>) -> Stop {
    match failure {
        zetesis_core::CarrierFailure::Stopped(stop) => *stop,
        zetesis_core::CarrierFailure::Storage(error) => match error {
            zetesis_core::CarrierError::Allocation => Stop::Allocation,
            zetesis_core::CarrierError::Bytes { .. } => Stop::StorageLimit,
            zetesis_core::CarrierError::Coordinates => Stop::InvalidProgram,
        },
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
    work.cancellation.poll()
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
    use super::fixtures::{catalogs as fixture, insert, intern};
    use super::*;
    use crate::Cancellation;
    use zetesis_core::{Atom, Value, ValueNodeRef};

    fn atom(value: Value) -> Atom {
        Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap()
    }

    #[test]
    fn retained_views_preserve_complete_tuple_order() {
        let predicate = Predicate::new("p", 1).unwrap();
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, u64::MAX);
        work.limits.max_derived_atoms = 3;
        let mut expected = [
            atom(Value::Symbol("a".into())),
            atom(Value::Number(2)),
            atom(Value::String("a".into())),
        ];
        let mut catalogs = fixture(&expected, &mut work);
        for atom in &expected {
            insert(&mut catalogs, atom, &mut work);
        }
        catalogs.prepare(&mut work).unwrap();
        expected.sort();
        // The first preparation is one run, in canonical order.
        let rows = catalogs.rows(&predicate);
        assert_eq!(rows.runs(), 1);
        let all: Vec<AtomRef<'_>> = rows
            .all()
            .into_iter()
            .map(|row| row.atom().unwrap())
            .collect();
        assert_eq!(all, expected.iter().map(AtomRef::from).collect::<Vec<_>>());
        assert!(rows.get(0, expected.len()).is_none());
    }

    #[test]
    fn ordered_rows_borrow_canonical_text() {
        let predicate = Predicate::new("p", 1).unwrap();
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, u64::MAX);
        work.limits.max_derived_atoms = 1;
        let source = atom(Value::String("payload".into()));
        let mut catalogs = fixture(std::slice::from_ref(&source), &mut work);
        insert(&mut catalogs, &source, &mut work);
        catalogs.prepare(&mut work).unwrap();
        let original = catalogs.authority.as_ref().unwrap().get(0).unwrap();
        let row = catalogs.rows(&predicate).get(0, 0).unwrap().atom().unwrap();
        let ValueNodeRef::String(original) = original.values().at(0).unwrap().descriptor() else {
            unreachable!()
        };
        let ValueNodeRef::String(actual) = row.values().at(0).unwrap().descriptor() else {
            unreachable!()
        };
        assert!(std::ptr::eq(original.as_ptr(), actual.as_ptr()));
        assert_eq!(original.len(), actual.len());
    }

    /// The work of the dense-relation propositions: a generous ceiling.
    fn dense_work(cancellation: &Cancellation) -> Work<'_> {
        let mut work = Work::source(cancellation, u64::MAX);
        work.limits.max_derived_atoms = 8;
        work.limits.max_closure_bytes = 1 << 20;
        work
    }

    /// `p` laid out over the values 1, 2 and 3, with the positions of 3 and
    /// 1 marked and absorbed: the predicate, the layouts, the catalogs and
    /// the pending marks.
    fn laid_out(work: &mut Work<'_>) -> (Predicate, Layouts, Catalogs, PendingMarks) {
        use crate::oracle::argument_bounds::Bound;
        let predicate = Predicate::new("p", 1).unwrap();
        let source = [1, 2, 3].map(|value| atom(Value::Number(value)));
        let program = fixtures::program(&source);
        let mut layouts = Layouts::default();
        layouts.push(
            Layout::new(
                &program,
                (&predicate).into(),
                &[Bound::Finite(vec![0, 1, 2])],
                64,
            )
            .unwrap()
            .unwrap(),
        );
        let mut catalogs = Catalogs::default();
        catalogs.bind_program(&program, work).unwrap();
        catalogs.create_dense_relations(&layouts, work).unwrap();
        let mut pending = PendingMarks::default();
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
        let cancellation = Cancellation::default();
        let mut work = dense_work(&cancellation);
        let (predicate, _, catalogs, _) = laid_out(&mut work);
        assert!(catalogs.relation(&predicate).is_dense());
        assert_eq!(catalogs.len(), 2);
    }

    #[test]
    fn a_dense_relation_takes_no_atom() {
        let cancellation = Cancellation::default();
        let mut work = dense_work(&cancellation);
        let (_, _, mut catalogs, _) = laid_out(&mut work);
        let id = intern(&mut catalogs, &atom(Value::Number(2)), &mut work);
        assert_eq!(catalogs.insert(id, 0, &mut work), Err(Stop::InvalidProgram));
    }

    #[test]
    fn a_dense_relation_answers_membership_by_position() {
        let cancellation = Cancellation::default();
        let mut work = dense_work(&cancellation);
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
                    .position(&key, &mut work)
                    .unwrap()
                    .is_some_and(|position| dense.contains(position)),
                expected
            );
        }
    }

    #[test]
    fn the_catalogs_answer_membership_for_trees_alone() {
        // A dense relation asked through the catalogs is an invariant
        // violation: the round asks it through its layout.
        let cancellation = Cancellation::default();
        let mut work = dense_work(&cancellation);
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
        let (round, appender) = catalogs.split().unwrap();
        assert_eq!(
            round.contains(&key, appender.storage_bytes(), &mut work),
            Err(Stop::InvalidProgram)
        );
    }

    #[test]
    fn a_dense_relations_rows_are_its_positions_new_until_advanced() {
        let cancellation = Cancellation::default();
        let mut work = dense_work(&cancellation);
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
        assert!(
            catalogs
                .split()
                .unwrap()
                .0
                .has_new((&predicate).into(), &mut work)
                .unwrap()
        );
        catalogs.advance(&mut work).unwrap();
        assert!(
            !catalogs
                .split()
                .unwrap()
                .0
                .has_new((&predicate).into(), &mut work)
                .unwrap()
        );
    }

    #[test]
    fn the_model_of_a_dense_relation_is_its_atoms_in_order() {
        let cancellation = Cancellation::default();
        let mut work = dense_work(&cancellation);
        let (_, _, mut catalogs, _) = laid_out(&mut work);
        catalogs.prepare_delta(&mut work).unwrap();
        catalogs.advance(&mut work).unwrap();
        let model = catalogs.take_model(&mut work).unwrap();
        assert_eq!(
            model,
            Model::new([atom(Value::Number(1)), atom(Value::Number(3))]).unwrap()
        );
    }

    #[test]
    fn an_emptied_dense_relation_is_reused_as_a_dense_one() {
        let cancellation = Cancellation::default();
        let mut work = dense_work(&cancellation);
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
    fn truth_reset_retains_discovery_identity_and_prior_models() {
        let cancellation = Cancellation::default();
        let mut work = dense_work(&cancellation);
        let source = [3, 1, 2].map(|value| atom(Value::Number(value)));
        let mut catalogs = fixture(&source, &mut work);
        insert(&mut catalogs, &source[0], &mut work);
        insert(&mut catalogs, &source[1], &mut work);
        catalogs.prepare(&mut work).unwrap();
        let first = catalogs.take_model(&mut work).unwrap();
        assert_eq!(catalogs.len(), 0);
        assert_eq!(catalogs.authority.as_ref().unwrap().len(), 2);
        // Reusing an earlier identity adds truth without a new discovery.
        insert(&mut catalogs, &source[1], &mut work);
        assert_eq!(catalogs.authority.as_ref().unwrap().len(), 2);
        insert(&mut catalogs, &source[2], &mut work);
        assert_eq!(catalogs.authority.as_ref().unwrap().len(), 3);
        catalogs.prepare(&mut work).unwrap();
        let second = catalogs.take_model(&mut work).unwrap();
        assert_eq!(
            first,
            Model::new([source[0].clone(), source[1].clone()]).unwrap()
        );
        assert_eq!(
            second,
            Model::new([source[1].clone(), source[2].clone()]).unwrap()
        );
    }

    #[test]
    fn cancelled_receipt_preserves_catalog_count() {
        let cancellation = Cancellation::default();
        let mut work = Work::source(&cancellation, u64::MAX);
        work.limits.max_derived_atoms = 1;
        let tuple = atom(Value::Number(1));
        let mut catalogs = fixture(std::slice::from_ref(&tuple), &mut work);
        let id = intern(&mut catalogs, &tuple, &mut work);
        catalogs.create(0, 0, 0, &mut work).unwrap();
        let total = catalogs.total_bytes();
        let Relation::Tree { catalog, .. } = &mut catalogs.relations[0] else {
            unreachable!()
        };
        let other = total - catalog.retained_bytes() as u128;
        let tuple = catalogs.authority.as_ref().unwrap().get(id).unwrap();
        let receipt = catalog
            .insert(tuple, limits(&work, other).unwrap())
            .unwrap();
        cancellation.cancel();
        assert_eq!(
            catalogs.publish(receipt, other, &mut work),
            Err(Stop::Cancelled)
        );
        assert_eq!(
            catalogs.len(),
            catalogs
                .relations
                .iter()
                .map(|relation| relation.catalog().len())
                .sum::<usize>()
        );
    }
}
