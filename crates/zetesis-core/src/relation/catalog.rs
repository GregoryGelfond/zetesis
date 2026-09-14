//! Appendable extensional tuple ownership with immutable relation views.

use std::mem::size_of;

use crate::{Atom, AtomKey, Predicate, Value, identity, ordered_index::{self, Directions, Index, Link, Node, Step}};

mod plan;
mod ordered;
pub use ordered::OrderedRows;

use super::{
    Cell, DictionaryIndex, Failure, Layout, LayoutOwner, Limits, Relation, Resource, Source, Storage, Work, ceiling,
};

/// One signed predicate's unique typed atoms and appendable equality layout.
///
/// The catalog owns every atom once; dictionary entries are stable source-cell
/// positions. An immutable view borrows both owners, preventing mutation while
/// any query, row or selection is live. Existing rows and equality IDs survive
/// insertion; query identity remains the particular immutable Relation object.
///
/// Byte limits cover the catalog object, atom-vector cells, row index, equality
/// layout and operation scratch. The supplied atoms' nested payload allocations
/// remain the source admission caller's responsibility, as for borrowed views.
/// They are reported conservatively as referenced payload, not unique RSS.
/// Membership uses the same checked typed identity comparisons as AtomInterner.
/// Row and dictionary AVL indexes contain only IDs and links. Row membership
/// visits O(log n) nodes; dictionary membership visits O(log d), with typed
/// descriptor/text-prefix comparison work additional. Inserting a tuple with a
/// newly distinct values keeps O(a log d) tentative node patches; checked overlay scans can
/// cost O(a² log² d) metadata work. No historical sorted ID sequence is shifted.
/// Ordered access requires an explicitly prepared view, reusable until append.
/// Column/vector growth and ordered preparation have separate admitted costs.
pub struct Catalog {
    predicate: Predicate,
    atoms: Vec<Atom>,
    rows: Index,
    ordered: Vec<usize>,
    ordered_valid: bool,
    layout: Layout,
    payload: u128,
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
    /// Refuses shape, byte/work limits or allocation failure without an owner.
    pub fn new(predicate: Predicate, limits: Limits) -> Result<Self, CatalogFailure> {
        let mut work =
            Work::new(limits, size_of::<Self>() as u128).map_err(|error| CatalogFailure {
                error,
                work: 0,
                retained_bytes: 0,
                peak_construction_bytes: 0,
            })?;
        let build = (|| {
            ceiling(
                Resource::Columns,
                predicate.arity() as u128,
                limits.max_columns as u128,
            )?;
            let mut columns = work.reserve(predicate.arity())?;
            for _ in 0..predicate.arity() {
                work.tick(1)?;
                columns.push(Vec::new());
            }
            Ok(Self {
                predicate,
                atoms: Vec::new(),
                rows: Index::default(),
                ordered: Vec::new(),
                ordered_valid: true,
                layout: Layout {
                    dictionary: Vec::new(),
                    index: DictionaryIndex::Append(Index::default()),
                    columns,
                },
                payload: 0,
                construction: Storage {
                    retained_bytes: work.live,
                    peak_construction_bytes: work.peak,
                    referenced_payload_bytes: 0,
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

    /// Full signed predicate identity. Constant time.
    #[must_use]
    pub const fn predicate(&self) -> &Predicate {
        &self.predicate
    }

    /// Unique typed tuples in stable insertion order. Constant-time borrow.
    #[must_use]
    pub fn atoms(&self) -> &[Atom] {
        &self.atoms
    }

    /// Consume the owner without copying atoms or their nested payloads.
    /// The returned sequence retains insertion order.
    #[must_use]
    pub fn into_atoms(self) -> Vec<Atom> {
        self.atoms
    }

    /// Borrow the current layout without rebuilding its dictionary or columns.
    /// The source lifetime prevents append while this immutable view is borrowed.
    /// View accounting includes only its own object: the catalog's retained
    /// capacity remains the caller's separate owner charge.
    #[must_use]
    pub fn view(&self) -> Relation<'_> {
        Relation {
            predicate: &self.predicate,
            source: Source::Atoms(&self.atoms),
            layout: LayoutOwner::Borrowed(&self.layout),
            storage: Storage {
                retained_bytes: size_of::<Relation<'_>>(),
                peak_construction_bytes: size_of::<Relation<'_>>(),
                referenced_payload_bytes: self.payload,
                borrowed_mapping_bytes: 0,
                construction_work: 0,
            },
        }
    }

    /// Check exact typed tuple membership without constructing another atom.
    ///
    /// # Errors
    /// Refuses foreign predicates or work/byte ceilings. Comparisons include
    /// nested typed-value payload costs and do not rely on equality IDs alone.
    pub fn lookup(&self, atom: &Atom, limits: Limits) -> Result<Lookup, CatalogFailure> {
        let mut work = self.work(limits)?;
        self.locate(atom.predicate(), |column| &atom.values()[column], &mut work, |_| {})
            .map(|row| Lookup { row, storage: self.receipt(&work) })
            .map_err(|error| self.failed(error, &work))
    }

    /// Look up a borrowed substitution with the same comparison and accounting
    /// as [`Self::lookup`]. No atom or value payload is copied.
    ///
    /// # Errors
    /// Refuses foreign predicates or work/byte ceilings, preserving completed
    /// comparison work in the failure receipt.
    pub fn lookup_key(&self, key: &AtomKey<'_>, limits: Limits) -> Result<Lookup, CatalogFailure> {
        let mut work = self.work(limits)?;
        self.locate(key.predicate(), |column| key.argument(column), &mut work, |_| {})
            .map(|row| Lookup { row, storage: self.receipt(&work) })
            .map_err(|error| self.failed(error, &work))
    }

    /// Insert an atom only after every required reservation and check succeeds.
    ///
    /// # Errors
    /// Refuses predicate mismatch, inclusive limits, overflow or allocation
    /// failure. Logical contents and all published row/equality IDs remain
    /// unchanged on failure; successful reservations may retain spare capacity.
    /// Subsequent views and admissions include that actual retained capacity.
    pub fn insert(&mut self, atom: Atom, limits: Limits) -> Result<Insertion, CatalogFailure> {
        let mut work = self.work(limits)?;
        self.insert_inner(atom, &mut work)
            .map_err(|error| self.failed(error, &work))
    }

    fn insert_inner(&mut self, atom: Atom, work: &mut Work) -> Result<Insertion, Failure> {
        let mut route = Directions::default();
        if let Some(row) = self.locate(atom.predicate(), |column| &atom.values()[column], work,
            |right| route.push(right).expect("AVL height fits two words"))? {
            return Ok(Insertion { row, inserted: false, storage: self.receipt(work) });
        }
        let row = self.atoms.len();
        ceiling(Resource::Rows, row as u128 + 1, work.limits.max_rows as u128)?;
        let row_root = plan::row(&mut self.rows, row, &route, work)?;
        let plan = plan::values(&mut self.layout, &self.atoms, &atom, self.payload, work)?;
        self.reserve(&plan, work)?;
        Ok(self.publish(atom, row_root, plan, work))
    }

    fn reserve(&mut self, plan: &plan::Plan, work: &mut Work) -> Result<(), Failure> {
        work.grow(&mut self.atoms, 1)?;
        work.grow(&mut self.rows.nodes, 1)?;
        work.grow(&mut self.layout.dictionary, plan.added.len())?;
        let DictionaryIndex::Append(index) = &mut self.layout.index else {
            return Err(Failure::Dictionary);
        };
        work.grow(&mut index.nodes, plan.added.len())?;
        for column in &mut self.layout.columns {
            work.grow(column, 1)?;
        }
        // Publication has no callbacks, allocations, comparisons or failure.
        // Include the branch inspection of each row-path step and every patch.
        work.tick(self.rows.path.len() as u128 + plan.ids.len() as u128
            + plan.added.len() as u128 * 2 + plan.patches.len() as u128 + 3)?;
        Ok(())
    }

    fn publish(&mut self, atom: Atom, row_root: Link, plan: plan::Plan, work: &mut Work) -> Insertion {
        let row = self.atoms.len();
        let DictionaryIndex::Append(index) = &mut self.layout.index else {
            unreachable!("catalog owns an append index");
        };
        index.nodes.resize(index.nodes.len() + plan.added.len(), Node::default());
        for patch in &plan.patches {
            index.nodes[patch.id] = patch.node;
        }
        index.root = plan.root;
        self.layout.dictionary.extend_from_slice(&plan.added);
        for (column, id) in self.layout.columns.iter_mut().zip(plan.ids.iter().copied()) {
            column.push(id);
        }
        self.rows.publish(row_root);
        self.atoms.push(atom);
        self.payload = plan.payload;
        self.ordered_valid = false;
        plan.release(work);
        Insertion { row, inserted: true, storage: self.receipt(work) }
    }

    fn locate<'value>(
        &self,
        predicate: &Predicate,
        value: impl Fn(usize) -> &'value Value,
        work: &mut Work,
        descend: impl FnMut(bool),
    ) -> Result<Option<usize>, Failure> {
        if !identity::predicate(predicate, &self.predicate, &mut || work.tick(1))?.is_eq() {
            return Err(Failure::Predicate);
        }
        ordered_index::search(&self.rows.nodes, self.rows.root, |row| {
            work.tick(1)?;
            for (column, right) in self.atoms[row].values().iter().enumerate() {
                let order = work.compare(value(column), right)?;
                if !order.is_eq() {
                    return Ok(order);
                }
            }
            Ok(std::cmp::Ordering::Equal)
        }, descend)
    }

    fn work(&self, limits: Limits) -> Result<Work, CatalogFailure> {
        let build = (|| {
            ceiling(Resource::Rows, self.atoms.len() as u128, limits.max_rows as u128)?;
            ceiling(Resource::Columns, self.predicate.arity() as u128, limits.max_columns as u128)?;
            ceiling(Resource::Values, self.layout.dictionary.len() as u128, limits.max_values as u128)?;
            Work::new(limits, self.retained_bytes() as u128)
        })();
        build.map_err(|error| CatalogFailure { error, work: 0,
            retained_bytes: self.retained_bytes(), peak_construction_bytes: self.retained_bytes() })
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
            + self.atoms.capacity() * size_of::<Atom>()
            + index_bytes(&self.rows)
            + self.ordered.capacity() * size_of::<usize>()
            + self.layout.dictionary.capacity() * size_of::<Cell>()
            + match &self.layout.index {
                DictionaryIndex::Sorted(ids) => ids.capacity() * size_of::<u32>(),
                DictionaryIndex::Append(index) => index_bytes(index),
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
            referenced_payload_bytes: self.payload,
            borrowed_mapping_bytes: 0,
            construction_work: work.used,
        }
    }
}

fn index_bytes(index: &Index) -> usize {
    index.nodes.capacity() * size_of::<Node>() + index.path.capacity() * size_of::<Step>()
}

#[cfg(test)]
mod tests;
