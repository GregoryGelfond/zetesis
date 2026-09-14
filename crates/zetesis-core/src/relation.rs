//! Bounded immutable columns over typed relation rows.
//!
//! This execution view preserves the source's signed predicate,
//! row order and duplicate row occurrences. It is a finite indexed tuple view:
//! two equal row occurrences do not denote distinct ASP atoms or additional
//! truth. Extensional atom uniqueness and source-instance provenance remain
//! separate caller contracts. It neither admits a program nor
//! establishes candidate truth. Equality identifiers have no numeric or ASP
//! ordering meaning. Typed row access decodes the same dictionary used by CPU
//! selection and device views.
//!
//! The source remains borrowed. Columns and dictionary representatives are
//! either owned by this view or borrowed from an appendable [`Catalog`]; no
//! complete atom or logical payload is cloned. A catalog mediates every append
//! and owns its typed tuples once. Other callers retain their authoritative
//! source and must remove superseded tuple owners when adopting this view.
//! Eager formula support uses columns for typed lookup and
//! row access; device consumers use the same representation for equality masks.
//! Structured-value clones already share their payload through `Arc`.
//!
//! The pre-1.0 `Catalog::ordered_row` operation is replaced by explicit
//! [`Catalog::prepare_ordered`] and [`Catalog::ordered`] views. A missing prepared
//! view denotes required preparation, never an empty relation. Relation and
//! catalog work now charge actual typed descriptor/text-prefix comparisons;
//! previous numerical work ceilings are not equivalent units.
//!
//! Limits cover one operation's relation, supplied query/selection and newly
//! allocated buffers. Other live caller frames and borrowed source allocations
//! remain the caller's responsibility. Reported capacity is not process RSS.

use std::{fmt, mem::size_of};

use crate::{Atom, Predicate, Value};

mod storage;
mod selection;
mod catalog;

pub use catalog::{Catalog, CatalogFailure, Insertion, Lookup, OrderedRows};

pub use selection::{Equality, Mask, Query, Selection};

/// Inclusive construction and operation ceilings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Maximum source row occurrences, including duplicates.
    pub max_rows: usize,
    /// Maximum predicate arity.
    pub max_columns: usize,
    /// Maximum distinct typed dictionary values.
    pub max_values: usize,
    /// Maximum operation-scoped named capacity, including input views and the
    /// conservative old/replacement-buffer overlap of a growing owner.
    pub max_bytes: usize,
    /// Maximum charged inspections, comparisons, copied reference/ID cells and payload bytes.
    pub max_work: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_rows: 65_536,
            max_columns: 64,
            max_values: 1_048_576,
            max_bytes: 134_217_728,
            max_work: 100_000_000,
        }
    }
}

/// The finite resource whose inclusive ceiling was exceeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    /// Source row occurrences.
    Rows,
    /// Predicate arity.
    Columns,
    /// Distinct typed dictionary entries.
    Values,
    /// Operation-scoped live owned capacity.
    Bytes,
    /// Charged logical work.
    Work,
}

/// A relation operation failed without publishing a partial result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// A source atom has a different signed predicate or arity.
    Predicate,
    /// An input catalog index is outside the borrowed atom catalog.
    CatalogIndex,
    /// A requested equality column is outside the predicate's arity.
    Column,
    /// Selection positions are not increasing, unique and in range.
    Selection,
    /// A packed selection has the wrong word count or nonzero unused tail bits.
    Mask,
    /// The supplied query or selection belongs to a different relation object.
    Owner,
    /// A validated source value was unexpectedly absent from its dictionary.
    Dictionary,
    /// A shape or capacity cannot be represented.
    Overflow,
    /// Storage could not be reserved.
    Allocation,
    /// An inclusive ceiling was exceeded.
    Limit {
        /// Exhausted resource.
        resource: Resource,
        /// Requested amount.
        observed: u128,
        /// Inclusive allowance.
        limit: u128,
    },
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Predicate => f.write_str("relation row has a foreign predicate"),
            Self::CatalogIndex => f.write_str("relation catalog index is out of range"),
            Self::Column => f.write_str("relation equality column is out of range"),
            Self::Selection => {
                f.write_str("relation selection is not ordered, unique and in range")
            }
            Self::Mask => f.write_str("relation mask has the wrong shape or nonzero tail bits"),
            Self::Owner => f.write_str("relation query or selection has a foreign owner"),
            Self::Dictionary => f.write_str("relation dictionary does not contain a source value"),
            Self::Overflow => f.write_str("relation shape or capacity is not representable"),
            Self::Allocation => f.write_str("relation storage could not be reserved"),
            Self::Limit {
                resource,
                observed,
                limit,
            } => {
                write!(f, "relation {resource:?} {observed} exceeds {limit}")
            }
        }
    }
}

impl std::error::Error for Failure {}

/// Capacity and work observed during one relation or catalog operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Storage {
    /// Operation owner's object and retained vector capacities. A borrowed
    /// catalog view excludes the catalog's separately reported owner capacity.
    pub retained_bytes: usize,
    /// Largest operation-scoped capacity envelope, including temporary buffers
    /// and old/replacement capacity overlap when a reservation occurs.
    pub peak_construction_bytes: usize,
    /// Sum of referenced source-value payload per occurrence, not unique memory.
    ///
    /// Repeated values and shared structural payload are counted repeatedly.
    /// These borrowed bytes are excluded from `retained_bytes`.
    pub referenced_payload_bytes: u128,
    /// Borrowed source catalog-index slice bytes; zero for contiguous rows.
    pub borrowed_mapping_bytes: usize,
    /// Charged construction work; payload comparisons are not unit-cost integers.
    pub construction_work: u128,
}

/// One immutable execution view of a signed predicate relation.
///
/// The dictionary refers to whole typed source values; columns hold equality IDs.
/// Borrowed-source construction owns the layout. A catalog view borrows it.
/// Local row positions preserve input occurrence order. Catalog indices remain
/// separately accessible through [`Row::source_index`]. Identity is this live
/// owner, not its address retained after destruction, contents or dimensions.
pub struct Relation<'source> {
    predicate: &'source Predicate,
    source: Source<'source>,
    layout: LayoutOwner<'source>,
    storage: Storage,
}

/// One equality layout shared by borrowed-source construction and owned catalogs.
/// Dictionary representatives are source positions, never duplicated values.
struct Layout {
    dictionary: Vec<Cell>,
    index: DictionaryIndex,
    columns: Vec<Vec<u32>>,
}

enum DictionaryIndex {
    Sorted(Vec<u32>),
    Append(crate::ordered_index::Index),
}

#[derive(Clone, Copy)]
struct Cell {
    row: usize,
    column: usize,
}

impl Cell {
    fn value<'source>(self, source: &Source<'source>) -> Result<&'source Value, Failure> {
        source
            .atom(self.row)
            .and_then(|atom| atom.values().get(self.column))
            .ok_or(Failure::CatalogIndex)
    }
}

enum LayoutOwner<'source> {
    Owned(Layout),
    Borrowed(&'source Layout),
}

impl std::ops::Deref for LayoutOwner<'_> {
    type Target = Layout;
    fn deref(&self) -> &Layout {
        match self {
            Self::Owned(layout) => layout,
            Self::Borrowed(layout) => layout,
        }
    }
}

enum Source<'source> {
    Atoms(&'source [Atom]),
    Catalog {
        atoms: &'source [Atom],
        indices: &'source [usize],
    },
}

impl<'source> Source<'source> {
    fn len(&self) -> usize {
        match self {
            Self::Atoms(atoms) => atoms.len(),
            Self::Catalog { indices, .. } => indices.len(),
        }
    }

    fn index(&self, row: usize) -> Option<usize> {
        match self {
            Self::Atoms(atoms) => (row < atoms.len()).then_some(row),
            Self::Catalog { indices, .. } => indices.get(row).copied(),
        }
    }

    fn atom(&self, row: usize) -> Option<&'source Atom> {
        let index = self.index(row)?;
        match self {
            Self::Atoms(atoms) | Self::Catalog { atoms, .. } => atoms.get(index),
        }
    }
}

impl<'source> Relation<'source> {
    /// Build columns without cloning atoms or their logical payload.
    ///
    /// The explicit predicate also identifies empty/nullary relations. Duplicate
    /// atom occurrences are retained. Construction uses O(c log c + c log d)
    /// typed comparisons for c argument cells and d distinct values, plus their
    /// payload costs. Temporary sort/reference capacity is charged.
    ///
    /// # Errors
    /// Refuses foreign predicates, exceeded limits, unrepresentable shapes or
    /// allocation failure. No partial relation is returned.
    pub fn from_atoms(
        predicate: &'source Predicate,
        atoms: &'source [Atom],
        limits: Limits,
    ) -> Result<Self, Failure> {
        storage::build(predicate, Source::Atoms(atoms), limits)
    }

    /// Build an ordered predicate view over explicit original catalog indices.
    ///
    /// Indices are borrowed and checked; arbitrary input order and repeated
    /// indices remain distinct row occurrences. This supplies a legitimate
    /// original-ID view without renumbering the catalog. Costs match
    /// [`Self::from_atoms`], with an additional borrowed index per row.
    ///
    /// # Errors
    /// Also refuses out-of-range catalog indices. All selected atoms must have
    /// the supplied signed predicate.
    pub fn from_catalog(
        predicate: &'source Predicate,
        atoms: &'source [Atom],
        indices: &'source [usize],
        limits: Limits,
    ) -> Result<Self, Failure> {
        storage::build(predicate, Source::Catalog { atoms, indices }, limits)
    }

    /// The full signed predicate. Constant-time borrow.
    #[must_use]
    pub const fn predicate(&self) -> &Predicate {
        self.predicate
    }

    /// Number of row occurrences, independent of arity. Constant time.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.source.len()
    }

    /// Charged construction and retained-storage observations. Constant time.
    #[must_use]
    pub const fn storage(&self) -> Storage {
        self.storage
    }

    /// Borrow one contiguous equality-ID column in original row order.
    ///
    /// IDs are meaningful only with this relation's dictionary. They must not
    /// be interpreted as numbers, term-order ranks or candidate truth.
    #[must_use]
    pub fn column(&self, column: usize) -> Option<&[u32]> {
        if column >= self.predicate.arity() {
            return None;
        }
        self.layout.columns.get(column).map(Vec::as_slice)
    }

    /// Borrow columns in original argument order without packing or allocation.
    ///
    /// Exactly arity slices are returned, each containing `row_count` IDs.
    /// Nullary relations yield no slices. A device consumer may copy these
    /// slices consecutively into its admitted column-major upload buffer.
    #[must_use]
    pub fn columns(&self) -> impl ExactSizeIterator<Item = &[u32]> {
        self.layout.columns.iter().map(Vec::as_slice)
    }

    /// Access a typed row occurrence without allocating or cloning an atom.
    #[must_use]
    pub fn row(&self, position: usize) -> Option<Row<'_, 'source>> {
        self.source.index(position).map(|source_index| Row {
            relation: self,
            position,
            source_index,
        })
    }

    /// Whether two live borrows identify exactly the same immutable owner.
    ///
    /// Equal rows, dimensions, predicates or overlapping lifetimes are not
    /// sufficient. No token can outlive the owner.
    #[must_use]
    pub fn same_owner(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }

    fn work(&self, limits: Limits, additional: usize) -> Result<Work, Failure> {
        ceiling(
            Resource::Rows,
            self.row_count() as u128,
            limits.max_rows as u128,
        )?;
        ceiling(
            Resource::Columns,
            self.predicate.arity() as u128,
            limits.max_columns as u128,
        )?;
        ceiling(
            Resource::Values,
            self.layout.dictionary.len() as u128,
            limits.max_values as u128,
        )?;
        Work::new(
            limits,
            self.storage.retained_bytes as u128 + additional as u128,
        )
    }
}

/// Borrowed typed row access, separate from an owned `Atom` representation.
#[derive(Clone, Copy)]
pub struct Row<'owner, 'source> {
    relation: &'owner Relation<'source>,
    position: usize,
    source_index: usize,
}

impl<'source> Row<'_, 'source> {
    /// The full signed predicate. Constant time.
    #[must_use]
    pub fn predicate(&self) -> &Predicate {
        self.relation.predicate()
    }

    /// Original occurrence position within this relation. Constant time.
    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }

    /// Index in the borrowed source atom slice/catalog. Constant time.
    ///
    /// This is distinct from local position for catalog views. Its meaning
    /// depends on the source catalog used to construct the relation.
    #[must_use]
    pub const fn source_index(&self) -> usize {
        self.source_index
    }

    /// Decode one argument to an equal borrowed dictionary representative.
    ///
    /// Interning preserves typed value equality, not the address of the original
    /// occurrence's `Value` cell. [`Self::source_index`] retains its catalog identity.
    /// The value borrows the source and can outlive this row and relation view.
    #[must_use]
    pub fn value(&self, column: usize) -> Option<&'source Value> {
        let id = *self.relation.column(column)?.get(self.position)?;
        self.relation
            .layout
            .dictionary
            .get(id as usize)?
            .value(&self.relation.source)
            .ok()
    }
}

fn ceiling(resource: Resource, observed: u128, limit: u128) -> Result<(), Failure> {
    if observed > limit {
        Err(Failure::Limit {
            resource,
            observed,
            limit,
        })
    } else {
        Ok(())
    }
}

struct Work {
    limits: Limits,
    used: u128,
    live: usize,
    peak: usize,
}

impl Work {
    fn new(limits: Limits, bytes: u128) -> Result<Self, Failure> {
        ceiling(Resource::Bytes, bytes, limits.max_bytes as u128)?;
        let live = usize::try_from(bytes).map_err(|_| Failure::Overflow)?;
        Ok(Self {
            limits,
            used: 0,
            live,
            peak: live,
        })
    }

    fn tick(&mut self, amount: u128) -> Result<(), Failure> {
        let next = self.used.checked_add(amount).ok_or(Failure::Overflow)?;
        ceiling(Resource::Work, next, u128::from(self.limits.max_work))?;
        self.used = next;
        Ok(())
    }

    fn compare(&mut self, left: &Value, right: &Value) -> Result<std::cmp::Ordering, Failure> {
        left.compare_identity_with(right, || self.tick(1))
    }

    fn include(&mut self, bytes: usize) -> Result<(), Failure> {
        let next = self.live.checked_add(bytes).ok_or(Failure::Overflow)?;
        ceiling(Resource::Bytes, next as u128, self.limits.max_bytes as u128)?;
        self.live = next;
        self.peak = self.peak.max(next);
        Ok(())
    }

    fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>, Failure> {
        let requested = self.live as u128 + count as u128 * size_of::<T>() as u128;
        ceiling(Resource::Bytes, requested, self.limits.max_bytes as u128)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| Failure::Allocation)?;
        let actual = self.live as u128 + values.capacity() as u128 * size_of::<T>() as u128;
        self.live = usize::try_from(actual).map_err(|_| Failure::Overflow)?;
        self.peak = self.peak.max(self.live);
        ceiling(Resource::Bytes, actual, self.limits.max_bytes as u128)?;
        Ok(values)
    }

    fn grow<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<(), Failure> {
        let needed = values
            .len()
            .checked_add(additional)
            .ok_or(Failure::Overflow)?;
        if needed <= values.capacity() {
            return Ok(());
        }
        let previous = values.capacity();
        let proposed = needed.max(previous.saturating_mul(2));
        ceiling(
            Resource::Bytes,
            self.live as u128 + proposed as u128 * size_of::<T>() as u128,
            self.limits.max_bytes as u128,
        )?;
        self.tick(values.len() as u128)?;
        values
            .try_reserve_exact(proposed - values.len())
            .map_err(|_| Failure::Allocation)?;
        // Reservation may transiently retain both old and replacement buffers.
        // Record actual slack even when that post-allocation envelope is refused.
        let overlap = self.live as u128 + values.capacity() as u128 * size_of::<T>() as u128;
        let actual = self.live as u128
            + (values.capacity() - previous) as u128 * size_of::<T>() as u128;
        self.live = usize::try_from(actual).map_err(|_| Failure::Overflow)?;
        self.peak = self.peak.max(usize::try_from(overlap).map_err(|_| Failure::Overflow)?);
        ceiling(Resource::Bytes, overlap, self.limits.max_bytes as u128)?;
        Ok(())
    }

    fn release<T>(&mut self, values: Vec<T>) {
        self.live -= values.capacity() * size_of::<T>();
        drop(values);
    }
}

#[cfg(test)]
mod tests;
