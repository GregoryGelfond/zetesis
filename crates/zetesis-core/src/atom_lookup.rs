//! Borrowed predicate windows and exact keys over one authoritative atom owner.

use std::{cmp::Ordering, fmt, iter::FusedIterator, slice};

use crate::{
    Atom,
    catalog::{self, AtomCatalog, AtomRef, PredicateRef},
};

/// Transitional borrowed ingress and canonical execution views share one lookup.
/// Neither variant owns atom payload or materializes query results.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Source<'a> {
    Borrowed(&'a [Atom]),
    Canonical(catalog::Atoms<'a>),
}

impl<'a> Source<'a> {
    fn len(self) -> usize {
        match self {
            Self::Borrowed(atoms) => atoms.len(),
            Self::Canonical(atoms) => atoms.len(),
        }
    }

    fn at(self, position: usize) -> AtomRef<'a> {
        match self {
            Self::Borrowed(atoms) => AtomRef::from(&atoms[position]),
            Self::Canonical(atoms) => atoms.at(position).expect("indexed occurrence is checked"),
        }
    }
}

/// Prepared lookup indices over immutable, distinct atom occurrences.
///
/// Only original row numbers are owned. The supplied atoms are neither copied
/// nor reordered; all lookups and rows borrow that same source. A canonical key
/// index supports membership, while a predicate-grouped index retains original
/// row order inside each predicate. This second integer order is needed because
/// original dense IDs need not be canonical atom order. Legacy owned atom slices
/// remain a borrowed ingress bridge; canonical execution uses catalog views.
#[derive(Debug)]
pub struct AtomIndex<'a> {
    atoms: Source<'a>,
    keys: Vec<usize>,
    rows: Vec<usize>,
    peak_bytes: u128,
}

/// A checked index construction refusal, never absence of an atom.
#[derive(Debug, PartialEq, Eq)]
pub enum AtomIndexError<E> {
    /// Two original rows denote the same complete typed atom.
    Duplicate {
        /// Earlier original row of the equal atom.
        first: usize,
        /// Later original row of the equal atom.
        second: usize,
    },
    /// An index or merge-scratch reservation failed.
    Allocation,
    /// Caller work or cancellation refused before the next operation.
    Stopped(E),
}
impl<E: fmt::Display> fmt::Display for AtomIndexError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate { first, second } => {
                write!(f, "equal atoms at catalog rows {first} and {second}")
            }
            Self::Allocation => f.write_str("atom lookup index reservation failed"),
            Self::Stopped(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for AtomIndexError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Stopped(error) => Some(error),
            Self::Allocation | Self::Duplicate { .. } => None,
        }
    }
}

impl<'a> AtomIndex<'a> {
    /// Prepare both integer orders with a stable, fallible merge sort.
    ///
    /// For n supplied rows, reserves exactly n cells in each of two indices and
    /// one reusable merge scratch vector (allocator slack is possible). The
    /// caller must admit the input row count before calling; this row-derived
    /// allocation bound does not impose a process memory limit. Every reservation
    /// is fallible and preceded by `before`. No atom payload is allocated.
    /// Preparation uses O(n log n) atom comparisons and integer copies;
    /// canonical references additionally resolve immutable segment ranges.
    /// Each visited descriptor/text byte and each integer write calls `before`
    /// before the operation. There is no uncharged standard sort callback.
    ///
    /// # Errors
    /// Refuses duplicate atoms, reservation failure or the first caller error.
    /// No partial index escapes; original atoms and row identities remain intact.
    pub fn new_with<E>(
        atoms: &'a [Atom],
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Self, AtomIndexError<E>> {
        Self::prepare(Source::Borrowed(atoms), &mut before)
    }

    /// Prepare checked integer orders over canonical catalog occurrences.
    /// Uses the same fallible sorting and comparison boundaries as
    /// [`Self::new_with`], resolving borrowed canonical references instead of
    /// retaining an ingress atom slice. Duplicate logical occurrences refuse.
    ///
    /// # Errors
    /// Refuses duplicate atoms, reservation failure or the first caller error.
    pub fn from_catalog_with<E>(
        atoms: catalog::Atoms<'a>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Self, AtomIndexError<E>> {
        Self::prepare(Source::Canonical(atoms), &mut before)
    }

    fn prepare<E>(
        atoms: Source<'a>,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Self, AtomIndexError<E>> {
        let Orders {
            keys,
            rows,
            scratch_bytes,
        } = Orders::prepare(atoms, before)?;
        let retained =
            std::mem::size_of::<Self>() as u128 + cells(keys.capacity()) + cells(rows.capacity());
        Ok(Self {
            atoms,
            keys,
            rows,
            peak_bytes: retained + scratch_bytes,
        })
    }

    /// Borrow both prepared orders. No allocation, validation or payload copy.
    #[must_use]
    pub fn lookup(&self) -> AtomLookup<'_, 'a> {
        AtomLookup {
            atoms: self.atoms,
            keys: &self.keys,
            rows: &self.rows,
        }
    }

    /// Owner header plus actual retained integer-vector capacity, not source
    /// atoms, allocator bookkeeping or RSS.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        std::mem::size_of::<Self>() as u128
            + cells(self.keys.capacity())
            + cells(self.rows.capacity())
    }

    /// Maximum simultaneous index/scratch headers and integer capacities during
    /// this successful construction; source atom payload is excluded.
    #[must_use]
    pub const fn preparation_peak_bytes(&self) -> u128 {
        self.peak_bytes
    }
}

/// An [`AtomIndex`] over every atom of one catalog that holds its own handle
/// to that catalog, so it can outlive the borrow it was built from and be
/// shared by every reader of the catalog.
///
/// The handle shares the catalog's storage (no atom is copied); the index
/// owns only its two integer orders, as [`AtomIndex`] does. Lookups go
/// through the same [`AtomLookup`] view, over the catalog it holds, so a
/// lookup can never be applied to atoms the orders do not describe.
#[derive(Debug)]
pub struct CatalogIndex {
    catalog: AtomCatalog,
    keys: Vec<usize>,
    rows: Vec<usize>,
    peak_bytes: u128,
}

impl CatalogIndex {
    /// Prepare both integer orders over `catalog`'s occurrences, with the
    /// bounds, charges and refusals of [`AtomIndex::from_catalog_with`].
    ///
    /// # Errors
    /// Refuses duplicate atoms, reservation failure or the first caller error.
    /// No partial index escapes.
    pub fn new_with<E>(
        catalog: &AtomCatalog,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Self, AtomIndexError<E>> {
        let Orders {
            keys,
            rows,
            scratch_bytes,
        } = Orders::prepare(Source::Canonical(catalog.atoms()), &mut before)?;
        let retained =
            std::mem::size_of::<Self>() as u128 + cells(keys.capacity()) + cells(rows.capacity());
        Ok(Self {
            catalog: catalog.clone(),
            keys,
            rows,
            peak_bytes: retained + scratch_bytes,
        })
    }

    /// The catalog this index orders.
    #[must_use]
    pub const fn catalog(&self) -> &AtomCatalog {
        &self.catalog
    }

    /// Borrow both prepared orders over the held catalog. No allocation,
    /// validation or payload copy.
    #[must_use]
    pub fn lookup(&self) -> AtomLookup<'_, '_> {
        AtomLookup {
            atoms: Source::Canonical(self.catalog.atoms()),
            keys: &self.keys,
            rows: &self.rows,
        }
    }

    /// Owner header plus actual retained integer-vector capacity, not the
    /// shared catalog, allocator bookkeeping or RSS.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        std::mem::size_of::<Self>() as u128
            + cells(self.keys.capacity())
            + cells(self.rows.capacity())
    }

    /// Maximum simultaneous index/scratch headers and integer capacities during
    /// this successful construction; catalog atom payload is excluded.
    #[must_use]
    pub const fn preparation_peak_bytes(&self) -> u128 {
        self.peak_bytes
    }
}

/// Both checked integer orders of one preparation, and the merge scratch's
/// bytes (header and capacity), released when preparation ends.
struct Orders {
    keys: Vec<usize>,
    rows: Vec<usize>,
    scratch_bytes: u128,
}

impl Orders {
    fn prepare<E>(
        atoms: Source<'_>,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Self, AtomIndexError<E>> {
        let mut checked = || before().map_err(AtomIndexError::Stopped);
        let mut keys = positions(atoms.len(), &mut checked)?;
        let mut rows = positions(atoms.len(), &mut checked)?;
        let mut scratch = positions(atoms.len(), &mut checked)?;
        sort(
            &mut keys,
            &mut scratch,
            atoms,
            SortOrder::Identity,
            &mut checked,
        )?;
        for pair in keys.windows(2) {
            if atoms
                .at(pair[0])
                .equals_ref_with(atoms.at(pair[1]), &mut checked)?
            {
                return Err(AtomIndexError::Duplicate {
                    first: pair[0],
                    second: pair[1],
                });
            }
        }
        sort(
            &mut rows,
            &mut scratch,
            atoms,
            SortOrder::Predicate,
            &mut checked,
        )?;
        Ok(Self {
            keys,
            rows,
            scratch_bytes: std::mem::size_of::<Vec<usize>>() as u128 + cells(scratch.capacity()),
        })
    }
}

fn cells(capacity: usize) -> u128 {
    capacity as u128 * std::mem::size_of::<usize>() as u128
}

fn positions<E>(
    count: usize,
    before: &mut impl FnMut() -> Result<(), AtomIndexError<E>>,
) -> Result<Vec<usize>, AtomIndexError<E>> {
    before()?;
    let mut positions = Vec::new();
    positions
        .try_reserve_exact(count)
        .map_err(|_| AtomIndexError::Allocation)?;
    for index in 0..count {
        before()?;
        positions.push(index);
    }
    Ok(positions)
}

#[derive(Clone, Copy)]
enum SortOrder {
    Identity,
    Predicate,
}

fn sort<E>(
    values: &mut Vec<usize>,
    scratch: &mut Vec<usize>,
    atoms: Source<'_>,
    order: SortOrder,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), E> {
    crate::checked_sort::sort(
        values,
        scratch,
        |values, left, right, before| {
            let left = atoms.at(values[left]);
            let right = atoms.at(values[right]);
            match order {
                SortOrder::Identity => left.compare_ref_with(right, &mut *before),
                SortOrder::Predicate => left
                    .predicate()
                    .compare_ref_with(right.predicate(), &mut *before),
            }
        },
        before,
    )
}

/// Allocation-free lookup over a model selection or a checked catalog index.
/// The view cannot outlive either its index or the authoritative atom owner.
#[derive(Clone, Copy, Debug)]
pub struct AtomLookup<'index, 'source> {
    pub(crate) atoms: Source<'source>,
    pub(crate) keys: &'index [usize],
    pub(crate) rows: &'index [usize],
}

impl<'index, 'source> AtomLookup<'index, 'source> {
    /// Select exactly one signed predicate and arity with two binary bounds.
    /// Uses O(log n) predicate comparisons; compared name bytes and canonical
    /// segment resolution are additional.
    /// Calls `before` before each binary probe and descriptor/text comparison.
    /// Returned rows keep the owner's order within the predicate and borrow its
    /// indices; there is no positions vector or tuple copy per query.
    ///
    /// # Errors
    /// Returns the first callback error without publishing a partial range.
    pub fn predicate_with<'query, E>(
        self,
        predicate: impl Into<PredicateRef<'query>>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomRows<'index, 'source>, E> {
        let predicate = predicate.into();
        let low = self.bound(predicate, false, &mut before)?;
        let high = self.bound(predicate, true, &mut before)?;
        Ok(AtomRows {
            atoms: self.atoms,
            positions: self.rows[low..high].iter(),
        })
    }

    fn bound<E>(
        self,
        predicate: PredicateRef<'_>,
        after: bool,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<usize, E> {
        let (mut low, mut high) = (0, self.rows.len());
        while low < high {
            before()?;
            let middle = low + (high - low) / 2;
            let order = self
                .atoms
                .at(self.rows[middle])
                .predicate()
                .compare_ref_with(predicate, &mut *before)?;
            if order.is_lt() || (after && order.is_eq()) {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        Ok(low)
    }

    /// Find a complete typed atom with O(log n) atom comparisons, inspecting only
    /// visited descriptor/text prefixes. An absent atom returns `None`, including
    /// a value present only in an unselected catalog row. No Atom is constructed.
    ///
    /// # Errors
    /// Returns a callback error before the refused probe/comparison. A refusal
    /// is never returned as absence; neither the view nor the source changes.
    pub fn get_with<'query, E>(
        self,
        query: impl Into<AtomRef<'query>>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<AtomRow<'source>>, E> {
        let query = query.into();
        self.find_with(&mut before, |atom, before| {
            atom.compare_ref_with(query, before)
        })
    }

    /// Find a fully bound pattern key in the same canonical atom index used by
    /// [`Self::get_with`]. This borrows the pattern and binding without allocating
    /// an intermediate atom. Work and failure semantics are identical to that
    /// operation: each probe and visited identity descriptor is charged.
    ///
    /// # Errors
    /// Returns the callback error before the refused operation, never absence.
    pub fn get_key_with<E>(
        self,
        query: &crate::AtomKey<'_>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<AtomRow<'source>>, E> {
        self.find_with(&mut before, |atom, before| {
            atom.compare_key_with(query, before)
        })
    }

    fn find_with<E, F: FnMut() -> Result<(), E>>(
        self,
        before: &mut F,
        mut compare: impl FnMut(AtomRef<'source>, &mut F) -> Result<Ordering, E>,
    ) -> Result<Option<AtomRow<'source>>, E> {
        let (mut low, mut high) = (0, self.keys.len());
        while low < high {
            before()?;
            let middle = low + (high - low) / 2;
            let position = self.keys[middle];
            let atom = self.atoms.at(position);
            match compare(atom, before)? {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(Some(AtomRow { position, atom })),
            }
        }
        Ok(None)
    }
}

/// One borrowed original catalog row. Its position is meaningful only in the
/// lookup's source; it is not a global atom ID or proof of membership elsewhere.
#[derive(Clone, Copy, Debug)]
pub struct AtomRow<'a> {
    position: usize,
    atom: AtomRef<'a>,
}
impl<'a> AtomRow<'a> {
    /// Original dense catalog position, without reordering or reminting.
    #[must_use]
    pub const fn position(self) -> usize {
        self.position
    }
    /// Authoritative borrowed atom at that position.
    #[must_use]
    pub const fn atom(self) -> AtomRef<'a> {
        self.atom
    }
}

/// A predicate-local borrowed iterator. Each next/back operation reads one
/// position and resolves one atom reference, searching retained segment ranges
/// for canonical storage. It allocates nothing and performs no payload
/// comparison. The consuming algorithm accounts these visits separately from
/// the range search. Clone copies cursor state only.
#[derive(Clone, Debug)]
pub struct AtomRows<'index, 'source> {
    atoms: Source<'source>,
    positions: slice::Iter<'index, usize>,
}
impl<'source> Iterator for AtomRows<'_, 'source> {
    type Item = AtomRow<'source>;
    fn next(&mut self) -> Option<Self::Item> {
        self.positions.next().map(|&position| AtomRow {
            position,
            atom: self.atoms.at(position),
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.positions.size_hint()
    }
}
impl DoubleEndedIterator for AtomRows<'_, '_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.positions.next_back().map(|&position| AtomRow {
            position,
            atom: self.atoms.at(position),
        })
    }
}
impl ExactSizeIterator for AtomRows<'_, '_> {}
impl FusedIterator for AtomRows<'_, '_> {}
