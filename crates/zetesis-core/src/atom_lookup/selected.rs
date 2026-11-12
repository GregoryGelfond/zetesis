//! Bounded seeding of the common index from selected original coordinates.

use std::{fmt, mem::size_of};

use zetesis_algebra::MeterBytes;

use super::{AtomIndex, AtomIndexError, Orders, Source, catalog, cells};

/// A refused selected index and its actual observed preparation storage.
///
/// No index is published. The peak excludes borrowed atoms and positions, as
/// does [`AtomIndex::from_catalog_selection_with`]. Requested but unallocated
/// capacity is reported by the byte refusal, not included in this peak.
#[derive(Debug, PartialEq, Eq)]
pub struct SelectedAtomIndexError<E> {
    kind: SelectedAtomIndexErrorKind<E>,
    peak_bytes: u128,
}

/// Why selected-index construction stopped.
#[derive(Debug, PartialEq, Eq)]
pub enum SelectedAtomIndexErrorKind<E> {
    /// The common index detected duplicate atoms, failed allocation or stopped.
    Index(AtomIndexError<E>),
    /// A selected position is outside the supplied catalog.
    Position {
        /// Offset in the borrowed selection.
        offset: usize,
        /// Invalid original catalog position.
        position: usize,
        /// Number of occurrences in that catalog.
        atoms: usize,
    },
    /// Positions must be strictly increasing, with no repeated occurrence.
    Order {
        /// Offset of the first nonincreasing position.
        offset: usize,
        /// Preceding original catalog position.
        previous: usize,
        /// Current original catalog position.
        position: usize,
    },
    /// Requested or actual index storage exceeded the explicit allowance.
    Bytes {
        /// Requested or observed simultaneous index bytes.
        required: u128,
        /// Supplied index-byte ceiling.
        limit: u128,
    },
}

impl<E> SelectedAtomIndexError<E> {
    /// Borrow the exact refusal, retaining any original callback error.
    #[must_use]
    pub const fn kind(&self) -> &SelectedAtomIndexErrorKind<E> {
        &self.kind
    }

    /// Recover the exact refusal, retaining any original callback error.
    #[must_use]
    pub fn into_kind(self) -> SelectedAtomIndexErrorKind<E> {
        self.kind
    }

    /// Maximum observed index/scratch headers and integer capacities before
    /// refusal. Allocator bookkeeping and borrowed input are excluded.
    #[must_use]
    pub const fn preparation_peak_bytes(&self) -> u128 {
        self.peak_bytes
    }
}

impl<E: fmt::Display> fmt::Display for SelectedAtomIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(formatter)
    }
}

impl<E: fmt::Display> fmt::Display for SelectedAtomIndexErrorKind<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Index(error) => error.fmt(formatter),
            Self::Position {
                offset,
                position,
                atoms,
            } => write!(
                formatter,
                "selected atom at offset {offset} has position {position} outside {atoms} occurrences"
            ),
            Self::Order {
                offset,
                previous,
                position,
            } => write!(
                formatter,
                "selected atom at offset {offset} has position {position} after {previous}"
            ),
            Self::Bytes { required, limit } => write!(
                formatter,
                "selected atom index requires {required} bytes, exceeding {limit}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SelectedAtomIndexError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        std::error::Error::source(&self.kind)
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SelectedAtomIndexErrorKind<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Index(error) => Some(error),
            Self::Position { .. } | Self::Order { .. } | Self::Bytes { .. } => None,
        }
    }
}

impl<'a> AtomIndex<'a> {
    /// Index selected original catalog positions without copying atom payload.
    ///
    /// Positions must be strictly increasing and in range. Lookups return those
    /// original positions, and predicate windows retain their occurrence order.
    /// Logical duplicates among selected atoms refuse through the common index;
    /// unselected occurrences are neither indexed nor checked for duplication.
    ///
    /// `max_index_bytes` admits this owner header, its two retained integer-vector
    /// capacities and one live merge-scratch header/capacity. Borrowed atoms and
    /// the selection slice remain the caller's responsibility. Every requested
    /// reservation is checked before allocation; actual capacity is recorded
    /// before enforcing its bound. Refusal never publishes a partial index.
    ///
    /// The initial `before` call precedes byte/selection validation, including
    /// for an empty selection. The empty index and scratch already exist, so
    /// their headers contribute to the peak even if that first call stops.
    /// The header bound then precedes position checks;
    /// each position's range check precedes its order check. Further callbacks
    /// precede each selected-cell validation/copy, reservation and common sort
    /// operation. The caller owns their work/cancellation policy. Preparation
    /// visits `m` selected positions and sorts `m` indices twice, retaining
    /// `O(m)` integers; comparisons also visit the selected atoms' typed payload.
    ///
    /// # Errors
    /// Returns the original callback failure, invalid selection, duplicate
    /// selected atom, allocation failure or index-byte refusal, with the actual
    /// observed preparation peak. Existing dense constructors are unchanged.
    pub fn from_catalog_selection_with<E>(
        atoms: catalog::Atoms<'a>,
        positions: &[usize],
        max_index_bytes: u128,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Self, SelectedAtomIndexError<E>> {
        let source = Source::Canonical(atoms);
        let mut index = Self {
            atoms: source,
            keys: Vec::new(),
            rows: Vec::new(),
            peak_bytes: 0,
        };
        let mut scratch = Vec::new();
        let mut memory = Memory::new(max_index_bytes);
        let result = (|| {
            checked(&mut before)?;
            memory.check(memory.live)?;
            validate(positions, atoms.len(), &mut before)?;
            memory.seed(&mut index.keys, positions, &mut before)?;
            memory.seed(&mut index.rows, positions, &mut before)?;
            memory.seed(&mut scratch, positions, &mut before)?;
            let orders = Orders::finish(source, index.keys, index.rows, scratch, &mut || {
                before().map_err(AtomIndexError::Stopped)
            })
            .map_err(SelectedAtomIndexErrorKind::Index)?;
            Ok(Self {
                atoms: source,
                keys: orders.keys,
                rows: orders.rows,
                peak_bytes: memory.peak,
            })
        })();
        result.map_err(|kind| SelectedAtomIndexError {
            kind,
            peak_bytes: memory.peak,
        })
    }
}

fn checked<E>(
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), SelectedAtomIndexErrorKind<E>> {
    before().map_err(|error| SelectedAtomIndexErrorKind::Index(AtomIndexError::Stopped(error)))
}

fn validate<E>(
    positions: &[usize],
    atoms: usize,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), SelectedAtomIndexErrorKind<E>> {
    let mut previous = None;
    for (offset, position) in positions.iter().enumerate() {
        checked(before)?;
        let position = *position;
        if position >= atoms {
            return Err(SelectedAtomIndexErrorKind::Position {
                offset,
                position,
                atoms,
            });
        }
        if let Some(previous) = previous
            && position <= previous
        {
            return Err(SelectedAtomIndexErrorKind::Order {
                offset,
                previous,
                position,
            });
        }
        previous = Some(position);
    }
    Ok(())
}

// Only three integer capacities are added to these fixed headers. Each capacity
// is a usize; with the supported at-most-64-bit usize, their byte sum fits u128.
// Sorting only swaps buffers, so it cannot increase this simultaneous peak.
struct Memory {
    live: u128,
    peak: u128,
    limit: u128,
}

impl Memory {
    fn new(limit: u128) -> Self {
        let live = size_of::<AtomIndex<'_>>() as u128 + size_of::<Vec<usize>>() as u128;
        Self {
            live,
            peak: live,
            limit,
        }
    }

    fn check<E>(&self, required: u128) -> Result<(), SelectedAtomIndexErrorKind<E>> {
        MeterBytes::check(required, self.limit).map_err(|_| SelectedAtomIndexErrorKind::Bytes {
            required,
            limit: self.limit,
        })
    }

    fn seed<E>(
        &mut self,
        result: &mut Vec<usize>,
        positions: &[usize],
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<(), SelectedAtomIndexErrorKind<E>> {
        // Each of the three initially empty owned buffers is seeded once.
        checked(before)?;
        self.check(self.live + cells(positions.len()))?;
        result
            .try_reserve_exact(positions.len())
            .map_err(|_| SelectedAtomIndexErrorKind::Index(AtomIndexError::Allocation))?;
        self.live += cells(result.capacity());
        MeterBytes::observe_peak(self.live, self.limit, &mut self.peak).map_err(|_| {
            SelectedAtomIndexErrorKind::Bytes {
                required: self.live,
                limit: self.limit,
            }
        })?;
        for position in positions {
            checked(before)?;
            result.push(*position);
        }
        Ok(())
    }
}
