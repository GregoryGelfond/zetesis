//! One appendable typed atom owner with stable, owner-local dense positions.
//!
//! A committed prefix may be borrowed during a synchronous source scan while a
//! disjoint appender interns newly encountered identities. New discoveries do not
//! enter that prefix until commit. Neither identity nor commitment asserts truth.
//! Canonical payload occurs once; ordered views and discovery maps hold only IDs.
//! [`crate::AtomCatalog::new`] separately preserves arbitrary occurrence order and
//! duplicate positions, without retaining the supplied description addresses.
//!
//! This [`AtomInterner`] is the atom owner grounding uses today. Its positions
//! are discovery coordinates, not content order. It is distinct from the
//! generation-freezing [`crate::intern::Interner`], which grounding does not yet
//! call.

mod closed;
mod discovery;
mod ordering;
mod query;
mod terms;
pub use closed::{CloseFailure, ClosedCatalog};
pub use terms::{
    AssignedFailure, PreparedFacts, PreparedPattern, PreparedRows, RowColumn, TermLookup,
};

use std::{collections::TryReserveError, fmt};

use zetesis_algebra::{Meter, MeterBytes};

use super::{
    AtomCatalog, AtomRef, Atoms, CatalogRead, DeclaredConstructor, DeclaredPredicate,
    Limits as TermLimits, PredicateRef,
    storage::{self, AtomId, Snapshot, Store},
};
use crate::AtomKey;
use discovery::Discovery;
use query::{Identity, Query};

/// Owned prepared-result slot retained throughout an exclusive entry.
const PREPARED_BYTES: u128 = size_of::<Option<storage::PreparedAtom>>() as u128;

/// An immutable candidate position, never tuple identity or absence. A prepared
/// pattern authenticates the writer; each consumer checks the current signed
/// predicate at its position before using the ordinary authoritative owner.
#[derive(Clone, Copy, Debug)]
struct PredicateLocations {
    column: usize,
}

/// Bounds on this interner's population and named storage, independent of truth.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Maximum discovered atoms, including the uncommitted discovery tail.
    /// Undiscovered canonical rows in a shared closed base count only as storage.
    pub max_atoms: usize,
    /// Canonical store, discovery maps, current snapshot directory, order
    /// scratch, projected-input and prepared-result headers, including
    /// admitted growth overlap. Named canonical text and term storage is
    /// included. Allocator bookkeeping, Arc counters, caller frames and
    /// externally retained snapshots remain separate.
    pub max_bytes: u128,
}

impl Limits {
    /// Combine an explicit canonical-storage allowance with discovery metadata.
    ///
    /// Uses the actual ID layouts: three n-cell discovery buffers, a discovery
    /// table across one growth, one n-cell order with two n-cell merge buffers,
    /// an (n+1)-cell run-start buffer and an n-bit selection mask with their
    /// headers, plus the owner, largest
    /// projected-input and prepared-result headers. This conservatively includes
    /// geometric old/new-buffer overlap; it is not a requirement to allocate all
    /// those buffers. `canonical_bytes` is a caller-selected allowance for the
    /// store and snapshot directories; atom count alone cannot bound arbitrary
    /// text and terms. Actual allocation capacity is rechecked.
    #[must_use]
    pub fn for_atoms(max_atoms: usize, canonical_bytes: usize) -> Self {
        Self {
            max_atoms,
            max_bytes: canonical_bytes as u128
                + size_of::<AtomInterner>() as u128
                + PREPARED_BYTES
                + query::Projected::MAX_BYTES
                + 3 * cells::<AtomId>(max_atoms)
                + discovery::bound(max_atoms)
                + 4 * cells::<usize>(max_atoms)
                + cells::<usize>(1)
                + 4 * size_of::<Vec<usize>>() as u128
                + cells::<u64>(max_atoms.div_ceil(64))
                + size_of::<Vec<u64>>() as u128,
        }
    }
}

/// Refusal to complete an index operation; never absence or logical rejection.
#[derive(Debug)]
pub enum Failure<E> {
    /// Canonical term or tuple construction could not complete.
    Catalog(super::Error),
    /// The distinct population would exceed its ceiling.
    Atoms {
        /// Requested total population.
        required: usize,
        /// Configured maximum.
        limit: usize,
    },
    /// A named storage envelope exceeds its ceiling.
    Bytes {
        /// Requested bytes, including a reservation's old/new overlap.
        required: u128,
        /// Configured maximum.
        limit: u128,
    },
    /// A vector reservation failed.
    Allocation(TryReserveError),
    /// A size or position calculation is not representable.
    Overflow,
    /// The caller stopped work before the next operation.
    Stopped(E),
}

impl<E: fmt::Display> fmt::Display for Failure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalog(error) => error.fmt(f),
            Self::Atoms { required, limit } => write!(
                f,
                "atom interner requests {required} atoms, limit is {limit}"
            ),
            Self::Bytes { required, limit } => write!(
                f,
                "atom interner requests {required} storage bytes, limit is {limit}"
            ),
            Self::Allocation(error) => write!(f, "atom interner reservation: {error}"),
            Self::Overflow => f.write_str("atom interner size is not representable"),
            Self::Stopped(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for Failure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Catalog(error) => Some(error),
            Self::Allocation(error) => Some(error),
            Self::Stopped(error) => Some(error),
            Self::Atoms { .. } | Self::Bytes { .. } | Self::Overflow => None,
        }
    }
}

/// Authoritative unique atoms in first-insertion order, with one derived hash
/// view for exact local identity. Typed order is sorted on demand.
///
/// Authenticated canonical queries use Store's exact row index and an ID-only
/// inverse to discovery positions. Foreign/ingress queries first resolve their
/// signed predicate, each argument and then the row by content through the
/// same exact hash indexes; any missing component proves absence. No ordered
/// index is maintained: insertion publishes the canonical identity and its
/// discovery position only. Typed enumeration sorts the selected positions
/// with the one typed atom comparison, by a natural merge sort that pays
/// n - 1 comparisons for positions already in typed order. Vector growth is geometric (work admission
/// includes possible relocation of live cells even for in-place allocator
/// growth); commit moves only the pending suffix.
/// Positions are local to this owner; equal atoms in unrelated owners need not
/// have equal positions. There is no process-global interning or shared truth.
pub struct AtomInterner {
    store: Store,
    snapshot: Snapshot,
    committed: Vec<AtomId>,
    pending: Vec<AtomId>,
    /// Greatest admitted named storage of the current operation.
    peak: u128,
    /// Discovery positions by canonical identity: a hash view, no order.
    discovery: Discovery,
}

impl Default for AtomInterner {
    fn default() -> Self {
        Self::from_store(Store::new(usize::MAX))
    }
}

impl AtomInterner {
    fn from_store(store: Store) -> Self {
        let snapshot = store.empty_snapshot();
        Self {
            store,
            snapshot,
            committed: Vec::new(),
            pending: Vec::new(),
            peak: 0,
            discovery: Discovery::default(),
        }
    }

    /// Create an empty owner. Canonical identity and empty snapshot Arc envelopes
    /// allocate; tuple buffers and indexes remain empty.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an independent tuple authority over an admitted template vocabulary.
    /// Shared terms and predicates retain their canonical identity; atoms receive
    /// a fresh scope. No new vocabulary is admitted by the returned writer.
    /// The allowance includes the shared vocabulary and this writer's metadata.
    ///
    /// # Errors
    /// Refuses an initial footprint beyond `max_bytes`, or `UnindexedVocabulary`
    /// for selected read snapshots, which deliberately retain no lookup indexes.
    pub fn for_template_catalog(
        catalog: &crate::template::TemplateCatalog,
        max_bytes: usize,
    ) -> Result<Self, super::Error> {
        Self::with_vocabulary(
            catalog
                .vocabulary()
                .ok_or(super::Error::UnindexedVocabulary)?
                .clone(),
            max_bytes,
        )
    }

    fn with_vocabulary(
        vocabulary: super::storage::FrozenVocabulary,
        max_bytes: usize,
    ) -> Result<Self, super::Error> {
        let owner = Self::from_store(Store::with_vocabulary(vocabulary, max_bytes)?);
        MeterBytes::check(owner.storage_bytes(), max_bytes as u128).map_err(|_| {
            super::Error::Storage {
                required: owner.storage_bytes(),
                limit: max_bytes,
            }
        })?;
        Ok(owner)
    }

    /// Shared immutable vocabulary already included in [`Self::storage_bytes`].
    /// A combined template-catalog/tuple ledger counts that same retained base once.
    /// General evolving owners have no separately shared vocabulary allocation.
    #[must_use]
    pub fn shared_vocabulary_bytes(&self) -> u128 {
        self.store.shared_vocabulary_bytes()
    }

    /// Distinct committed and pending atom count. Constant time.
    #[must_use]
    pub fn len(&self) -> usize {
        self.committed.len() + self.pending.len()
    }

    /// Whether no identity has been inserted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Borrow a local position, including its pending identity when present.
    #[must_use]
    pub fn get(&self, id: usize) -> Option<AtomRef<'_>> {
        get(&self.store, &self.committed, &self.pending, id)
    }

    /// Borrow the complete current canonical prefix, including pending identity.
    /// This grants no extensional truth and excludes mutation for its lifetime.
    #[must_use]
    pub fn read(&self) -> CatalogRead<'_> {
        CatalogRead(storage::Read::from(&self.store))
    }

    /// Declare a signed predicate without inventing an atom or discovering truth.
    /// Predicate storage is shared with later tuples of that signature.
    ///
    /// # Errors
    /// Returns storage or caller refusal. Complete canonical components may
    /// remain, but the discovery population is unchanged.
    pub fn declare_predicate_with<'predicate, E>(
        &mut self,
        predicate: impl Into<PredicateRef<'predicate>>,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<DeclaredPredicate, Failure<E>> {
        self.appender()
            .declare_predicate_with(predicate, limits, before)
    }

    /// Find a borrowed atom's local position in committed and pending identities.
    ///
    /// Borrows the owner immutably and changes no payload, index, scratch or
    /// capacity observation. Calls `before` before each visited index
    /// operation. Authenticated local atoms use their identity directly; local
    /// tuples use Store's exact coordinate hash index. Both resolve the
    /// discovery position through the fixed-ID hash inverse. Foreign/ingress
    /// coordinates first resolve by content through the same exact hash
    /// indexes, holding one arity-cell argument buffer and the term lookup's
    /// expansion scratch; a missing component proves absence.
    ///
    /// # Errors
    /// Current population and storage must satisfy `limits` before probing.
    /// Returns the first callback refusal without publishing an ID or absence.
    pub fn find_atom_with<'atom, E>(
        &self,
        atom: impl Into<AtomRef<'atom>>,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<usize>, Failure<E>> {
        self.find(Query::Atom(atom.into()), limits, before)
    }

    /// Find the same argument tuple under an explicit classical predicate sign.
    /// This borrows the original name and values; it neither materializes an
    /// opposite atom nor adds a predicate or row. Checked lookup preserves the
    /// population, storage and callback contract of [`Self::find_atom_with`].
    ///
    /// # Errors
    /// Returns admission or caller refusal before producing a lookup result.
    pub fn find_signed_atom_with<E>(
        &self,
        atom: AtomRef<'_>,
        sign: crate::Sign,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<usize>, Failure<E>> {
        self.find(Query::SignedAtom(atom, sign), limits, before)
    }

    /// Find a checked borrowed substitution without materializing an atom.
    /// Work, ownership and capacity contracts match [`Self::find_atom_with`].
    ///
    /// # Errors
    /// Same admission and callback failures as [`Self::find_atom_with`].
    pub fn find_key_with<E>(
        &self,
        key: AtomKey<'_>,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<usize>, Failure<E>> {
        self.find(Query::Key(key), limits, before)
    }

    fn find<E>(
        &self,
        query: Query<'_>,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<usize>, Failure<E>> {
        Lookup {
            store: &self.store,
            committed: &self.committed,
            pending: &self.pending,
            discovery: &self.discovery,
            bytes: self.storage_bytes(),
        }
        .find(query, limits, before)
    }

    /// Current named canonical and discovery capacities, including typed payload.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        self.store.current_bytes()
            + self.snapshot.metadata_bytes()
            + storage(
                self.committed.capacity(),
                self.pending.capacity(),
                self.discovery.bytes(),
            )
    }

    /// Greatest admitted live capacity or actual reservation-overlap envelope.
    /// A preflighted proposal stopped before allocation does not increase it. Allocator slack
    /// acquired before a later refusal remains reflected here and in capacity.
    #[must_use]
    pub fn storage_peak_bytes(&self) -> u128 {
        self.peak.max(self.storage_bytes())
    }

    /// Begin a new operation's peak observation at current retained capacity.
    /// Identity and discovery are unchanged; later limits still charge all
    /// retained storage, including unused canonical identities from prior work.
    pub fn restart_storage_peak(&mut self) {
        self.store.restart_peak();
        self.peak = self.storage_bytes();
    }

    /// Borrow the current committed prefix without mutating the owner.
    #[must_use]
    pub fn committed(&self) -> CommittedAtoms<'_> {
        CommittedAtoms {
            atoms: Atoms::new(&self.snapshot, &self.committed),
        }
    }

    /// Split committed rows from a disjoint append capability. Neither borrow
    /// can survive commit. Preparing row/membership selections before this call
    /// lets a synchronous scanner coexist with append and index growth.
    #[must_use]
    pub fn split(&mut self) -> (CommittedAtoms<'_>, AtomAppender<'_>) {
        let capacity = self.committed.capacity();
        (
            CommittedAtoms {
                atoms: Atoms::new(&self.snapshot, &self.committed),
            },
            AtomAppender {
                store: &mut self.store,
                snapshot_bytes: self.snapshot.metadata_bytes(),
                committed: &self.committed,
                committed_capacity: capacity,
                pending: &mut self.pending,
                peak: &mut self.peak,
                discovery: &mut self.discovery,
            },
        )
    }

    /// Borrow an appender without retaining a separate committed view.
    #[must_use]
    pub fn appender(&mut self) -> AtomAppender<'_> {
        self.split().1
    }

    /// Find or prepare an entry from an already owned atom, without copying it.
    /// Entry records each descent in fixed local stack state without changing
    /// retained scratch. The charged node operation includes that constant-size
    /// recording; typed comparison work is separate. Only a vacant entry replays
    /// the recorded child links to prepare its insertion path. The two target-
    /// sized words and checked length live only through this call.
    ///
    /// # Errors
    /// Returns the first work, storage or reservation refusal. Vacant-path
    /// preparation may retain grown scratch capacity; membership is unchanged.
    pub fn entry_atom_with<'owner, 'key, E>(
        &'owner mut self,
        atom: impl Into<AtomRef<'key>>,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomEntry<'owner, 'key>, Failure<E>> {
        self.appender()
            .entry(Query::Atom(atom.into()), limits, before)
    }

    /// Find or prepare an entry using a borrowed checked substitution.
    ///
    /// # Errors
    /// Same failure and retained-capacity contract as [`Self::entry_atom_with`].
    pub fn entry_key_with<'owner, 'key, E>(
        &'owner mut self,
        key: AtomKey<'key>,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomEntry<'owner, 'key>, Failure<E>> {
        self.appender().entry(Query::Key(key), limits, before)
    }

    /// Return committed positions in exact typed Atom order. Owns only the
    /// positions; no Atom is copied. A natural merge sort with the typed atom
    /// comparison orders the committed prefix: n - 1 charged comparisons when
    /// discovery order is already typed order, O(n log n) at worst. The returned vector's capacity
    /// is included in the operation's peak and must be retained by its consumer.
    ///
    /// # Errors
    /// Refuses work, storage or reservation without returning a partial order.
    /// Existing atoms remain unchanged; no scratch is retained.
    pub fn ordered_ids_with<E>(
        &mut self,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Vec<usize>, Failure<E>> {
        let count = self.committed.len();
        self.appender().ordered_prefix_with(count, limits, before)
    }

    /// Publish the pending discovery suffix, preserving every local position.
    /// Borrowing prevents this while a scan or appender is retained. Each ID move
    /// and snapshot-directory operation is pre-admitted before publication.
    /// Sealed canonical payload is shared, never recopied. An empty discovery
    /// prefix takes the pending ID vector by swap; snapshot publication has its
    /// own allocation and capacity checks.
    ///
    /// # Errors
    /// On refusal no membership or prefix changes. A successful capacity growth
    /// may remain. Bounds cover old/new committed buffers and the live tail.
    pub fn commit_with<E>(
        &mut self,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        let mut checked = || before().map_err(Failure::Stopped);
        let live = self.storage_bytes();
        admit(live, limits)?;
        population(self.len(), limits)?;
        let additional = self.pending.len();
        if additional == 0 && !self.store.has_unpublished() {
            return Ok(());
        }
        if !self.committed.is_empty() {
            reserve(
                &mut self.committed,
                additional,
                limits.max_atoms,
                live,
                &mut self.peak,
                limits,
                &mut checked,
            )?;
        }
        for _ in &self.pending {
            checked()?;
        }
        for _ in 0..self
            .store
            .publication_steps(&mut self.snapshot)
            .map_err(Failure::Catalog)?
        {
            checked()?;
        }
        let metadata = self.storage_bytes() - self.store.current_bytes();
        // Publication's `extra` already includes discovery/index metadata in
        // every reservation envelope, so its ceiling is the total owner bound.
        self.store
            .ceiling(usize::try_from(limits.max_bytes).unwrap_or(usize::MAX))
            .map_err(|error| store_failure(error, metadata, limits))?;
        self.store.restart_peak();
        let publication = self.store.publish_into(&mut self.snapshot, metadata);
        self.peak = self.peak.max(self.store.peak_bytes());
        publication.map_err(|error| store_failure(error, 0, limits))?;
        if self.committed.is_empty() {
            std::mem::swap(&mut self.committed, &mut self.pending);
        } else {
            self.committed.append(&mut self.pending);
        }
        Ok(())
    }

    /// Commit and transfer the discovery map into an immutable catalog.
    /// The catalog retains its canonical prefix without exporting atom payloads.
    /// Writer indexes and scratch drop; original insertion order survives.
    ///
    /// # Errors
    /// A commit refusal consumes this builder and drops its owned data. Use
    /// [`Self::commit_with`] first when failure recovery needs the builder.
    pub fn into_catalog_with<E>(
        mut self,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomCatalog, Failure<E>> {
        self.commit_with(limits, &mut before)?;
        self.admit_publication(limits, &mut before)?;
        Ok(AtomCatalog::published(self.snapshot, self.committed))
    }

    /// Publish selected discovery positions in the caller's supplied order.
    /// Duplicate occurrences remain duplicates. The returned catalog shares the
    /// committed canonical prefix and owns only its selected identity map;
    /// this writer remains reusable. Publication does not assert truth or order.
    /// Its live overlap with the writer is included in the operation's budget.
    ///
    /// # Errors
    /// Refuses work, storage or a position outside the committed discovery map.
    /// No partial selection is published. A completed commit may remain visible
    /// even if the later selection fails; identity publication is not truth.
    pub fn publish_selection_with<E>(
        &mut self,
        positions: &[usize],
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomCatalog, Failure<E>> {
        self.commit_with(limits, &mut before)?;
        self.publish(positions, 0, limits, before)
    }

    /// Consume the writer into a catalog ordered by complete typed identity.
    /// A merge sort with the typed atom comparison supplies order; no term,
    /// atom or predicate payload is copied. Temporary order and output ID maps are both charged
    /// while live, then the writer indexes and discovery metadata are dropped.
    ///
    /// # Errors
    /// Returns any commit, ordering, capacity or caller refusal. As this operation
    /// consumes the writer, use the retaining publication API for retry semantics.
    pub fn into_ordered_catalog_with<E>(
        mut self,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomCatalog, Failure<E>> {
        self.commit_with(limits, &mut before)?;
        let order = self.ordered_ids_with(limits, &mut before)?;
        let order_bytes = size_of::<Vec<usize>>() as u128 + cells::<usize>(order.capacity());
        self.publish(&order, order_bytes, limits, before)
    }

    fn publish<E>(
        &mut self,
        positions: &[usize],
        external: u128,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomCatalog, Failure<E>> {
        let mut ids = Vec::new();
        let base = self
            .storage_bytes()
            .checked_add(external)
            .and_then(|bytes| bytes.checked_add(size_of::<super::CatalogData>() as u128))
            .ok_or(Failure::Overflow)?;
        let mut checked = || before().map_err(Failure::Stopped);
        admit(base, limits)?;
        reserve(
            &mut ids,
            positions.len(),
            positions.len(),
            base,
            &mut self.peak,
            limits,
            &mut checked,
        )?;
        for &position in positions {
            checked()?;
            let id = *self
                .committed
                .get(position)
                .ok_or(Failure::Catalog(super::Error::Shape))?;
            // The catalog caches a portable encoding measure. Admit the
            // following row/argument reads before copying its identity.
            let atom =
                AtomRef::new(&self.snapshot, id).expect("committed identity is in its snapshot");
            for _ in 0..atom.predicate().arity() {
                checked()?;
            }
            checked()?;
            ids.push(id);
        }
        checked()?;
        Ok(AtomCatalog::published(self.snapshot.clone(), ids))
    }

    /// Admit the final measure traversal and output header while the retired
    /// writer and its indexes remain live. No allocation or publication occurs.
    fn admit_publication<E>(
        &self,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        // The final catalog caches its portable per-occurrence encoded measure.
        // Admit the measure's argument visits before transferring its sole ID map.
        for id in 0..self.committed.len() {
            before().map_err(Failure::Stopped)?;
            let atom = self.get(id).expect("committed occurrence");
            for _ in 0..atom.predicate().arity() {
                before().map_err(Failure::Stopped)?;
            }
        }
        let publication = self.storage_bytes() + size_of::<super::CatalogData>() as u128;
        admit(publication, limits)?;
        before().map_err(Failure::Stopped)?;
        Ok(())
    }
}

/// Immutable committed prefix, independent of the disjoint mutable append tail.
#[derive(Clone, Copy)]
pub struct CommittedAtoms<'a> {
    atoms: Atoms<'a>,
}
impl<'a> CommittedAtoms<'a> {
    /// Canonical resolver for exactly this committed prefix. Declarations and
    /// rows in the append tail remain inaccessible until the next commit.
    #[must_use]
    pub const fn read(self) -> CatalogRead<'a> {
        CatalogRead(self.atoms.snapshot)
    }
    /// Prefix length; later pending identities are excluded.
    #[must_use]
    pub fn len(self) -> usize {
        self.atoms.len()
    }
    /// Whether the prefix has no identities.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.atoms.is_empty()
    }
    /// Borrow an admitted prefix position, or return absence outside that prefix.
    #[must_use]
    pub fn get(self, id: usize) -> Option<AtomRef<'a>> {
        self.atoms.at(id)
    }
    /// Borrow the exact committed dense sequence without copying.
    #[must_use]
    pub const fn atoms(self) -> Atoms<'a> {
        self.atoms
    }
}

/// Translate a store-local byte refusal into its complete named owner scope.
/// Publication already includes its external metadata and passes zero here.
fn store_failure<E>(error: storage::Fault, extra: u128, limits: Limits) -> Failure<E> {
    if let storage::Fault::Storage { required, .. } = &error {
        let Some(required) = required.checked_add(extra) else {
            return Failure::Overflow;
        };
        if required > limits.max_bytes {
            return Failure::Bytes {
                required,
                limit: limits.max_bytes,
            };
        }
        // A usize store ceiling may bind below a u128 owner allowance. Retain
        // that explicit representation limit instead of reporting required <= limit.
    }
    Failure::Catalog(error)
}

/// One immutable search implementation for a complete writer and its append capability.
struct Lookup<'a> {
    store: &'a Store,
    committed: &'a [AtomId],
    pending: &'a [AtomId],
    discovery: &'a Discovery,
    bytes: u128,
}
impl Lookup<'_> {
    fn admit<E>(&self, extra: u128, limits: Limits) -> Result<(), Failure<E>> {
        population(self.committed.len() + self.pending.len(), limits)?;
        admit(self.bytes + extra, limits)
    }

    fn find<E>(
        &self,
        query: Query<'_>,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<usize>, Failure<E>> {
        self.admit(query.extra_bytes(), limits)?;
        self.find_admitted(query, before)
    }

    /// The same immutable owner's population and query envelope have passed
    /// admission. No capacity or identity changes between that check and search.
    fn find_admitted<E>(
        &self,
        query: Query<'_>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<usize>, Failure<E>> {
        let present = match query.identity_with(self.store, &mut before)? {
            Identity::Local(atom) => atom.present(),
            Identity::Foreign => query.resolve_with(self.store, &mut before)?,
        };
        match present {
            Some(atom) => self
                .discovery
                .find(self.committed, self.pending, atom, &mut before)
                .map_err(Failure::Stopped),
            None => Ok(None),
        }
    }
}

/// Scoped append capability that cannot mutate committed canonical payloads.
pub struct AtomAppender<'a> {
    store: &'a mut Store,
    snapshot_bytes: u128,
    committed: &'a [AtomId],
    committed_capacity: usize,
    pending: &'a mut Vec<AtomId>,
    peak: &'a mut u128,
    discovery: &'a mut Discovery,
}
impl AtomAppender<'_> {
    fn lookup(&self) -> Lookup<'_> {
        Lookup {
            store: self.store,
            committed: self.committed,
            pending: self.pending,
            discovery: self.discovery,
            bytes: self.storage_bytes(),
        }
    }

    /// Search the committed prefix and pending tail without mutation or allocation.
    /// Uses the same checked identity probes as the complete interner.
    ///
    /// # Errors
    /// Refuses population, storage, or caller work before reporting membership.
    pub fn find_key_with<E>(
        &self,
        key: AtomKey<'_>,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<usize>, Failure<E>> {
        self.lookup().find(Query::Key(key), limits, before)
    }

    /// Find an existing opposite-sign tuple without importing any component.
    /// The returned position belongs to this exact discovery population.
    ///
    /// # Errors
    /// Refuses the population, storage or caller-work limit before returning a
    /// position or absence. No predicate, term, atom or membership is changed.
    pub fn find_signed_atom_with<E>(
        &self,
        atom: AtomRef<'_>,
        sign: crate::Sign,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<usize>, Failure<E>> {
        self.lookup()
            .find(Query::SignedAtom(atom, sign), limits, before)
    }

    /// Borrow the existing term indexes without importing or discovering values.
    /// The append authority cannot mutate while this read capability is live.
    /// The caller owns the temporary lookup header and its scratch allowance.
    #[must_use]
    pub fn term_lookup(&self) -> TermLookup<'_> {
        TermLookup::new(self.store, self.len(), self.storage_bytes())
    }

    /// Borrow canonical identities already available to this append authority.
    #[must_use]
    pub fn read(&self) -> CatalogRead<'_> {
        CatalogRead(storage::Read::from(&*self.store))
    }

    /// Declare a canonical signed predicate without adding a discovery row.
    ///
    /// # Errors
    /// Returns the first population, capacity or caller refusal. Previously
    /// committed rows remain unchanged, including after partial component import.
    pub fn declare_predicate_with<'predicate, E>(
        &mut self,
        predicate: impl Into<PredicateRef<'predicate>>,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<DeclaredPredicate, Failure<E>> {
        population(self.len(), limits)?;
        admit(self.storage_bytes(), limits)?;
        before().map_err(Failure::Stopped)?;
        let predicate = self.import(limits, |store| {
            store.import_predicate_with(predicate.into(), &mut before)
        })?;
        Ok(DeclaredPredicate::new(
            storage::Read::from(&*self.store),
            predicate,
        ))
    }

    /// Declare a function or tuple shape without creating a term or atom.
    /// Function names share the vocabulary's canonical text arena. Tuple shapes
    /// have no name or sign, as in [`crate::ValueNodeRef`].
    ///
    /// # Errors
    /// Refuses non-constructor descriptors, empty function names, owner storage
    /// or caller work. Complete text imports may remain after a later refusal;
    /// no discovery or support occurrence is added.
    pub fn declare_constructor_with<E>(
        &mut self,
        descriptor: crate::ValueNodeRef<'_>,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<DeclaredConstructor, Failure<E>> {
        population(self.len(), limits)?;
        admit(self.storage_bytes(), limits)?;
        before().map_err(Failure::Stopped)?;
        let data = self.import(limits, |store| {
            store.declare_constructor_with(descriptor, &mut before)
        })?;
        Ok(DeclaredConstructor::new(
            storage::Read::from(&*self.store),
            data,
        ))
    }

    /// Bound a canonical import by the complete owner's remaining capacity and
    /// retain its temporary peak even when the operation stops before publication.
    fn import<R, E>(
        &mut self,
        limits: Limits,
        operation: impl FnOnce(&mut Store) -> Result<R, storage::Failure<E>>,
    ) -> Result<R, Failure<E>> {
        self.import_with_extra(limits, 0, operation)
    }

    fn import_with_extra<R, E>(
        &mut self,
        limits: Limits,
        extra: u128,
        operation: impl FnOnce(&mut Store) -> Result<R, storage::Failure<E>>,
    ) -> Result<R, Failure<E>> {
        let metadata = (self.storage_bytes() - self.store.current_bytes())
            .checked_add(extra)
            .ok_or(Failure::Overflow)?;
        admit(self.store.current_bytes() + metadata, limits)?;
        let available = limits
            .max_bytes
            .checked_sub(metadata)
            .ok_or(Failure::Overflow)?;
        self.store
            .ceiling(usize::try_from(available).unwrap_or(usize::MAX))
            .map_err(|error| store_failure(error, metadata, limits))?;
        self.store.restart_peak();
        let receipt = ImportPeak {
            store: self.store,
            peak: &mut *self.peak,
            metadata,
        };
        let imported = operation(&mut *receipt.store);
        drop(receipt);
        imported.map_err(|failure| match failure {
            storage::Failure::Storage(error) => store_failure(error, metadata, limits),
            storage::Failure::Stopped(error) => Failure::Stopped(error),
        })
    }
    /// Distinct committed plus pending count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.committed.len() + self.pending.len()
    }
    /// Whether no identities exist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Borrow one local committed or pending identity.
    #[must_use]
    pub fn get(&self, id: usize) -> Option<AtomRef<'_>> {
        get(self.store, self.committed, self.pending, id)
    }
    /// Current named capacity, including prior insertion/order scratch.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        self.store.current_bytes()
            + self.snapshot_bytes
            + storage(
                self.committed_capacity,
                self.pending.capacity(),
                self.discovery.bytes(),
            )
    }
    /// Peak named canonical and discovery capacity, including retained payload.
    #[must_use]
    pub fn storage_peak_bytes(&self) -> u128 {
        (*self.peak).max(self.storage_bytes())
    }

    /// Restart this append operation's peak at its actual retained capacity.
    pub fn restart_storage_peak(&mut self) {
        self.store.restart_peak();
        *self.peak = self.storage_bytes();
    }

    fn reborrow(&mut self) -> AtomAppender<'_> {
        AtomAppender {
            store: self.store,
            snapshot_bytes: self.snapshot_bytes,
            committed: self.committed,
            committed_capacity: self.committed_capacity,
            pending: self.pending,
            peak: self.peak,
            discovery: self.discovery,
        }
    }

    /// Search without copying an owned atom. Lookup changes no retained
    /// scratch; a vacant entry holds no placement, only the exclusive writer.
    ///
    /// # Errors
    /// Refusal changes no membership or retained capacity.
    pub fn entry_atom_with<'owner, 'key, E>(
        &'owner mut self,
        atom: impl Into<AtomRef<'key>>,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomEntry<'owner, 'key>, Failure<E>> {
        self.reborrow()
            .entry(Query::Atom(atom.into()), limits, before)
    }
    /// Search a checked borrowed substitution without materialization.
    ///
    /// # Errors
    /// Same failure contract as [`Self::entry_atom_with`].
    pub fn entry_key_with<'owner, 'key, E>(
        &'owner mut self,
        key: AtomKey<'key>,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomEntry<'owner, 'key>, Failure<E>> {
        self.reborrow().entry(Query::Key(key), limits, before)
    }
}
impl<'a> AtomAppender<'a> {
    /// Admit the owned entry scratch before validation or query callbacks. Its
    /// fixed projected header, when supplied, excludes borrowed caller arrays.
    fn admit_entry<E>(&mut self, extra: u128, limits: Limits) -> Result<(), Failure<E>> {
        population(self.len(), limits)?;
        let live = self.storage_bytes() + extra + PREPARED_BYTES;
        admit(live, limits)?;
        *self.peak = (*self.peak).max(live);
        Ok(())
    }

    fn entry<'key, E>(
        mut self,
        query: Query<'key>,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomEntry<'a, 'key>, Failure<E>> {
        self.admit_entry(query.extra_bytes(), limits)?;
        let prepared = match query.identity_with(self.store, &mut before)? {
            Identity::Foreign => None,
            Identity::Local(prepared) => Some(prepared),
        };
        self.entry_prepared(query, prepared, before)
    }

    /// The caller has admitted this query and prepared-result slot against the
    /// current population/storage limits. Only the same immutable query and
    /// exclusive writer may consume the result. A prepared local identity is
    /// found through the discovery inverse; foreign coordinates resolve by
    /// content. Neither path mutates the Store atom index or retained scratch.
    fn entry_prepared<'key, E>(
        self,
        query: Query<'key>,
        prepared: Option<storage::PreparedAtom>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomEntry<'a, 'key>, Failure<E>> {
        let present = match &prepared {
            Some(prepared) => prepared.present(),
            None => query.resolve_with(self.store, &mut before)?,
        };
        let position = match present {
            Some(atom) => self
                .discovery
                .find(self.committed, self.pending, atom, &mut before)
                .map_err(Failure::Stopped)?,
            None => None,
        };
        Ok(AtomEntry {
            appender: self,
            query,
            prepared,
            position: position.map_or(EntryPosition::Vacant, EntryPosition::Occupied),
        })
    }
}

/// One checked lookup, holding the exclusive append path until insertion/drop.
/// The key is borrowed. Occupied lookup never copies payload or mutation scratch.
/// Its owned prepared-result slot counts in named storage even when occupied;
/// borrowed caller frames remain excluded. The result cannot escape this
/// exclusive append capability or be reused after insertion/refusal.
pub struct AtomEntry<'owner, 'key> {
    appender: AtomAppender<'owner>,
    query: Query<'key>,
    prepared: Option<storage::PreparedAtom>,
    position: EntryPosition,
}

#[derive(Clone, Copy)]
enum EntryPosition {
    Occupied(usize),
    Vacant,
}
impl<'owner> AtomEntry<'owner, '_> {
    /// Import identity before publishing discovery. A refusal may
    /// retain complete canonical components, but never a discovery position.
    fn intern<E>(
        &mut self,
        limits: Limits,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<AtomId, Failure<E>> {
        let logical = TermLimits {
            max_nodes: usize::MAX,
            max_depth: usize::MAX,
            max_bytes: usize::MAX,
        };
        let extra = self.extra_bytes();
        let prepared = self.prepared.take();
        self.appender.import_with_extra(limits, extra, |store| {
            if let Some(prepared) = prepared {
                self.query.publish_with(store, prepared, before)
            } else {
                self.query.intern_with(store, logical, before)
            }
        })
    }

    fn extra_bytes(&self) -> u128 {
        self.query.extra_bytes() + PREPARED_BYTES
    }

    /// Existing local position, or a vacant insertion capability.
    #[must_use]
    pub const fn position(&self) -> Option<usize> {
        match self.position {
            EntryPosition::Occupied(position) => Some(position),
            EntryPosition::Vacant => None,
        }
    }
    /// Current named storage, including the entry's owned slots.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        self.appender.storage_bytes() + self.extra_bytes()
    }
    /// Current peak including the entry's owned slots.
    #[must_use]
    pub fn storage_peak_bytes(&self) -> u128 {
        self.appender.storage_peak_bytes().max(self.storage_bytes())
    }

    /// Return an existing discovery position or publish a new one.
    ///
    /// Import canonical identity before publishing discovery.
    /// Same-authority terms reuse their IDs; ingress or foreign terms use the
    /// checked typed importer. Each visited descriptor, text byte, comparison,
    /// reservation and publication operation is admitted before its work. Named
    /// canonical payload is part of `max_bytes`; caller-owned ingress descriptions
    /// remain outside this owner. Arc envelope allocation is infallible on stable
    /// Rust. Planned metadata writes publish as one indivisible update.
    ///
    /// # Errors
    /// Refusal publishes no discovery position. Complete
    /// canonical components and reserved capacity may remain after a later
    /// refusal; they assert neither discovery nor truth and count toward storage.
    pub fn insert_with<E>(
        mut self,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, Failure<E>> {
        self.insert(limits, before)
    }

    /// Publish discovery and borrow its canonical atom without exporting it.
    /// The returned read excludes further append until it is released. This is
    /// the same insertion as [`Self::insert_with`], followed by one pre-admitted
    /// discovery-map read.
    ///
    /// # Errors
    /// Returns the insertion refusal or the initial read-admission refusal.
    /// An internal missing published identity is a typed shape error.
    pub fn insert_ref_with<E>(
        mut self,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomRef<'owner>, Failure<E>> {
        before().map_err(Failure::Stopped)?;
        let id = self.insert(limits, before)?;
        get(
            self.appender.store,
            self.appender.committed,
            self.appender.pending,
            id,
        )
        .ok_or(Failure::Catalog(super::Error::Shape))
    }

    fn insert<E>(
        &mut self,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, Failure<E>> {
        let mut checked = || before().map_err(Failure::Stopped);
        population(self.appender.len(), limits)?;
        admit(self.storage_bytes(), limits)?;
        if let EntryPosition::Occupied(id) = self.position {
            return Ok(id);
        }
        let id = self.appender.len();
        let required = id.checked_add(1).ok_or(Failure::Overflow)?;
        population(required, limits)?;
        let live = self.storage_bytes();
        reserve(
            self.appender.pending,
            1,
            limits.max_atoms,
            live,
            self.appender.peak,
            limits,
            &mut checked,
        )?;
        checked()?;
        let atom = self.intern(limits, &mut before)?;
        let extra = self.extra_bytes();
        self.appender
            .reserve_discovery(extra, limits, &mut before)?;
        self.publish(atom, id, &mut before)?;
        Ok(id)
    }

    /// Discovery and its canonical identity become visible together, after
    /// both writes have permits. No caller code or allocation runs between them.
    fn publish<E>(
        &mut self,
        atom: AtomId,
        id: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        before().map_err(Failure::Stopped)?; // Discovery-to-canonical-identity write.
        before().map_err(Failure::Stopped)?; // Discovery position publication.
        self.appender
            .discovery
            .insert(self.appender.committed, self.appender.pending, atom, id);
        self.appender.pending.push(atom);
        Ok(())
    }
}

/// Preserve completed canonical reservations even when a caller catches an
/// unwind from its work callback. The borrowed projection remains live while
/// this receipt combines the canonical peak with discovery and query storage.
struct ImportPeak<'a> {
    store: &'a mut Store,
    peak: &'a mut u128,
    metadata: u128,
}

impl Drop for ImportPeak<'_> {
    fn drop(&mut self) {
        *self.peak = (*self.peak).max(self.store.peak_bytes() + self.metadata);
    }
}

fn get<'a>(
    store: &'a Store,
    committed: &[AtomId],
    pending: &[AtomId],
    id: usize,
) -> Option<AtomRef<'a>> {
    AtomRef::new(store, discovery::identity(committed, pending, id)?)
}

fn storage(committed: usize, pending: usize, discovery: u128) -> u128 {
    (size_of::<AtomInterner>() - size_of::<Store>()) as u128
        + cells::<AtomId>(committed)
        + cells::<AtomId>(pending)
        + discovery
}
fn cells<T>(count: usize) -> u128 {
    count as u128 * size_of::<T>() as u128
}
fn admit<E>(required: u128, limits: Limits) -> Result<(), Failure<E>> {
    MeterBytes::check(required, limits.max_bytes).map_err(|_| Failure::Bytes {
        required,
        limit: limits.max_bytes,
    })
}
fn population<E>(required: usize, limits: Limits) -> Result<(), Failure<E>> {
    Meter::check_count(required, 0, limits.max_atoms)
        .map(|_| ())
        .map_err(|_| Failure::Atoms {
            required,
            limit: limits.max_atoms,
        })
}

/// The capacity [`reserve`] reaches from `capacity` growing one cell at a
/// time until it holds `required` cells.
fn grown(mut capacity: usize, required: usize, maximum: usize) -> usize {
    while capacity < required {
        capacity = capacity
            .checked_mul(2)
            .unwrap_or(maximum)
            .max(4)
            .min(maximum)
            .max(capacity + 1);
    }
    capacity
}

fn reserve<T, E>(
    values: &mut Vec<T>,
    additional: usize,
    maximum: usize,
    live: u128,
    peak: &mut u128,
    limits: Limits,
    before: &mut impl FnMut() -> Result<(), Failure<E>>,
) -> Result<(), Failure<E>> {
    admit(live, limits)?;
    *peak = (*peak).max(live);
    let required = values
        .len()
        .checked_add(additional)
        .ok_or(Failure::Overflow)?;
    if required <= values.capacity() {
        return Ok(());
    }
    // One growth to the capacity growing one cell at a time would reach.
    let capacity = grown(values.capacity(), required, maximum);
    let envelope = live + cells::<T>(capacity);
    admit(envelope, limits)?;
    before()?;
    // Admit possible relocation of every live cell before requesting growth.
    // This remains a conservative charge when an allocator grows in place.
    for _ in values.iter() {
        before()?;
    }
    let old = values.capacity();
    values
        .try_reserve_exact(capacity - values.len())
        .map_err(Failure::Allocation)?;
    let actual = live + cells::<T>(values.capacity());
    *peak = (*peak).max(actual);
    // `live` already includes the old buffer. Keeping it in this envelope is
    // conservative even when the allocator grew that buffer in place.
    admit(actual, limits)?;
    debug_assert!(values.capacity() >= old);
    Ok(())
}

#[cfg(test)]
mod tests;
