//! Canonical typed terms and atoms with immutable read snapshots.
//!
//! A catalog owns identities, not truth. Publishing a selection neither derives
//! its atoms nor establishes answer-set membership. Dense occurrence positions
//! remain distinct from canonical identities: equal atoms may occur at several
//! positions in a supplied program.

pub(crate) mod storage;
mod view;
mod membership;
mod constructor;
mod predicate_mask;
mod terms;
mod term_read;
mod vocabulary;

#[path = "atom_interner.rs"]
pub mod interner;
mod compare;

use std::{cmp::Ordering, fmt, iter::FusedIterator, marker::PhantomData, slice, sync::Arc};

use crate::Atom;

pub(crate) use constructor::ConstructorData;
pub use constructor::DeclaredConstructor;
pub use membership::{CatalogRead, DeclaredPredicate, ReadError};
pub(crate) use membership::{Member, Membership};
pub use predicate_mask::{PredicateMask, PredicateMaskFailure};
pub use storage::Fault as Error;
pub use storage::{DerivedFailure, DerivedTerms};
pub use term_read::TermRead;
pub use terms::{
    AssignmentError, AssignmentFailure, AssignmentSlice, TermAssignment, TermKey, TermSet,
};
pub use view::{ArgumentIter, Arguments, AtomRef, PredicateRef, TermNodes, TermRef};
pub use vocabulary::{Vocabulary, VocabularyBuilder, VocabularyFailure};

const LENGTH_BYTES: usize = size_of::<u64>();
const ATOM_HEADER_BYTES: usize = 2 * LENGTH_BYTES + 1;

/// Per-value limits for canonical import, construction and indexed lookup.
/// The writer's total named-storage ceiling is separate. Unlike
/// [`crate::ValueLimits`], bytes here do not count an owned flat node vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Maximum nodes in the expanded logical tree, counting repeated children.
    pub max_nodes: usize,
    /// Maximum root-inclusive logical depth.
    pub max_depth: usize,
    /// Maximum encoded identity and rendered spelling lengths plus temporary
    /// operation ID capacity. Spelling is measured, not retained as another value.
    pub max_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_nodes: 262_144,
            max_depth: 128,
            max_bytes: 16_777_216,
        }
    }
}

/// One append authority for canonical terms and complete atom tuples.
///
/// Read snapshots retain sealed allocations, never this writer or its indexes.
/// Appending cannot mutate a previously published model. The storage ceiling
/// measures the named capacities described by [`Error`], not process memory.
pub struct Catalog {
    store: storage::Store,
}

impl Catalog {
    /// Create an empty authority with an inclusive named-storage allowance,
    /// enforced before import and publication. The empty authority and its Arc
    /// envelope are constructed here. Zero is a real allowance, not unlimited.
    #[must_use]
    pub fn new(max_bytes: usize) -> Self {
        Self {
            store: storage::Store::new(max_bytes),
        }
    }

    /// Open a scoped append capability. Keys cannot escape this scope except
    /// inside a published catalog or model. This prevents mixing owner-local
    /// integers from unrelated catalogs without storing an owner in every cell.
    ///
    /// Successfully interned components may remain after a later refusal.
    /// Existing snapshots remain valid; no partial atom row is published.
    ///
    /// An atom key from another authority cannot be supplied to this builder:
    ///
    /// ```compile_fail
    /// use zetesis_core::{Atom, Predicate};
    /// use zetesis_core::catalog::{Catalog, Limits};
    /// let atom = Atom::new(Predicate::new("p", 0).unwrap(), vec![]).unwrap();
    /// let mut first = Catalog::new(1_000_000);
    /// let mut second = Catalog::new(1_000_000);
    /// first.edit(Limits::default(), |left| {
    ///     let foreign = left.intern_atom(&atom)?;
    ///     second.edit(Limits::default(), |right| right.publish(&[foreign]))
    /// });
    /// ```
    ///
    /// # Errors
    /// Returns construction, storage or caller failure from the operation.
    pub fn edit<R>(
        &mut self,
        limits: Limits,
        operation: impl for<'id> FnOnce(&mut Builder<'_, 'id>) -> Result<R, Error>,
    ) -> Result<R, Error> {
        operation(&mut Builder {
            store: &mut self.store,
            limits,
            brand: PhantomData,
        })
    }
}

/// Scoped construction in one canonical owner. Cloning an immutable result
/// never creates another append capability.
pub struct Builder<'store, 'id> {
    store: &'store mut storage::Store,
    limits: Limits,
    brand: PhantomData<fn(&'id ()) -> &'id ()>,
}

/// A complete canonical atom identity valid only in one edit scope.
/// The key denotes neither an occurrence position nor a true atom.
#[derive(Clone, Copy)]
pub struct AtomKey<'id> {
    id: storage::AtomId,
    brand: PhantomData<fn(&'id ()) -> &'id ()>,
}

impl<'id> Builder<'_, 'id> {
    /// Intern a typed ingress description. Equal complete atoms share one row;
    /// strings and symbols remain distinct and argument order is preserved.
    ///
    /// # Errors
    /// Refuses exceeded logical or storage bounds before publishing the atom.
    pub fn intern_atom(&mut self, atom: &Atom) -> Result<AtomKey<'id>, Error> {
        self.store.import_atom(atom, self.limits).map(|id| AtomKey {
            id,
            brand: PhantomData,
        })
    }

    /// Publish original occurrence positions over the current immutable prefix.
    /// Repeated keys retain repeated positions. Only integer mappings are copied;
    /// authoritative term and atom payloads remain in sealed shared storage.
    ///
    /// # Errors
    /// Refuses allocation or publication bounds without exposing a partial catalog.
    pub fn publish(&mut self, atoms: &[AtomKey<'id>]) -> Result<AtomCatalog, Error> {
        self.store
            .check_publication(occurrence_bytes(atoms.len()))?;
        let mut occurrences = Vec::new();
        occurrences
            .try_reserve_exact(atoms.len())
            .map_err(|_| Error::Allocation)?;
        occurrences.extend(atoms.iter().map(|atom| atom.id));
        let snapshot = self
            .store
            .snapshot(occurrence_bytes(occurrences.capacity()))?;
        Ok(AtomCatalog::published(snapshot, occurrences))
    }
}

/// Immutable original atom occurrences over one canonical typed store.
///
/// Equal logical atoms may occupy different dense positions. Clone shares the
/// snapshot and occurrence mapping in constant time. Models retain the declared
/// prefix, including unselected occurrences, but never later append segments.
#[derive(Clone)]
pub struct AtomCatalog(Arc<CatalogData>);

struct CatalogData {
    snapshot: storage::Snapshot,
    occurrences: Vec<storage::AtomId>,
    canonical_bytes: Option<usize>,
}

/// Retained canonical-prefix population and named storage.
/// These counts include the entire retained prefix, not only selected atoms or
/// the occurrences in one model. They establish neither truth nor support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Storage {
    /// Distinct typed terms, including interned subterms.
    pub terms: usize,
    /// Distinct complete atoms in the retained canonical prefix.
    pub atoms: usize,
    /// Named retained headers and capacities, including the occurrence mapping.
    /// Arc reference counters and allocator bookkeeping are excluded. Adding
    /// this value across shared catalogs overcounts shared allocations; it is
    /// not the portable result-admission measure or process RSS.
    pub bytes: u128,
}

impl AtomCatalog {
    /// Import owned atom descriptions into canonical column storage, preserving
    /// their original occurrence order. Input addresses and capacities are not
    /// retained. This replaces the former vector-adoption contract.
    ///
    /// Values have already been admitted by their constructors. This operation
    /// does not impose the default source depth limit a second time. Checked ID
    /// widths, arithmetic and fallible buffer reservations still apply. Arc
    /// envelope allocation remains infallible on stable Rust.
    ///
    /// # Errors
    /// Returns a canonical storage or representation refusal; no catalog escapes.
    pub fn new(atoms: Vec<Atom>) -> Result<Self, Error> {
        let mut store = storage::Store::new(usize::MAX);
        let mut occurrences = Vec::new();
        occurrences
            .try_reserve_exact(atoms.len())
            .map_err(|_| Error::Allocation)?;
        let limits = Limits {
            max_nodes: usize::MAX,
            max_depth: usize::MAX,
            max_bytes: usize::MAX,
        };
        for atom in atoms {
            occurrences.push(store.import_atom(&atom, limits)?);
        }
        Ok(Self::published(
            store.snapshot(occurrence_bytes(occurrences.capacity()))?,
            occurrences,
        ))
    }

    fn published(snapshot: storage::Snapshot, occurrences: Vec<storage::AtomId>) -> Self {
        let canonical_bytes = occurrences.iter().try_fold(LENGTH_BYTES, |bytes, &id| {
            let atom = AtomRef::new(&snapshot, id).expect("published atom belongs to snapshot");
            let bytes = bytes
                .checked_add(ATOM_HEADER_BYTES)?
                .checked_add(atom.predicate().name().len())?;
            atom.values().iter().try_fold(bytes, |bytes, term| {
                bytes.checked_add(term.canonical_bytes())
            })
        });
        Self(Arc::new(CatalogData {
            snapshot,
            occurrences,
            canonical_bytes,
        }))
    }

    /// Borrow original dense-order occurrences without allocating or copying
    /// typed payload. Resolving a reference searches immutable segment ranges.
    #[must_use]
    pub fn atoms(&self) -> Atoms<'_> {
        Atoms {
            snapshot: storage::Read::from(&self.0.snapshot),
            positions: &self.0.occurrences,
        }
    }

    /// Borrow this catalog's complete immutable canonical prefix. Occurrence
    /// selection does not restrict which admitted identities can be resolved;
    /// resolution alone establishes no support or truth.
    #[must_use]
    pub fn read(&self) -> CatalogRead<'_> {
        CatalogRead(storage::Read::from(&self.0.snapshot))
    }

    /// This publication's private header and occurrence-map capacity.
    /// The immutable snapshot and canonical segments are shared with its writer
    /// or other readers and are excluded here. Add this amount to an owner
    /// ledger that already counts that exact prefix, rather than adding the
    /// catalog's complete retained storage a second time. Arc bookkeeping and
    /// allocator metadata remain outside named capacity accounting.
    #[must_use]
    pub fn publication_bytes(&self) -> u128 {
        occurrence_bytes(self.0.occurrences.capacity())
    }

    /// Retained capacity of the occurrence-to-identity mapping, in cells.
    /// Canonical payload storage is separate; input vector capacity is not kept.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.0.occurrences.capacity()
    }

    /// Whether both handles share the exact occurrence catalog allocation.
    /// This is not logical equality or merely a shared term-store lineage.
    #[must_use]
    pub fn same_owner(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Whether both catalogs retain the exact same immutable prefix allocation.
    /// Their occurrence maps may differ. Equal contents, a common vocabulary or
    /// partially shared segments do not establish this stronger relation.
    #[must_use]
    pub fn shares_snapshot(&self, other: &Self) -> bool {
        self.0.snapshot.shares_snapshot(&other.0.snapshot)
    }

    /// Shared immutable prefix allocations, excluding this catalog's header and
    /// occurrence map. Subtract this subtotal once only when `shares_snapshot`
    /// establishes that two catalogs retain exactly these same allocations.
    /// Measuring visits sealed segment metadata; this is not process memory.
    #[must_use]
    pub fn shared_snapshot_bytes(&self) -> u128 {
        self.0.snapshot.retained_bytes() - size_of::<storage::Snapshot>() as u128
    }

    /// Whether two catalogs share their exact canonical term authority.
    /// Atom-row authorities, prefixes and occurrence positions may still differ.
    #[must_use]
    pub fn shares_terms(&self, other: &Self) -> bool {
        storage::Read::from(&self.0.snapshot)
            .same_vocabulary(storage::Read::from(&other.0.snapshot))
    }

    /// Portable encoded payload of all declared occurrences, including repeats
    /// and unselected atoms. Canonical identities outside that occurrence map
    /// are excluded, even though the shared prefix keeps them alive. See
    /// [`Self::storage`] for named retained storage; neither measure is RSS.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> Option<usize> {
        self.0.canonical_bytes
    }

    /// Inspect the immutable prefix's population and named retained storage.
    /// Work is proportional to the number of sealed storage segments. Later
    /// append operations cannot change the returned charge for this catalog.
    #[must_use]
    pub fn storage(&self) -> Storage {
        Storage {
            terms: self.0.snapshot.term_count(),
            atoms: self.0.snapshot.atom_count(),
            bytes: self.0.snapshot.retained_bytes()
                + (size_of::<CatalogData>() - size_of::<storage::Snapshot>()) as u128
                + (self.0.occurrences.capacity() as u128) * size_of::<storage::AtomId>() as u128,
        }
    }

    /// Checked version of [`Self::storage`]. Admission precedes each retained
    /// vocabulary/row segment, predicate-column group and argument capacity read.
    /// It visits metadata only, not individual atoms or term payload bytes.
    ///
    /// # Errors
    /// Returns the first caller refusal without a completed storage measure.
    pub fn storage_with<E>(&self, mut before: impl FnMut() -> Result<(), E>) -> Result<Storage, E> {
        let bytes = self.0.snapshot.retained_bytes_with(&mut before)?;
        before()?;
        Ok(Storage {
            terms: self.0.snapshot.term_count(),
            atoms: self.0.snapshot.atom_count(),
            bytes: bytes + self.publication_bytes() - size_of::<storage::Snapshot>() as u128,
        })
    }

    pub(crate) fn owner_key(&self) -> usize {
        Arc::as_ptr(&self.0).addr()
    }
}

fn occurrence_bytes(capacity: usize) -> u128 {
    size_of::<CatalogData>() as u128 + capacity as u128 * size_of::<storage::AtomId>() as u128
}

impl fmt::Debug for AtomCatalog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AtomCatalog")
            .field("atoms", &self.atoms())
            .finish()
    }
}

impl Default for AtomCatalog {
    fn default() -> Self {
        Self::published(storage::Snapshot::empty(), Vec::new())
    }
}

/// Allocation-free view of original dense atom occurrences.
#[derive(Clone, Copy)]
pub struct Atoms<'a> {
    snapshot: storage::Read<'a>,
    positions: &'a [storage::AtomId],
}

impl<'a> Atoms<'a> {
    /// Borrow the already-bound canonical prefix, without another scope handle.
    pub(crate) fn read(self) -> CatalogRead<'a> {
        CatalogRead(self.snapshot)
    }

    fn new(snapshot: impl Into<storage::Read<'a>>, positions: &'a [storage::AtomId]) -> Self {
        Self {
            snapshot: snapshot.into(),
            positions,
        }
    }
    /// Number of original occurrences, including repeated logical atoms.
    #[must_use]
    pub fn len(self) -> usize {
        self.positions.len()
    }

    /// Whether this occurrence range is empty.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.positions.is_empty()
    }

    /// Borrow the occurrence at a checked dense position.
    #[must_use]
    pub fn at(self, position: usize) -> Option<AtomRef<'a>> {
        self.positions.get(position).map(|&id| self.resolve(id))
    }

    /// Resolve an admitted identity in this immutable occurrence map. The map's
    /// construction establishes prefix membership once, before any view exists.
    fn resolve(self, id: storage::AtomId) -> AtomRef<'a> {
        AtomRef::new(self.snapshot, id).expect("catalog occurrence is admitted")
    }

    /// Whether two live views borrow the exact same occurrence mapping and atom
    /// authority. Logical equality alone does not establish position identity.
    /// Constant time; neither view can outlive its mapping.
    #[must_use]
    pub fn same_occurrences(self, other: Self) -> bool {
        self.snapshot.same_atoms(other.snapshot) && std::ptr::eq(self.positions, other.positions)
    }

    /// Compare positions already checked against this exact occurrence view.
    pub(crate) fn compare_positions(self, left: usize, right: usize) -> Ordering {
        self.resolve(self.positions[left])
            .cmp(&self.resolve(self.positions[right]))
    }

    /// Search occurrences already sorted in semantic atom order. The returned
    /// position is a dense occurrence, never a canonical identity. As for a
    /// sorted slice, an unsorted input has no specified search result.
    /// No allocation or atom materialization occurs.
    ///
    /// # Errors
    /// Returns the insertion position when no occurrence denotes the atom.
    pub fn binary_search(self, atom: AtomRef<'_>) -> Result<usize, usize> {
        self.positions
            .binary_search_by(|&id| self.resolve(id).cmp(&atom))
    }

    pub(crate) fn binary_search_key(self, key: &crate::AtomKey<'_>) -> Result<usize, usize> {
        self.positions
            .binary_search_by(|&id| key.compare_ref(self.resolve(id)).reverse())
    }

    /// Iterate in original dense order; cloning the cursor copies only borrows.
    #[must_use]
    pub fn iter(self) -> AtomIter<'a> {
        AtomIter {
            snapshot: self.snapshot,
            positions: self.positions.iter(),
        }
    }
}

impl fmt::Debug for Atoms<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for Atoms<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}
impl Eq for Atoms<'_> {}
impl PartialOrd for Atoms<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Atoms<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<'a> IntoIterator for Atoms<'a> {
    type Item = AtomRef<'a>;
    type IntoIter = AtomIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Exact, double-ended traversal of original atom occurrences.
#[derive(Clone)]
pub struct AtomIter<'a> {
    snapshot: storage::Read<'a>,
    positions: slice::Iter<'a, storage::AtomId>,
}

impl<'a> Iterator for AtomIter<'a> {
    type Item = AtomRef<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let id = *self.positions.next()?;
        Some(AtomRef::new(self.snapshot, id).expect("catalog occurrence is admitted"))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.positions.size_hint()
    }
}

impl DoubleEndedIterator for AtomIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let id = *self.positions.next_back()?;
        Some(AtomRef::new(self.snapshot, id).expect("catalog occurrence is admitted"))
    }
}

impl ExactSizeIterator for AtomIter<'_> {}
impl FusedIterator for AtomIter<'_> {}
