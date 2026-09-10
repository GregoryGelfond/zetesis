//! Bounded immutable columns over typed relation rows.
//!
//! This experimental execution view preserves the source's signed predicate,
//! row order and duplicate row occurrences. It is a finite indexed tuple view:
//! two equal row occurrences do not denote distinct ASP atoms or additional
//! truth. Extensional atom uniqueness and source-instance provenance remain
//! separate caller contracts. It neither admits a program nor
//! establishes candidate truth. Equality identifiers have no numeric or ASP
//! ordering meaning. Typed row access decodes the same dictionary used by CPU
//! selection and device views.
//!
//! The source remains borrowed. Columns and dictionary references are owned;
//! no complete atom or logical payload is cloned. An eventual authoritative
//! column store must replace superseded tuple owners before production adoption.
//! Current structured-value clones already share their payload through `Arc`.
//!
//! Limits cover one operation's relation, supplied query/selection and newly
//! allocated buffers. Other live caller frames and borrowed source allocations
//! remain the caller's responsibility. Reported capacity is not process RSS.

use std::{fmt, mem::size_of};

use crate::{Atom, Predicate, Value};

mod storage;
mod selection;

pub use selection::{Equality, Query, Selection};

/// Inclusive construction and operation ceilings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Maximum source row occurrences, including duplicates.
    pub max_rows: usize,
    /// Maximum predicate arity.
    pub max_columns: usize,
    /// Maximum distinct typed dictionary values.
    pub max_values: usize,
    /// Maximum operation-scoped live owned capacity, including its input views.
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

/// Capacity and work observed while constructing one immutable view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Storage {
    /// Relation object, dictionary-reference capacity and column-ID capacity.
    pub retained_bytes: usize,
    /// Largest operation-scoped live capacity, including temporary sort buffers.
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
/// The dictionary borrows whole typed source values; columns own equality IDs.
/// Local row positions preserve input occurrence order. Catalog indices remain
/// separately accessible through [`Row::source_index`]. Identity is this live
/// owner, not its address retained after destruction, contents or dimensions.
pub struct Relation<'source> {
    predicate: &'source Predicate,
    source: Source<'source>,
    dictionary: Vec<&'source Value>,
    columns: Vec<u32>,
    storage: Storage,
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
        let start = column * self.row_count();
        Some(&self.columns[start..start + self.row_count()])
    }

    /// Borrow the column-major ID cells used by [`Self::column`].
    ///
    /// Shape is arity times row count, including an explicit zero-cell nullary
    /// relation. No dictionary re-interning is needed for a device copy.
    #[must_use]
    pub fn columns(&self) -> &[u32] {
        &self.columns
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
            self.dictionary.len() as u128,
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

impl Row<'_, '_> {
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
    #[must_use]
    pub fn value(&self, column: usize) -> Option<&Value> {
        let id = *self.relation.column(column)?.get(self.position)?;
        self.relation.dictionary.get(id as usize).copied()
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
        self.tick(1 + left.payload_bytes() as u128 + right.payload_bytes() as u128)?;
        Ok(left.cmp(right))
    }

    fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>, Failure> {
        let requested = self.live as u128 + count as u128 * size_of::<T>() as u128;
        ceiling(Resource::Bytes, requested, self.limits.max_bytes as u128)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| Failure::Allocation)?;
        let actual = self.live as u128 + values.capacity() as u128 * size_of::<T>() as u128;
        ceiling(Resource::Bytes, actual, self.limits.max_bytes as u128)?;
        self.live = usize::try_from(actual).map_err(|_| Failure::Overflow)?;
        self.peak = self.peak.max(self.live);
        Ok(values)
    }

    fn release<T>(&mut self, values: Vec<T>) {
        self.live -= values.capacity() * size_of::<T>();
        drop(values);
    }
}

#[cfg(test)]
mod tests;
