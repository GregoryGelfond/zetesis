//! One append authority and immutable, fixed-prefix canonical storage.
//!
//! Imports retain no `Value`/`Atom` payload. Children precede parents; text and
//! argument tuples occur once. Failed imports may retain complete interned
//! components and reserved capacity, but never expose a partial term or row.
//! Fixed owner, snapshot and segment envelopes use stable Rust's infallible
//! Box/Arc allocation; named buffers and indexes use checked fallible reserves.

use std::{fmt, mem::size_of, sync::Arc};

use crate::ValueError;

mod admission;
mod assigned;
mod lookup;
mod intern;
mod budget;
mod index;
mod control;
mod read;
mod segments;
mod nodes;
mod derived;
pub(super) use derived::DerivedTerm;
pub use derived::{DerivedFailure, DerivedTerms};
mod vocabulary;
mod publication;
mod rows;
mod closed;
use closed::IndexedRows;
pub(crate) use closed::{CloseError, Closed};
use rows::{RowBase, RowView};

use budget::Budget;
use index::Index;
pub(crate) use intern::PreparedAtom;
pub(crate) use lookup::TermLookup;
pub(crate) use read::{AtomScope, Read, VocabularyScope};
pub(super) use segments::{Atom, Predicate, Term};
use segments::{Counts, RowSegment, VocabularySegment};
pub(crate) use vocabulary::FrozenVocabulary;
use vocabulary::{Vocabulary, VocabularyData};

macro_rules! identifier {
    ($name:ident) => {
        /// Owner-local identity, not semantic order or an execution coordinate.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub(crate) struct $name(u32);
    };
}
identifier!(TextId);
identifier!(TermId);
impl TermId {
    pub(crate) fn position(self) -> usize {
        self.0 as usize
    }
}
identifier!(PredicateId);
identifier!(AtomId);

/// A typed refusal; previously published snapshots remain valid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The supplied logical value exceeds its admission contract.
    Value(ValueError),
    /// A named buffer or index could not reserve its required capacity.
    Allocation,
    /// Named storage capacities exceed the owner ceiling.
    Storage {
        /// Requested or observed named bytes, not allocator RSS.
        required: u128,
        /// Inclusive byte allowance.
        limit: usize,
    },
    /// Another stable identifier cannot be represented without truncation.
    IdExhausted,
    /// A checked size or expanded-tree measure cannot be represented.
    Overflow,
    /// The input does not describe a complete valid value or atom.
    Shape,
    /// A closed vocabulary cannot admit a new text, term or predicate.
    FrozenVocabulary,
    /// Vocabulary freezing requires an authority with no materialized atoms.
    VocabularyHasAtoms,
    /// A published read snapshot does not retain indexed vocabulary admission.
    UnindexedVocabulary,
    /// A closed-catalog descendant cannot itself become another shared base.
    CatalogHasBase,
}

impl From<ValueError> for Fault {
    fn from(value: ValueError) -> Self {
        Self::Value(value)
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(error) => error.fmt(f),
            Self::Allocation => f.write_str("canonical storage reservation failed"),
            Self::Storage { required, limit } => {
                write!(f, "canonical storage {required} exceeds {limit} bytes")
            }
            Self::IdExhausted => f.write_str("canonical identifier space exhausted"),
            Self::Overflow => f.write_str("canonical storage measure overflow"),
            Self::Shape => f.write_str("canonical input shape is invalid"),
            Self::FrozenVocabulary => {
                f.write_str("canonical frozen vocabulary does not contain this identity")
            }
            Self::VocabularyHasAtoms => {
                f.write_str("canonical vocabulary freeze contains atom rows")
            }
            Self::UnindexedVocabulary => {
                f.write_str("canonical snapshot has no vocabulary lookup indexes")
            }
            Self::CatalogHasBase => f.write_str("canonical catalog already extends a closed base"),
        }
    }
}
impl std::error::Error for Fault {}

/// Canonical import failed either in storage or at the caller's work boundary.
/// The caller's stop value is preserved without conversion to a storage fault.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure<E> {
    /// A logical, capacity, shape or allocation refusal.
    Storage(Fault),
    /// The caller stopped before the next admitted operation.
    Stopped(E),
}

impl<E> From<Fault> for Failure<E> {
    fn from(error: Fault) -> Self {
        Self::Storage(error)
    }
}

impl<E: fmt::Display> fmt::Display for Failure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => error.fmt(f),
            Self::Stopped(error) => error.fmt(f),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Failure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Storage(error) => error,
            Self::Stopped(error) => error,
        })
    }
}

/// Identity only: retaining this allocation cannot retain any payload or writer.
#[derive(Debug)]
struct Owner;

#[derive(Debug)]
pub(crate) struct Store {
    atom_owner: Arc<Owner>,
    vocabulary: Vocabulary,
    base: Option<Arc<IndexedRows>>,
    sealed: Vec<Arc<RowSegment>>,
    tail: RowSegment,
    atoms: Index,
    budget: Budget,
}

impl Store {
    pub(crate) const fn current_bytes(&self) -> u128 {
        self.budget.used
    }
    /// Named capacity and successful old/replacement overlap, not allocator RSS.
    pub(crate) const fn peak_bytes(&self) -> u128 {
        self.budget.peak
    }
    pub(crate) fn restart_peak(&mut self) {
        self.budget.peak = self.budget.used;
    }
    pub(crate) fn ceiling(&mut self, max_bytes: usize) -> Result<(), Fault> {
        self.budget.limit = max_bytes;
        self.budget.check()
    }

    pub(crate) fn new(max_bytes: usize) -> Self {
        Self {
            atom_owner: Arc::new(Owner),
            vocabulary: Vocabulary::new(),
            base: None,
            sealed: Vec::new(),
            tail: RowSegment::new(0),
            atoms: Index::default(),
            budget: Budget::new(
                max_bytes,
                size_of::<Self>() + size_of::<vocabulary::Growing>(),
            ),
        }
    }

    /// A closed execution keeps one exact vocabulary and a fresh atom scope.
    /// The shared base is included in this standalone writer's ceiling once.
    pub(crate) fn with_vocabulary(base: FrozenVocabulary, max_bytes: usize) -> Result<Self, Fault> {
        let mut budget = Budget::new(max_bytes, size_of::<Self>());
        budget.add(base.retained_bytes())?;
        Ok(Self {
            atom_owner: Arc::new(Owner),
            vocabulary: Vocabulary::Frozen(base),
            base: None,
            sealed: Vec::new(),
            tail: RowSegment::new(0),
            atoms: Index::default(),
            budget,
        })
    }

    /// Shared frozen allocations, already included in `current_bytes`. A combined
    /// owner ledger may union this base instead of summing it per writer.
    pub(crate) fn shared_vocabulary_bytes(&self) -> u128 {
        match &self.vocabulary {
            Vocabulary::Growing(_) => 0,
            Vocabulary::Frozen(base) => base.retained_bytes(),
        }
    }

    pub(crate) fn has_unpublished(&self) -> bool {
        !self.tail.is_empty() || self.vocabulary.has_unpublished()
    }

    /// Initial read for empty discovery. Growing vocabulary starts empty; a
    /// closed vocabulary and shared row base are available in full. Base rows
    /// acquire this writer's fresh atom scope, without asserting discovery.
    pub(crate) fn empty_snapshot(&self) -> Snapshot {
        Snapshot {
            atom_owner: Arc::clone(&self.atom_owner),
            data: Arc::new(SnapshotData {
                vocabulary: self.vocabulary.empty_prefix(),
                base: self.base.as_ref().map(|base| Arc::clone(&base.payload)),
                segments: Vec::new(),
                atoms: self.base_count(),
            }),
        }
    }

    pub(crate) fn check_publication(&self, extra: u128) -> Result<(), Fault> {
        self.budget.check_extra(extra)
    }

    fn counts(&self) -> Counts {
        let mut counts = self.vocabulary.counts();
        counts.atoms = self.tail.count();
        counts
    }

    fn vocabulary_segment(
        &self,
        id: usize,
        start: impl Fn(Counts) -> usize,
    ) -> Option<&VocabularySegment> {
        self.vocabulary.segment(id, start)
    }

    fn row_segment(&self, id: usize) -> Option<&RowSegment> {
        if id >= self.tail.start {
            Some(&self.tail)
        } else {
            self.rows().segment(id)
        }
    }

    fn rows(&self) -> RowView<'_> {
        RowView {
            base: self.base.as_ref().map(|base| base.payload.as_ref()),
            segments: &self.sealed,
        }
    }

    fn base_count(&self) -> usize {
        self.base.as_ref().map_or(0, |base| base.payload.atoms)
    }
}

#[derive(Debug)]
struct SnapshotData {
    vocabulary: Arc<VocabularyData>,
    base: Option<Arc<RowBase>>,
    segments: Vec<Arc<RowSegment>>,
    atoms: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Snapshot {
    atom_owner: Arc<Owner>,
    data: Arc<SnapshotData>,
}

impl Snapshot {
    pub(super) fn shares_snapshot(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.atom_owner, &other.atom_owner) && Arc::ptr_eq(&self.data, &other.data)
    }

    /// Allocation-free buffers; fixed Arc envelopes retain the standard Rust
    /// infallible allocation boundary.
    pub(super) fn empty() -> Self {
        Self {
            atom_owner: Arc::new(Owner),
            data: Arc::new(SnapshotData {
                vocabulary: Arc::new(VocabularyData::empty(Arc::new(Owner))),
                base: None,
                segments: Vec::new(),
                atoms: 0,
            }),
        }
    }
    pub(super) fn atom_count(&self) -> usize {
        self.data.atoms
    }
    pub(super) fn term_count(&self) -> usize {
        self.data.vocabulary.counts.terms
    }
    /// Independent prefix envelopes/directories; excludes a shared frozen base,
    /// segments, inline Snapshot, Arc counters and allocator metadata.
    pub(super) fn metadata_bytes(&self) -> u128 {
        size_of::<SnapshotData>() as u128
            + budget::capacity(&self.data.segments)
            + if self.data.vocabulary.frozen {
                0
            } else {
                self.data.vocabulary.metadata_bytes()
            }
    }
    pub(super) fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + size_of::<SnapshotData>() as u128
            + self.data.vocabulary.retained_bytes()
            + self.rows().bytes()
    }

    pub(super) fn retained_bytes_with<E>(
        &self,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<u128, E> {
        before()?;
        let mut bytes = size_of::<Self>() as u128
            + size_of::<SnapshotData>() as u128
            + self.data.vocabulary.metadata_bytes();
        for segment in &self.data.vocabulary.segments {
            before()?;
            bytes += segment.bytes();
        }
        bytes += self.rows().bytes_with(&mut before)?;
        Ok(bytes)
    }

    fn rows(&self) -> RowView<'_> {
        RowView {
            base: self.data.base.as_deref(),
            segments: &self.data.segments,
        }
    }
}

fn locate<T>(segments: &[Arc<T>], id: usize, start: impl Fn(&T) -> usize) -> Option<&T> {
    let after = segments.partition_point(|segment| start(segment) <= id);
    after.checked_sub(1).map(|index| segments[index].as_ref())
}

fn next_id(count: usize) -> Result<u32, Fault> {
    count.checked_add(1).ok_or(Fault::IdExhausted)?;
    u32::try_from(count).map_err(|_| Fault::IdExhausted)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod metered_tests;
#[cfg(test)]
mod frozen_tests;
#[cfg(test)]
mod constructor_tests;
