//! Consuming exact storage closure and fresh discovery over its immutable base.

use std::fmt;

use super::{
    AtomAppender, AtomEntry, AtomInterner, Directions, EntryPosition, Failure, InsertionPath,
    Limits, PREPARED_BYTES, Query, admit, population, storage, store_failure,
};
use crate::Model;
use crate::catalog::{AtomCatalog, AtomRef, CatalogRead, Error, ReadError};

/// Immutable canonical vocabulary, rows and exact indexes. Clone shares their
/// allocations. Storage presence establishes neither discovery nor truth.
/// Descendants share this base but receive distinct atom scopes; no descendant
/// can be closed into another base. Detached model snapshots retain payload only.
#[derive(Clone, Debug)]
pub struct ClosedCatalog {
    storage: storage::Closed,
}

/// A consuming close refused, preserving its typed cause and actual named peak.
/// The builder is not returned. Previously published snapshots remain valid.
/// Callback panic has the same snapshot guarantee but returns no receipt.
#[derive(Debug)]
pub struct CloseFailure<E> {
    failure: Failure<E>,
    peak_bytes: u128,
}

impl<E> CloseFailure<E> {
    /// Original storage, population, allocation or caller refusal.
    #[must_use]
    pub const fn failure(&self) -> &Failure<E> {
        &self.failure
    }
    /// Actual named capacity and successful replacement overlap during this
    /// attempt, excluding rejected proposals, prior history and external owners.
    #[must_use]
    pub const fn peak_bytes(&self) -> u128 {
        self.peak_bytes
    }
    /// Recover the original typed refusal together with its attempt receipt.
    #[must_use]
    pub fn into_parts(self) -> (Failure<E>, u128) {
        (self.failure, self.peak_bytes)
    }
}

impl<E: fmt::Display> fmt::Display for CloseFailure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.failure.fmt(f)
    }
}
impl<E: std::error::Error + 'static> std::error::Error for CloseFailure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.failure)
    }
}

impl ClosedCatalog {
    /// Named retained header, immutable payload, directories and exact indexes.
    /// Arc counters and allocator bookkeeping are excluded; this is not RSS.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        size_of::<Self>() as u128 + self.storage.retained_bytes()
    }
    /// Successful close-attempt peak, including the old discovery/snapshot
    /// metadata through publication. Earlier import peaks remain separate.
    #[must_use]
    pub fn publication_peak_bytes(&self) -> u128 {
        self.storage.publication_peak()
    }
    /// Borrow the closed text, term and predicate prefix. This vocabulary read
    /// has no atom authority and cannot translate a raw atom coordinate.
    #[must_use]
    pub fn vocabulary_read(&self) -> CatalogRead<'_> {
        CatalogRead(self.storage.vocabulary().into())
    }
    /// Borrow the original writer's read at its close: its atom and vocabulary
    /// scopes and every canonical row at its coordinate, including rows never
    /// published. That writer's memberships bind to it; another writer's are
    /// refused. Like the writer's own read, it grants no extensional truth.
    #[must_use]
    pub fn read(&self) -> CatalogRead<'_> {
        CatalogRead(storage::Read::from(&self.storage))
    }
    /// Additional named metadata retained by one earlier publication of the
    /// original writer: its catalog/map and independent prefix directories.
    /// Shared payload and indexes are excluded. Authenticate in constant time
    /// using the original atom scope, vocabulary scope and exact prefix extents.
    /// Callers deduplicate catalog clones with `same_owner` and snapshot metadata
    /// with `shares_snapshot`; this method does not change either relation.
    ///
    /// # Errors
    /// Refuses foreign, descendant or unavailable prefixes. Logical equality or
    /// a shared vocabulary alone cannot authorize a payload-sharing deduction.
    pub fn prior_publication_metadata_bytes(&self, prior: &AtomCatalog) -> Result<u128, ReadError> {
        self.storage
            .prior_metadata(&prior.0.snapshot)
            .map(|snapshot| prior.publication_bytes() + snapshot)
    }
}

impl AtomInterner {
    /// Consume the original writer into immutable indexed storage. All canonical
    /// rows survive, including rows never published into discovery. Discovery
    /// maps and order indexes are dropped, not committed or recopied solely to
    /// discard them. Work visits segment/column metadata, never typed payload.
    ///
    /// Current snapshot/discovery capacities remain charged through publication.
    /// Fixed Arc/Box envelopes use stable Rust's infallible allocation boundary;
    /// directory reserves are fallible. External catalogs/caller frames remain
    /// outside this owner allowance. Population still means discovered atoms.
    ///
    /// # Errors
    /// Returns a typed cause and actual close-attempt peak. Refusal consumes this
    /// builder; earlier snapshots remain valid. Closing a descendant is refused.
    pub fn into_closed_with<E>(
        mut self,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<ClosedCatalog, CloseFailure<E>> {
        let peak_bytes = self.storage_bytes();
        let metadata = peak_bytes - self.store.current_bytes();
        let admitted = admit(peak_bytes, limits)
            .and_then(|()| population(self.len(), limits))
            .and_then(|()| {
                self.store
                    .ceiling(usize::try_from(limits.max_bytes).unwrap_or(usize::MAX))
                    .map_err(|error| store_failure(error, metadata, limits))
            });
        admitted.map_err(|failure| CloseFailure {
            failure,
            peak_bytes,
        })?;
        let Self {
            store,
            snapshot,
            committed,
            pending,
            index,
            discovery,
            spines,
            subtrees,
        } = self;
        let retained = (
            snapshot, committed, pending, index, discovery, spines, subtrees,
        );
        let result = store
            .close_with(metadata, before)
            .map(|storage| ClosedCatalog { storage })
            .map_err(|storage::CloseError { failure, peak }| CloseFailure {
                failure: match failure {
                    storage::Failure::Storage(error) => store_failure(error, 0, limits),
                    storage::Failure::Stopped(error) => Failure::Stopped(error),
                },
                peak_bytes: peak,
            });
        drop(retained);
        result
    }

    /// Start empty discovery over one immutable base without copying payload or
    /// exact indexes. The initial canonical prefix exposes all base rows under
    /// a fresh atom scope; their presence does not count toward discovery limits.
    /// No new vocabulary is admitted. Fixed Arc envelopes follow `new`.
    ///
    /// # Errors
    /// Refuses an initial named footprint beyond the inclusive byte allowance.
    pub fn for_closed_catalog(base: &ClosedCatalog, max_bytes: usize) -> Result<Self, Error> {
        let owner = Self::from_store(storage::Store::with_closed(&base.storage, max_bytes)?);
        let required = owner.storage_bytes();
        if required > max_bytes as u128 {
            return Err(Error::Storage {
                required,
                limit: max_bytes,
            });
        }
        Ok(owner)
    }

    /// Discover exactly a model's selected atoms in an empty descendant of its
    /// original closed catalog. Authenticate both the supplied base allocation
    /// and the model's original writer scope and prefix before copying any IDs.
    /// Descendant, sibling and merely equal catalogs cannot supply this seed.
    /// The model's semantic order and uniqueness justify each new predicate at
    /// the end of the relation directory and each row beyond its predicate's
    /// last row. Canonical IDs themselves are not treated as semantic order.
    ///
    /// Reuses the ordinary discovery map, AVL insertion and storage receipts;
    /// no tuple lookup, term import or additional index is needed. For n selected
    /// atoms, work is O(n log n) bounded index operations plus immutable segment
    /// lookups, and retained discovery space is O(n), excluding the shared base
    /// and caller's model.
    /// The committed prefix stays empty until an explicit commit. Discovery
    /// records the supplied selection; it does not establish model membership.
    ///
    /// # Errors
    /// Nonempty discovery or an unauthenticated base/model returns a catalog
    /// shape refusal before discovery changes. A later resource or caller
    /// refusal retains the completed selected prefix and any reserved capacity;
    /// no partial row or AVL update is published. Ordinary entries can continue
    /// from that prefix, but this operation requires a fresh empty descendant.
    ///
    /// # Panics
    /// Panics if the immutable model iterator violates its exact length.
    pub fn discover_model_with<E>(
        &mut self,
        base: &ClosedCatalog,
        model: &Model,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        population(self.len(), limits)?;
        admit(self.storage_bytes(), limits)?;
        before().map_err(Failure::Stopped)?;
        if !self.is_empty()
            || !self.store.shares_closed_base(&base.storage)
            || base
                .storage
                .prior_metadata(&model.catalog().0.snapshot)
                .is_err()
        {
            return Err(Failure::Catalog(Error::Shape));
        }
        let mut atoms = model.atoms().iter();
        for _ in 0..atoms.len() {
            before().map_err(Failure::Stopped)?;
            let atom = atoms.next().expect("immutable model selection length");
            self.appender()
                .discover_ordered_atom(atom, limits, &mut before)?;
        }
        Ok(())
    }

    /// Shared closed allocations already counted by `storage_bytes`, excluding
    /// the separately retained `ClosedCatalog` inline handle. Zero for an ordinary
    /// or vocabulary-only writer. A ledger that owns this writer and its actual
    /// supplied base subtracts this subtotal once, never based on term equality.
    #[must_use]
    pub fn shared_closed_catalog_bytes(&self) -> u128 {
        self.store.shared_closed_bytes()
    }
}

impl AtomAppender<'_> {
    /// The caller authenticated the whole selection and started empty. The
    /// current discovery is exactly its completed, strictly ordered prefix.
    /// Hence this row is absent and follows all earlier rows of its predicate.
    fn discover_ordered_atom<E>(
        mut self,
        atom: AtomRef<'_>,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        self.admit_entry(0, limits)?;
        before().map_err(Failure::Stopped)?;
        let (_, identity) = atom.canonical().expect("authenticated original selection");
        let predicate = atom
            .predicate()
            .canonical()
            .expect("authenticated original predicate")
            .1;
        let relation = match self.subtrees.last() {
            Some(last) => {
                before().map_err(Failure::Stopped)?;
                let previous = AtomRef::new(&*self.store, last.representative)
                    .expect("published relation representative")
                    .predicate()
                    .canonical()
                    .expect("canonical relation predicate")
                    .1;
                if predicate == previous {
                    Ok(self.subtrees.len() - 1)
                } else {
                    Err(self.subtrees.len())
                }
            }
            None => Err(0),
        };
        let directions = Directions::default();
        let (root, route) = relation.map_or((None, InsertionPath::Recorded(&directions)), |at| {
            let subtree = &self.subtrees[at];
            (subtree.root, InsertionPath::BeyondLast(subtree.last))
        });
        self.prepare_path(root, route, PREPARED_BYTES, limits, &mut || {
            before().map_err(Failure::Stopped)
        })?;
        AtomEntry {
            appender: self,
            query: Query::Atom(atom),
            prepared: Some(storage::PreparedAtom::existing(identity)),
            position: EntryPosition::Vacant {
                relation,
                beyond: relation.is_ok(),
            },
        }
        .insert_with(limits, before)?;
        Ok(())
    }
}
