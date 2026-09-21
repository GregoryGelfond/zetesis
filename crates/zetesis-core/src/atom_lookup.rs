//! Borrowed predicate windows and exact keys over one authoritative atom owner.

use std::{cmp::Ordering, fmt, iter::FusedIterator, slice};

use crate::{Atom, Predicate, identity};

/// Prepared lookup indices over an immutable, distinct atom catalog.
///
/// Only original row numbers are owned. The supplied atoms are neither copied
/// nor reordered; all lookups and rows borrow that same source. A canonical key
/// index supports membership, while a predicate-grouped index retains original
/// row order inside each predicate. This second integer order is needed because
/// original dense IDs need not be canonical atom order.
#[derive(Debug)]
pub struct AtomIndex<'a> {
    atoms: &'a [Atom],
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
    /// Preparation uses O(n log n) descriptor comparisons and integer copies;
    /// each visited descriptor/text byte and each integer write calls `before`
    /// before the operation. There is no uncharged standard sort callback.
    ///
    /// # Errors
    /// Refuses duplicate atoms, reservation failure or the first caller error.
    /// No partial index escapes; original atoms and row identities remain intact.
    pub fn new_with<E>(
        atoms: &'a [Atom],
        mut before: impl FnMut() -> Result<(), E>,
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
            if identity::atom(&atoms[pair[0]], &atoms[pair[1]], &mut checked)?.is_eq() {
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
        let retained =
            std::mem::size_of::<Self>() as u128 + cells(keys.capacity()) + cells(rows.capacity());
        Ok(Self {
            atoms,
            keys,
            rows,
            peak_bytes: retained
                + std::mem::size_of::<Vec<usize>>() as u128
                + cells(scratch.capacity()),
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
    atoms: &[Atom],
    order: SortOrder,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), E> {
    let count = values.len();
    let mut width = 1;
    while width < count {
        let mut start = 0;
        while start < count {
            let middle = start.saturating_add(width).min(count);
            let end = middle.saturating_add(width).min(count);
            let (mut left, mut right) = (start, middle);
            for output in &mut scratch[start..end] {
                let take_left = if right == end {
                    true
                } else if left == middle {
                    false
                } else {
                    let left = &atoms[values[left]];
                    let right = &atoms[values[right]];
                    !match order {
                        SortOrder::Identity => identity::atom(left, right, before)?,
                        SortOrder::Predicate => {
                            identity::predicate(left.predicate(), right.predicate(), before)?
                        }
                    }
                    .is_gt()
                };
                before()?;
                *output = if take_left {
                    let value = values[left];
                    left += 1;
                    value
                } else {
                    let value = values[right];
                    right += 1;
                    value
                };
            }
            start = end;
        }
        std::mem::swap(values, scratch);
        width = width.saturating_mul(2);
    }
    Ok(())
}

/// Allocation-free lookup over a model selection or a checked catalog index.
/// The view cannot outlive either its index or the authoritative atom owner.
#[derive(Clone, Copy, Debug)]
pub struct AtomLookup<'index, 'source> {
    pub(crate) atoms: &'source [Atom],
    pub(crate) keys: &'index [usize],
    pub(crate) rows: &'index [usize],
}

impl<'index, 'source> AtomLookup<'index, 'source> {
    /// Select exactly one signed predicate and arity with two binary bounds.
    /// Uses O(log n) predicate comparisons; compared name bytes are additional.
    /// Calls `before` before each binary probe and descriptor/text comparison.
    /// Returned rows keep the owner's order within the predicate and borrow its
    /// indices; there is no positions vector or tuple copy per query.
    ///
    /// # Errors
    /// Returns the first callback error without publishing a partial range.
    pub fn predicate_with<E>(
        self,
        predicate: &Predicate,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomRows<'index, 'source>, E> {
        let low = self.bound(predicate, false, &mut before)?;
        let high = self.bound(predicate, true, &mut before)?;
        Ok(AtomRows {
            atoms: self.atoms,
            positions: self.rows[low..high].iter(),
        })
    }

    fn bound<E>(
        self,
        predicate: &Predicate,
        after: bool,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<usize, E> {
        let (mut low, mut high) = (0, self.rows.len());
        while low < high {
            before()?;
            let middle = low + (high - low) / 2;
            let order =
                identity::predicate(self.atoms[self.rows[middle]].predicate(), predicate, before)?;
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
    pub fn get_with<E>(
        self,
        query: &Atom,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<AtomRow<'source>>, E> {
        self.find_with(&mut before, |atom, before| {
            identity::atom(atom, query, before)
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
            query
                .compare_identity_with(atom, before)
                .map(Ordering::reverse)
        })
    }

    fn find_with<E, F: FnMut() -> Result<(), E>>(
        self,
        before: &mut F,
        mut compare: impl FnMut(&Atom, &mut F) -> Result<Ordering, E>,
    ) -> Result<Option<AtomRow<'source>>, E> {
        let (mut low, mut high) = (0, self.keys.len());
        while low < high {
            before()?;
            let middle = low + (high - low) / 2;
            let position = self.keys[middle];
            let atom = &self.atoms[position];
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
    atom: &'a Atom,
}
impl<'a> AtomRow<'a> {
    /// Original dense catalog position, without reordering or reminting.
    #[must_use]
    pub const fn position(self) -> usize {
        self.position
    }
    /// Authoritative borrowed atom at that position.
    #[must_use]
    pub const fn atom(self) -> &'a Atom {
        self.atom
    }
}

/// A predicate-local borrowed iterator. Each next/back operation costs one
/// index access and one atom access, allocates nothing and performs no payload
/// comparison. The consuming algorithm accounts those visits separately from
/// the range search. Clone copies cursor state only.
#[derive(Clone, Debug)]
pub struct AtomRows<'index, 'source> {
    atoms: &'source [Atom],
    positions: slice::Iter<'index, usize>,
}
impl<'source> Iterator for AtomRows<'_, 'source> {
    type Item = AtomRow<'source>;
    fn next(&mut self) -> Option<Self::Item> {
        self.positions.next().map(|&position| AtomRow {
            position,
            atom: &self.atoms[position],
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
            atom: &self.atoms[position],
        })
    }
}
impl ExactSizeIterator for AtomRows<'_, '_> {}
impl FusedIterator for AtomRows<'_, '_> {}
