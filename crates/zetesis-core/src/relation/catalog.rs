//! Appendable extensional membership over one canonical atom authority.

use std::mem::size_of;

use crate::{
    AtomKey,
    catalog::{
        AtomRef, Atoms, CatalogRead, DeclaredPredicate, Member, Membership, PredicateRef, TermRef,
    },
};

mod plan;
mod appender;
pub use appender::{Appended, Appender};
mod ordered;
pub use ordered::{Canonical, Preparation, Runs};

use super::{
    Cell, DictionaryIndex, Failure, Layout, LayoutOwner, Limits, Relation, Resource, Source,
    Storage, Work, ceiling,
    column::OwnedColumn,
    dictionary::{self, Content},
};

/// One signed predicate's unique atom membership and appendable equality layout.
///
/// Logical payload stays in the canonical authority supplied by each checked
/// read. This catalog owns only membership IDs, row/equality indexes and ordered
/// runs. Dictionary entries are stable local source-cell positions. Rows and
/// equality IDs survive append; [`Self::clear`] invalidates them. Query identity
/// remains the particular immutable Relation object.
///
/// Byte limits cover the catalog object and its metadata capacities, including
/// operation scratch and replacement overlap. Canonical payload is separately
/// charged to its authority. Referenced encoding bytes count argument occurrences
/// and are neither unique storage nor RSS.
/// The canonical inverse counts addressable hash-entry capacity, excluding
/// opaque bucket/control allocation. Fixed-ID hash operations each admit one
/// container operation; their internal probes are not individually cancellable.
///
/// Row membership is found by the hash of the row's equality IDs: a tuple's
/// values resolve to their equality IDs first, and a tuple with a new value is
/// new without any row comparison. Otherwise only the rows chained under the
/// same hash are compared, one equality ID per column; no row order is kept on
/// insertion. Same-vocabulary dictionary lookup uses expected O(1) fixed-ID
/// hashing (O(d) worst case); a value the read cannot name is found by its
/// content hash. Historical row and equality IDs do not shift. Order exists
/// only in the prepared runs, which sort appended rows when they are prepared.
pub struct Catalog {
    membership: Membership,
    arity: usize,
    /// Rows chained under the hash of their equality IDs.
    rows: Content,
    ordered: ordered::Ordered,
    layout: Layout,
    encoding_bytes: u128,
    construction: Storage,
}

/// The outcome and operation-scoped accounting of one complete insertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Insertion {
    /// Stable local row ID, including when the tuple was already present.
    pub row: usize,
    /// Whether a new tuple was published.
    pub inserted: bool,
    /// Current capacity and work performed by this operation only.
    pub storage: Storage,
}

/// Exact membership result with the work needed to establish it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lookup {
    /// Original row ID when a complete typed tuple is present.
    pub row: Option<usize>,
    /// Current capacity and operation-scoped work.
    pub storage: Storage,
}

/// A catalog operation failed after the reported completed work.
/// Logical membership is unchanged; existing owners may retain reserved capacity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogFailure {
    /// Original shape, limit or allocation cause. A failed work charge retains
    /// its proposed count; `work` includes only work charged before that attempt.
    pub error: Failure,
    /// Completed charged work before failure.
    pub work: u128,
    /// Current retained owner capacity; zero when construction returns no owner.
    pub retained_bytes: usize,
    /// Recorded operation envelope, including scratch and growth overlap.
    /// A refused proposal that never allocates does not increase this receipt.
    pub peak_construction_bytes: usize,
}

impl std::fmt::Display for CatalogFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for CatalogFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl Catalog {
    /// Create an empty signed relation owner, including nullary relations.
    ///
    /// # Errors
    /// Refuses an incompatible read, shape, byte/work limits or allocation
    /// failure without a metadata owner.
    pub fn new(
        read: CatalogRead<'_>,
        predicate: DeclaredPredicate,
        limits: Limits,
    ) -> Result<Self, CatalogFailure> {
        Self::with_ids(read, predicate, limits, false)
    }

    /// Create an empty owner whose equality IDs are its vocabulary's own term
    /// identities. Values are never copied, renumbered or hashed again: an
    /// argument's ID is its interned identity and the read resolves it.
    /// Equal values still share an ID, absent values still have none, and a
    /// value the read cannot name is found by content as before. Only the
    /// numbering differs from [`Self::new`], so IDs are no longer dense.
    ///
    /// # Errors
    /// As [`Self::new`].
    pub fn interned(
        read: CatalogRead<'_>,
        predicate: DeclaredPredicate,
        limits: Limits,
    ) -> Result<Self, CatalogFailure> {
        Self::with_ids(read, predicate, limits, true)
    }

    fn with_ids(
        read: CatalogRead<'_>,
        predicate: DeclaredPredicate,
        limits: Limits,
        terms: bool,
    ) -> Result<Self, CatalogFailure> {
        let mut work =
            Work::new(limits, size_of::<Self>() as u128).map_err(|error| CatalogFailure {
                error,
                work: 0,
                retained_bytes: 0,
                peak_construction_bytes: 0,
            })?;
        let build = (|| {
            let membership = Membership::new(read, predicate).map_err(Failure::Read)?;
            let arity = membership.predicate(read).map_err(Failure::Read)?.arity();
            ceiling(Resource::Columns, arity as u128, limits.max_columns as u128)?;
            let mut columns = work.reserve(arity)?;
            for _ in 0..arity {
                work.tick(1)?;
                columns.push(OwnedColumn::new());
            }
            Ok(Self {
                membership,
                arity,
                rows: Content::default(),
                ordered: ordered::Ordered::default(),
                layout: Layout {
                    dictionary: Vec::new(),
                    index: DictionaryIndex::Append(super::dictionary::Append {
                        terms,
                        ..super::dictionary::Append::default()
                    }),
                    columns,
                },
                encoding_bytes: 0,
                construction: Storage {
                    retained_bytes: work.live,
                    peak_construction_bytes: work.peak,
                    referenced_encoding_bytes: 0,
                    borrowed_mapping_bytes: 0,
                    construction_work: work.charged(),
                },
            })
        })();
        build.map_err(|error| CatalogFailure {
            error,
            work: work.charged(),
            retained_bytes: 0,
            peak_construction_bytes: work.peak,
        })
    }

    /// Initial empty-owner construction receipt. It does not change on insertion.
    #[must_use]
    pub const fn construction(&self) -> Storage {
        self.construction
    }

    /// Number of unique local rows, independent of canonical discovery order.
    #[must_use]
    pub fn len(&self) -> usize {
        self.membership.ids.len()
    }

    /// Whether this predicate currently has no extensional members.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.membership.ids.is_empty()
    }

    /// Resolve the signed predicate through a compatible canonical prefix.
    ///
    /// # Errors
    /// Refuses a foreign authority or a prefix older than this membership.
    pub fn predicate<'a>(&self, read: CatalogRead<'a>) -> Result<PredicateRef<'a>, Failure> {
        self.membership.predicate(read).map_err(Failure::Read)
    }

    /// Borrow unique tuples in stable local insertion order. Prefix validation
    /// is constant time and neither imports payload nor copies a directory.
    ///
    /// # Errors
    /// Refuses a foreign authority or a prefix missing any retained member.
    pub fn atoms<'a>(&'a self, read: CatalogRead<'a>) -> Result<Atoms<'a>, Failure> {
        self.membership.bind(read).map_err(Failure::Read)
    }

    /// Begin an empty local extent while retaining reusable metadata capacity.
    /// Canonical identity remains in its authority; no truth is inherited.
    /// Rows and equality IDs are invalidated, and new inserts start at row zero.
    ///
    /// All checks precede the indivisible reset. Work covers column resets,
    /// fixed index bookkeeping, retained hash capacity, and releasing the
    /// previously retained run levels.
    ///
    /// # Errors
    /// Refuses shape/capacity or reset work without changing the published extent.
    pub fn clear(&mut self, limits: Limits) -> Result<Storage, CatalogFailure> {
        const RESET_BOOKKEEPING: u128 = 13;
        let mut work = self.work(limits)?;
        let DictionaryIndex::Append(index) = &self.layout.index else {
            unreachable!("catalog owns an append index");
        };
        work.tick(
            self.layout.columns.len() as u128
                + self.ordered.level_count() as u128
                + index.identities.clear_work()
                + index.content.clear_work()
                + self.rows.clear_work()
                + RESET_BOOKKEEPING,
        )
        .map_err(|error| self.failed(error, &work))?;
        let DictionaryIndex::Append(index) = &mut self.layout.index else {
            unreachable!("catalog owns an append index");
        };
        self.membership.clear();
        self.rows.clear();
        index.identities.clear();
        index.content.clear();
        self.layout.dictionary.clear();
        for column in &mut self.layout.columns {
            column.clear();
        }
        self.ordered.clear();
        self.encoding_bytes = 0;
        Ok(self.receipt(&work))
    }

    /// Borrow existing columns and canonical rows without rebuilding either.
    /// The view borrows metadata and the supplied read prefix. Its receipt
    /// includes only the view object; both owners remain separate caller charges.
    ///
    /// # Errors
    /// Refuses a foreign authority or a prefix older than this membership.
    pub fn view<'a>(&'a self, read: CatalogRead<'a>) -> Result<Relation<'a>, Failure> {
        let atoms = self.atoms(read)?;
        Ok(Relation {
            predicate: self.predicate(read)?,
            source: Source::Canonical(atoms),
            layout: LayoutOwner::Borrowed(&self.layout),
            storage: Storage {
                retained_bytes: size_of::<Relation<'_>>(),
                peak_construction_bytes: size_of::<Relation<'_>>(),
                referenced_encoding_bytes: self.encoding_bytes,
                borrowed_mapping_bytes: 0,
                construction_work: 0,
            },
        })
    }

    /// Check exact typed tuple membership without constructing another atom.
    ///
    /// # Errors
    /// Refuses incompatible reads, foreign predicates or work/byte ceilings. Comparisons include
    /// nested typed-value payload costs and do not rely on equality IDs alone.
    ///
    /// # Panics
    /// Panics if an admitted atom's argument extent disagrees with its predicate
    /// arity. Atom construction maintains this invariant.
    pub fn lookup(
        &self,
        read: CatalogRead<'_>,
        atom: AtomRef<'_>,
        limits: Limits,
    ) -> Result<Lookup, CatalogFailure> {
        let mut work = self.work(limits)?;
        work.tick(1).map_err(|error| self.failed(error, &work))?;
        self.locate(
            read,
            atom.predicate(),
            |column| atom.values().at(column).expect("checked atom arity"),
            &mut work,
        )
        .map(|row| Lookup {
            row,
            storage: self.receipt(&work),
        })
        .map_err(|error| self.failed(error, &work))
    }

    /// Look up a borrowed substitution with the same comparison and accounting
    /// as [`Self::lookup`]. No atom or value payload is copied.
    ///
    /// # Errors
    /// Refuses incompatible reads, foreign predicates or work/byte ceilings, preserving completed
    /// comparison work in the failure receipt.
    pub fn lookup_key(
        &self,
        read: CatalogRead<'_>,
        key: &AtomKey<'_>,
        limits: Limits,
    ) -> Result<Lookup, CatalogFailure> {
        let mut work = self.work(limits)?;
        work.tick(1).map_err(|error| self.failed(error, &work))?;
        self.locate(
            read,
            key.predicate(),
            |column| key.argument(column),
            &mut work,
        )
        .map(|row| Lookup {
            row,
            storage: self.receipt(&work),
        })
        .map_err(|error| self.failed(error, &work))
    }

    /// Insert an atom only after every required reservation and check succeeds.
    ///
    /// # Errors
    /// Refuses uninterned atoms, incompatible scopes/prefixes, predicate mismatch,
    /// inclusive limits, overflow or allocation
    /// failure. Logical contents and all published row/equality IDs remain
    /// unchanged on failure; successful reservations may retain spare capacity.
    /// Subsequent views and admissions include that actual retained capacity.
    pub fn insert(
        &mut self,
        atom: AtomRef<'_>,
        limits: Limits,
    ) -> Result<Insertion, CatalogFailure> {
        let mut work = self.work(limits)?;
        let mut plan = plan::Plan::default();
        self.insert_inner(atom, &mut plan, plan::Admission::Temporary, &mut work)
            .map_err(|error| self.failed(error, &work))
    }

    fn insert_inner(
        &mut self,
        atom: AtomRef<'_>,
        plan: &mut plan::Plan,
        admission: plan::Admission,
        work: &mut Work,
    ) -> Result<Insertion, Failure> {
        plan.clear();
        work.tick(1)?;
        let member = self.membership.member(atom).map_err(Failure::Read)?;
        let atom = member.atom();
        work.tick(1)?;
        let atoms = self.membership.bind(member.read()).map_err(Failure::Read)?;
        // An occupied row is found from its values' equality IDs before any
        // transaction scratch; a value without an equality ID makes it new.
        let DictionaryIndex::Append(index) = &self.layout.index else {
            return Err(Failure::Dictionary);
        };
        // A session insertion into an interned owner resolves the values'
        // term IDs once, for the plan; the row hash and any chain comparison
        // then read the planned IDs, and a value new to this owner makes the
        // row new with no lookup at all. Only a new row reads its encoding
        // bytes. The session's plan is already retained scratch, so planning
        // first admits no new bytes.
        if index.terms && matches!(admission, plan::Admission::Retained) {
            plan.clear();
            plan::identities(index, atoms, atom, plan, work)?;
            let hash = row_hash(&plan.ids, work)?;
            if plan.added.is_empty() {
                let columns = &self.layout.columns;
                let ids = &plan.ids;
                let found = self.rows.find(hash, |row| {
                    work.tick(1)?;
                    for (owned, &id) in columns.iter().zip(ids) {
                        work.tick(1)?;
                        if owned.view().get(row as usize) != Some(id) {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                })?;
                if let Some(row) = found {
                    return Ok(Insertion {
                        row: row as usize,
                        inserted: false,
                        storage: self.receipt(work),
                    });
                }
            }
            plan::encoding(atom, self.encoding_bytes, plan, work)?;
            return self.admit(member, plan, hash, work);
        }
        let read = atoms.read();
        let value = |column| atom.values().at(column).expect("checked atom arity");
        if let Some(row) = self.find_tuple(
            |column, work| {
                Ok(
                    match index
                        .identities
                        .probe_with(read, value(column), &mut || work.tick(1))?
                    {
                        dictionary::Probe::Local { equality, .. } => equality,
                        dictionary::Probe::Unavailable(_) => None,
                    },
                )
            },
            work,
        )? {
            return Ok(Insertion {
                row,
                inserted: false,
                storage: self.receipt(work),
            });
        }
        if matches!(admission, plan::Admission::Temporary) {
            work.include(size_of::<plan::Plan>())?;
        }
        plan::values(
            &mut self.layout,
            atoms,
            atom,
            self.encoding_bytes,
            plan,
            work,
        )?;
        let hash = row_hash(&plan.ids, work)?;
        self.admit(member, plan, hash, work)
    }

    fn admit(
        &mut self,
        member: Member<'_>,
        plan: &plan::Plan,
        hash: u64,
        work: &mut Work,
    ) -> Result<Insertion, Failure> {
        let row = self.len();
        ceiling(
            Resource::Rows,
            row as u128 + 1,
            (work.limits.max_rows as u128).min(u128::from(u32::MAX)),
        )?;
        self.reserve(plan, work)?;
        Ok(self.publish(member, plan, hash, work))
    }

    /// The row whose column `c` has the equality ID `resolve(c)`. The IDs
    /// are resolved once for the row hash and again for each row chained
    /// under it, so no ID buffer is allocated; a column without an ID is in
    /// no row.
    fn find_tuple(
        &self,
        mut resolve: impl FnMut(usize, &mut Work) -> Result<Option<u32>, Failure>,
        work: &mut Work,
    ) -> Result<Option<usize>, Failure> {
        let mut hash = row_seed(self.arity);
        for column in 0..self.arity {
            let Some(id) = resolve(column, work)? else {
                return Ok(None);
            };
            hash = crate::mix_word(hash, u64::from(id));
        }
        for _ in 0..=self.arity {
            work.tick(1)?;
        }
        let columns = &self.layout.columns;
        let found = self.rows.find(hash, |row| {
            work.tick(1)?;
            for (column, owned) in columns.iter().enumerate() {
                if resolve(column, work)? != owned.view().get(row as usize) {
                    return Ok(false);
                }
            }
            Ok(true)
        })?;
        Ok(found.map(|row| row as usize))
    }
}

impl Catalog {
    fn reserve(&mut self, plan: &plan::Plan, work: &mut Work) -> Result<(), Failure> {
        work.grow(&mut self.membership.ids, 1)?;
        self.rows.reserve(1, work)?;
        let DictionaryIndex::Append(index) = &mut self.layout.index else {
            return Err(Failure::Dictionary);
        };
        let terms = index.terms;
        if !terms {
            work.grow(&mut self.layout.dictionary, plan.added.len())?;
        }
        let DictionaryIndex::Append(index) = &mut self.layout.index else {
            return Err(Failure::Dictionary);
        };
        index.identities.reserve(plan.added.len(), work)?;
        if !terms {
            index.content.reserve(plan.added.len(), work)?;
        }
        for (column, id) in self.layout.columns.iter_mut().zip(plan.ids.iter().copied()) {
            column.reserve(id, work)?;
        }
        // Publication has no callbacks, allocations, payload comparisons or
        // failure. Include each fixed-ID hash insertion, the row's chain link
        // and each column write before beginning the indivisible writes. An
        // interned owner publishes one presence entry per new value.
        let per_value = if terms { 1 } else { 3 };
        work.tick(plan.ids.len() as u128 + plan.added.len() as u128 * per_value + 3)
    }

    fn publish(
        &mut self,
        member: Member<'_>,
        plan: &plan::Plan,
        hash: u64,
        work: &mut Work,
    ) -> Insertion {
        let row = self.len();
        let DictionaryIndex::Append(index) = &mut self.layout.index else {
            unreachable!("catalog owns an append index");
        };
        for added in &plan.added {
            if index.terms {
                index.identities.publish(added.term, added.term.word());
                continue;
            }
            let equality =
                u32::try_from(self.layout.dictionary.len()).expect("admitted equality ID");
            index.identities.publish(added.term, equality);
            index.content.publish(added.hash, equality);
            self.layout.dictionary.push(added.cell);
        }
        for (column, id) in self.layout.columns.iter_mut().zip(plan.ids.iter().copied()) {
            column.push(id);
        }
        self.rows
            .publish(hash, u32::try_from(row).expect("admitted row ID"));
        self.membership.publish(member);
        self.encoding_bytes = plan.encoding_bytes;
        Insertion {
            row,
            inserted: true,
            storage: self.receipt(work),
        }
    }

    fn locate<'value>(
        &self,
        read: CatalogRead<'_>,
        predicate: PredicateRef<'_>,
        value: impl Fn(usize) -> TermRef<'value>,
        work: &mut Work,
    ) -> Result<Option<usize>, Failure> {
        work.tick(1)?;
        let atoms = self.atoms(read)?;
        let expected = self.predicate(read)?;
        if !predicate.equals_ref_with(expected, || work.tick(1))? {
            return Err(Failure::Predicate);
        }
        let source = Source::Canonical(atoms);
        self.find_tuple(
            |column, work| super::storage::lookup(&self.layout, &source, value(column), work),
            work,
        )
    }

    fn work(&self, limits: Limits) -> Result<Work, CatalogFailure> {
        self.work_with(limits, 0)
    }

    fn work_with(&self, limits: Limits, scratch: usize) -> Result<Work, CatalogFailure> {
        let retained = self.retained_bytes();
        let bytes = retained as u128 + scratch as u128;
        let build = (|| {
            ceiling(Resource::Rows, self.len() as u128, limits.max_rows as u128)?;
            ceiling(
                Resource::Columns,
                self.arity as u128,
                limits.max_columns as u128,
            )?;
            ceiling(
                Resource::Values,
                self.layout.dictionary.len() as u128,
                limits.max_values as u128,
            )?;
            Work::new(limits, bytes)
        })();
        build.map_err(|error| CatalogFailure {
            error,
            work: 0,
            retained_bytes: retained,
            peak_construction_bytes: retained.saturating_add(scratch),
        })
    }

    fn failed(&self, error: Failure, work: &Work) -> CatalogFailure {
        CatalogFailure {
            error,
            work: work.charged(),
            retained_bytes: self.retained_bytes(),
            peak_construction_bytes: work.peak,
        }
    }

    /// Current owner capacity, including reservations retained after a refusal.
    /// Inspects one capacity per column; logical values and rows are not visited.
    #[must_use]
    pub fn retained_bytes(&self) -> usize {
        size_of::<Self>()
            + vector_bytes(&self.membership.ids)
            + self.rows.bytes()
            + self.ordered.capacity_bytes()
            + self.layout.dictionary.capacity() * size_of::<Cell>()
            + match &self.layout.index {
                DictionaryIndex::Sorted(ids) => ids.capacity() * size_of::<u32>(),
                DictionaryIndex::Append(index) => index.identities.bytes() + index.content.bytes(),
            }
            + self.layout.columns.capacity() * size_of::<OwnedColumn>()
            + self
                .layout
                .columns
                .iter()
                .map(OwnedColumn::retained_bytes)
                .sum::<usize>()
    }

    fn receipt(&self, work: &Work) -> Storage {
        Storage {
            retained_bytes: self.retained_bytes(),
            peak_construction_bytes: work.peak,
            referenced_encoding_bytes: self.encoding_bytes,
            borrowed_mapping_bytes: 0,
            construction_work: work.charged(),
        }
    }
}

fn vector_bytes<T>(values: &Vec<T>) -> usize {
    values.capacity() * size_of::<T>()
}

/// The hash of a row's equality IDs, charged one unit per column and one
/// for the hash itself.
fn row_hash(ids: &[u32], work: &mut Work) -> Result<u64, Failure> {
    for _ in 0..=ids.len() {
        work.tick(1)?;
    }
    Ok(ids.iter().fold(row_seed(ids.len()), |state, &id| {
        crate::mix_word(state, u64::from(id))
    }))
}

fn row_seed(arity: usize) -> u64 {
    crate::mix_word(0, arity as u64)
}

#[cfg(test)]
mod tests;
