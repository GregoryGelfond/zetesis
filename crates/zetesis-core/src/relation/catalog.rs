//! Appendable extensional membership over one canonical atom authority.

use std::mem::size_of;

use crate::{
    AtomKey,
    catalog::{
        AtomRef, Atoms, CatalogRead, DeclaredPredicate, Member, Membership, PredicateRef, TermRef,
    },
    ordered_index::{self, Directions, Index, Link, Node, Step},
};

mod plan;
mod ordered;
pub use ordered::{Canonical, Preparation, Runs};

use super::{
    Cell, DictionaryIndex, Failure, Layout, LayoutOwner, Limits, Relation, Resource, Source,
    Storage, Work, ceiling,
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
/// Row membership visits O(log n) nodes. Same-vocabulary dictionary lookup uses
/// expected O(1) fixed-ID hashing (O(d) worst case); foreign lookup and new-value
/// placement visit O(log d) semantic nodes, with typed descriptor/text-prefix
/// comparison work additional. Inserting a
/// tuple with `a` newly distinct values retains O(a log d) tentative node patches;
/// checked overlay scans can cost O(a² log² d) metadata work. Historical row and
/// equality IDs do not shift. Ordered runs preserve the existing incremental
/// preparation and separate allocation/work admission.
pub struct Catalog {
    membership: Membership,
    arity: usize,
    rows: Index,
    /// The last row in typed order: it changes only when a row arrives
    /// beyond it.
    last_row: Option<usize>,
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
                columns.push(Vec::new());
            }
            Ok(Self {
                membership,
                arity,
                rows: Index::default(),
                last_row: None,
                ordered: ordered::Ordered::default(),
                layout: Layout {
                    dictionary: Vec::new(),
                    index: DictionaryIndex::Append(super::dictionary::Append::default()),
                    columns,
                },
                encoding_bytes: 0,
                construction: Storage {
                    retained_bytes: work.live,
                    peak_construction_bytes: work.peak,
                    referenced_encoding_bytes: 0,
                    borrowed_mapping_bytes: 0,
                    construction_work: work.used,
                },
            })
        })();
        build.map_err(|error| CatalogFailure {
            error,
            work: work.used,
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
        const RESET_BOOKKEEPING: u128 = 12;
        let mut work = self.work(limits)?;
        let DictionaryIndex::Append(index) = &self.layout.index else {
            unreachable!("catalog owns an append index");
        };
        work.tick(
            self.layout.columns.len() as u128
                + self.ordered.level_count() as u128
                + index.identities.clear_work()
                + RESET_BOOKKEEPING,
        )
        .map_err(|error| self.failed(error, &work))?;
        let DictionaryIndex::Append(index) = &mut self.layout.index else {
            unreachable!("catalog owns an append index");
        };
        self.membership.clear();
        self.rows.nodes.clear();
        self.rows.path.clear();
        self.rows.root = None;
        self.last_row = None;
        index.order.nodes.clear();
        index.order.path.clear();
        index.order.root = None;
        index.identities.clear();
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
            |_| {},
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
            |_| {},
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
        self.insert_inner(atom, &mut work)
            .map_err(|error| self.failed(error, &work))
    }

    fn insert_inner(&mut self, atom: AtomRef<'_>, work: &mut Work) -> Result<Insertion, Failure> {
        work.tick(1)?;
        let member = self.membership.member(atom).map_err(Failure::Read)?;
        let atom = member.atom();
        let mut route = Directions::default();
        work.tick(1)?;
        let atoms = self.membership.bind(member.read()).map_err(Failure::Read)?;
        let value = |column| atom.values().at(column).expect("checked atom arity");
        // A row beyond the last in order is placed without a search.
        let last = ordered_index::last(
            &self.rows.nodes,
            self.rows.root,
            self.last_row,
            work,
            |work| work.tick(1),
            |row, work| compare_row(atoms, row, &value, work),
            |right| route.push(right).expect("AVL height fits two words"),
        )?;
        // An empty tree's first row is its last, as is a row beyond the last.
        let extends = matches!(last, ordered_index::Last::Beyond) || self.rows.root.is_none();
        let found = match last {
            ordered_index::Last::Found(row) => Some(row),
            ordered_index::Last::Beyond => None,
            ordered_index::Last::Search => self.locate_bound(atoms, value, work, |right| {
                route.push(right).expect("AVL height fits two words");
            })?,
        };
        if let Some(row) = found {
            return Ok(Insertion {
                row,
                inserted: false,
                storage: self.receipt(work),
            });
        }
        let row = self.len();
        ceiling(
            Resource::Rows,
            row as u128 + 1,
            work.limits.max_rows as u128,
        )?;
        let row_root = plan::row(&mut self.rows, row, &route, work)?;
        let plan = plan::values(&mut self.layout, atoms, atom, self.encoding_bytes, work)?;
        self.reserve(&plan, work)?;
        let insertion = self.publish(member, row_root, plan, work);
        if extends {
            self.last_row = Some(insertion.row);
        }
        Ok(insertion)
    }

    fn reserve(&mut self, plan: &plan::Plan, work: &mut Work) -> Result<(), Failure> {
        work.grow(&mut self.membership.ids, 1)?;
        work.grow(&mut self.rows.nodes, 1)?;
        work.grow(&mut self.layout.dictionary, plan.added.len())?;
        let DictionaryIndex::Append(index) = &mut self.layout.index else {
            return Err(Failure::Dictionary);
        };
        work.grow(&mut index.order.nodes, plan.added.len())?;
        index.identities.reserve(plan.added.len(), work)?;
        for column in &mut self.layout.columns {
            work.grow(column, 1)?;
        }
        // Publication has no callbacks, allocations, payload comparisons or
        // failure. Include each fixed-ID hash insertion, row-path inspection
        // and tentative patch write before beginning the indivisible writes.
        work.tick(
            self.rows.path.len() as u128
                + plan.ids.len() as u128
                + plan.added.len() as u128 * 3
                + plan.patches.len() as u128
                + 3,
        )?;
        Ok(())
    }

    fn publish(
        &mut self,
        member: Member<'_>,
        row_root: Link,
        plan: plan::Plan,
        work: &mut Work,
    ) -> Insertion {
        let row = self.len();
        let DictionaryIndex::Append(index) = &mut self.layout.index else {
            unreachable!("catalog owns an append index");
        };
        index
            .order
            .nodes
            .resize(index.order.nodes.len() + plan.added.len(), Node::default());
        for patch in &plan.patches {
            index.order.nodes[patch.id] = patch.node;
        }
        index.order.root = plan.root;
        for added in &plan.added {
            let equality =
                u32::try_from(self.layout.dictionary.len()).expect("admitted equality ID");
            index.identities.publish(added.term, equality);
            self.layout.dictionary.push(added.cell);
        }
        for (column, id) in self.layout.columns.iter_mut().zip(plan.ids.iter().copied()) {
            column.push(id);
        }
        self.rows.publish(row_root);
        self.membership.publish(member);
        self.encoding_bytes = plan.encoding_bytes;
        plan.release(work);
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
        descend: impl FnMut(bool),
    ) -> Result<Option<usize>, Failure> {
        work.tick(1)?;
        let atoms = self.atoms(read)?;
        let expected = self.predicate(read)?;
        if !predicate.equals_ref_with(expected, || work.tick(1))? {
            return Err(Failure::Predicate);
        }
        self.locate_bound(atoms, value, work, descend)
    }

    /// Search this catalog's bound membership after the caller has checked the
    /// query's signed predicate. Insertion establishes that through `member`;
    /// public lookups retain their reader and predicate checks in `locate`.
    fn locate_bound<'value>(
        &self,
        atoms: Atoms<'_>,
        value: impl Fn(usize) -> TermRef<'value>,
        work: &mut Work,
        descend: impl FnMut(bool),
    ) -> Result<Option<usize>, Failure> {
        ordered_index::search(
            &self.rows.nodes,
            self.rows.root,
            |row| compare_row(atoms, row, &value, work),
            descend,
        )
    }

    fn work(&self, limits: Limits) -> Result<Work, CatalogFailure> {
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
            Work::new(limits, self.retained_bytes() as u128)
        })();
        build.map_err(|error| CatalogFailure {
            error,
            work: 0,
            retained_bytes: self.retained_bytes(),
            peak_construction_bytes: self.retained_bytes(),
        })
    }

    fn failed(&self, error: Failure, work: &Work) -> CatalogFailure {
        CatalogFailure {
            error,
            work: work.used,
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
            + index_bytes(&self.rows)
            + self.ordered.capacity_bytes()
            + self.layout.dictionary.capacity() * size_of::<Cell>()
            + match &self.layout.index {
                DictionaryIndex::Sorted(ids) => ids.capacity() * size_of::<u32>(),
                DictionaryIndex::Append(index) => {
                    index_bytes(&index.order) + index.identities.bytes()
                }
            }
            + self.layout.columns.capacity() * size_of::<Vec<u32>>()
            + self
                .layout
                .columns
                .iter()
                .map(|column| column.capacity() * size_of::<u32>())
                .sum::<usize>()
    }

    fn receipt(&self, work: &Work) -> Storage {
        Storage {
            retained_bytes: self.retained_bytes(),
            peak_construction_bytes: work.peak,
            referenced_encoding_bytes: self.encoding_bytes,
            borrowed_mapping_bytes: 0,
            construction_work: work.used,
        }
    }
}

fn vector_bytes<T>(values: &Vec<T>) -> usize {
    values.capacity() * size_of::<T>()
}

fn index_bytes(index: &Index) -> usize {
    index.nodes.capacity() * size_of::<Node>() + index.path.capacity() * size_of::<Step>()
}

/// A query tuple against the indexed row `row`: its values in column order.
fn compare_row<'value>(
    atoms: Atoms<'_>,
    row: usize,
    value: &impl Fn(usize) -> TermRef<'value>,
    work: &mut Work,
) -> Result<std::cmp::Ordering, Failure> {
    work.tick(1)?;
    let atom = atoms.at(row).ok_or(Failure::CatalogIndex)?;
    for (column, right) in atom.values().iter().enumerate() {
        let order = work.compare(value(column), right)?;
        if !order.is_eq() {
            return Ok(order);
        }
    }
    Ok(std::cmp::Ordering::Equal)
}

#[cfg(test)]
mod tests;
