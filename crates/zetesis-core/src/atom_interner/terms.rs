//! Scoped generated-term admission through the existing canonical authority.

use super::query::{Identity, Projected, ProjectionSource};
use super::{
    AtomAppender, AtomInterner, CatalogRead, DeclaredPredicate, Failure, Limits, Query, TermLimits,
    admit, fmt, population, storage, store_failure,
};
use crate::catalog::{
    AssignmentError, AssignmentFailure, AssignmentSlice, ReadError, TermKey, TermRef,
};
use crate::{PatternRef, TemplateTerm, ValueNodeRef};

/// Assigned construction preserves frame errors and the interner's typed failures.
#[derive(Debug)]
pub enum AssignedFailure<E> {
    /// An invalid slot, unbound variable, vocabulary or read prefix.
    Assignment(AssignmentError),
    /// Canonical/discovery storage or caller-work refusal.
    Interner(Failure<E>),
}
impl<E: fmt::Display> fmt::Display for AssignedFailure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Assignment(error) => error.fmt(f),
            Self::Interner(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for AssignedFailure<E> {}
impl<E> From<Failure<E>> for AssignedFailure<E> {
    fn from(error: Failure<E>) -> Self {
        Self::Interner(error)
    }
}
impl<E> From<AssignmentFailure<E>> for AssignedFailure<E> {
    fn from(error: AssignmentFailure<E>) -> Self {
        match error {
            AssignmentFailure::Assignment(error) => Self::Assignment(error),
            AssignmentFailure::Stopped(error) => Self::Interner(Failure::Stopped(error)),
        }
    }
}

/// Immutable indexed access to an admitted vocabulary. Each checker borrows the
/// same authority while keeping its own temporary ID scratch and peak receipt.
/// The capability owns no logical payload and cannot admit missing terms.
pub struct TermLookup<'a> {
    storage: storage::TermLookup<'a>,
    population: usize,
    bytes: u128,
    metadata: u128,
}

impl AtomInterner {
    /// Borrow existing term indexes without creating a second identity authority.
    /// The owner cannot append while this capability or its read views are live.
    #[must_use]
    pub fn term_lookup(&self) -> TermLookup<'_> {
        TermLookup {
            storage: storage::TermLookup::new(&self.store),
            population: self.len(),
            bytes: self.storage_bytes(),
            metadata: self.storage_bytes() - self.store.current_bytes(),
        }
    }
}

impl<'a> TermLookup<'a> {
    /// Borrow the complete immutable vocabulary and atom prefix.
    #[must_use]
    pub fn read(&self) -> CatalogRead<'a> {
        CatalogRead(self.storage.read())
    }
    /// Shared authority capacity, counted once; this capability retains no scratch.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        self.bytes
    }
    /// Authority plus observed temporary capacity/overlap since the last restart.
    /// This local receipt does not mutate the shared owner's storage accounting.
    #[must_use]
    pub fn storage_peak(&self) -> u128 {
        self.storage.peak_bytes() + self.metadata
    }
    /// Start a fresh local receipt at the current immutable authority capacity.
    pub fn restart_storage_peak(&mut self) {
        self.storage.restart_peak();
    }

    fn prepare<E>(&mut self, limits: Limits) -> Result<(), Failure<E>> {
        population(self.population, limits)?;
        admit(self.bytes, limits)?;
        let available = limits
            .max_bytes
            .checked_sub(self.metadata)
            .ok_or(Failure::Overflow)?;
        self.storage
            .ceiling(usize::try_from(available).unwrap_or(usize::MAX))
            .map_err(|error| store_failure(error, self.metadata, limits))
    }

    /// Resolve exact content through the existing hash indexes. Same-vocabulary
    /// terms need no scratch. Other terms use at most one expanded-node ID stack;
    /// its capacity and replacement overlap count against the owner allowance.
    /// Canonical foreign input uses bounded rank navigation: worst-case visited
    /// nodes times depth times log(maximum arity + 1), plus text hash/probe work.
    /// A missing component returns None without scanning the remaining input;
    /// this lookup is not a new admission certificate for that input.
    /// # Errors
    /// Refuses visited logical measures, capacity, or caller work. No identity,
    /// discovery position, shared index or shared capacity is mutated.
    pub fn find_term_with<E>(
        &mut self,
        term: TermRef<'_>,
        term_limits: TermLimits,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<TermKey>, Failure<E>> {
        self.prepare(limits)?;
        let id = self
            .storage
            .find_term_with(term, term_limits, &mut before)
            .map_err(|error| match error {
                storage::Failure::Storage(error) => store_failure(error, self.metadata, limits),
                storage::Failure::Stopped(error) => Failure::Stopped(error),
            })?;
        before().map_err(Failure::Stopped)?;
        Ok(id.map(|id| TermKey {
            scope: self.read().0.vocabulary_scope(),
            id,
        }))
    }

    /// Resolve a node whose children are assigned identities from this vocabulary.
    /// Reuses the writer's normalization, typed key, measures and exact probes.
    /// Temporary child IDs count in both the logical and owner byte allowances.
    /// # Errors
    /// Refuses invalid assignments, shape, visited bounds or caller work. Missing
    /// identity returns None and never extends the shared completed vocabulary.
    pub fn find_constructed_with<E>(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        values: AssignmentSlice<'_>,
        children: &[usize],
        term_limits: TermLimits,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<TermKey>, AssignedFailure<E>> {
        self.prepare(limits)?;
        values.validate_with(self.read(), children, &mut before)?;
        let id = self
            .storage
            .find_constructed_with(descriptor, values.slots, children, term_limits, &mut before)
            .map_err(|error| match error {
                storage::Failure::Storage(error) => store_failure(error, self.metadata, limits),
                storage::Failure::Stopped(error) => Failure::Stopped(error),
            })?;
        before().map_err(Failure::Stopped)?;
        Ok(id.map(|id| TermKey {
            scope: self.read().0.vocabulary_scope(),
            id,
        }))
    }
}

impl AtomAppender<'_> {
    /// Current owner capacity plus the fixed borrowed-pattern lookup header.
    /// Caller assignment/pattern arrays and other live owners are excluded.
    /// Lookup retains no entry, prepared-result slot, path or allocated scratch;
    /// this receipt does not mutate the owner's current or peak counters.
    #[must_use]
    pub fn pattern_lookup_bytes(&self) -> u128 {
        self.storage_bytes() + Projected::HEADER_BYTES
    }

    /// Find a discovered pattern instance using only its referenced slots.
    /// The predicate, constants and assignment witness must belong to this
    /// vocabulary; referenced identities must fit the current read prefix.
    /// Unreferenced cells are not inspected. Even a nullary or constant-only
    /// pattern authenticates the assignment's witness. This is distinct from
    /// whole-frame `AssignmentSlice::bind_with` and foreign `AtomKey` lookup.
    ///
    /// Argument order and repetitions are preserved without a temporary vector.
    /// Work depends on projected arity, exact tuple probes and the discovery
    /// inverse, not unrelated assignment width. This query applies no new logical
    /// term limits and neither imports nor publishes identity or discovery.
    /// A canonical row without a discovery position returns `None`.
    ///
    /// The envelope from [`Self::pattern_lookup_bytes`] is admitted before
    /// callbacks. Borrowed input is immutable for this operation only; no absence
    /// certificate can be retained across later writer mutations.
    /// # Errors
    /// Refuses population/storage limits, foreign or inaccessible coordinates,
    /// invalid/unbound selected slots, or caller work. No shared state, capacity
    /// or peak receipt is mutated, including on refusal or caller unwind.
    pub fn find_pattern_with<E>(
        &self,
        pattern: PatternRef<'_>,
        values: AssignmentSlice<'_>,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<usize>, AssignedFailure<E>> {
        let lookup = self.lookup();
        lookup.admit(Projected::HEADER_BYTES, limits)?;
        let read = storage::Read::from(&*self.store);
        let predicate = projected_predicate(read, pattern, &mut before)?;
        let arguments = ProjectionSource::Pattern(pattern.terms());
        let projected = Projected {
            predicate,
            values: values.slots,
            arguments,
        };
        projected_scope(read, values, &mut before)?;
        for column in 0..arguments.len() {
            before().map_err(Failure::Stopped)?;
            projected_argument(read, values, arguments.at(column), &mut before)?;
        }
        lookup
            .find_admitted(Query::Projected(&projected), before)
            .map_err(Into::into)
    }

    /// Import one term without discovering an atom. Existing same-scope terms
    /// reuse their identity; foreign or ingress terms use the shared importer.
    /// Logical expanded measures and total owner capacity have distinct limits.
    /// # Errors
    /// Refuses logical/storage bounds or caller work; complete components may remain.
    pub fn import_term_with<E>(
        &mut self,
        source: TermRef<'_>,
        term_limits: TermLimits,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<TermKey, Failure<E>> {
        population(self.len(), limits)?;
        admit(self.storage_bytes(), limits)?;
        before().map_err(Failure::Stopped)?;
        let id = self.import(limits, |store| {
            store.import_term_with(source, term_limits, &mut before)
        })?;
        before().map_err(Failure::Stopped)?;
        Ok(TermKey {
            scope: self.read().0.vocabulary_scope(),
            id,
        })
    }

    /// Construct one node from existing canonical child slots, retaining no
    /// flattened subtree copy. Repeated slots retain repeated logical occurrences.
    /// Scalars use no child slots. Temporary child IDs count in both the named
    /// owner capacity and the logical constructor's byte allowance.
    /// # Errors
    /// Refuses scope, slot, logical/storage bounds or caller work before returning a key.
    pub fn construct_term_with<E>(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        values: AssignmentSlice<'_>,
        children: &[usize],
        term_limits: TermLimits,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<TermKey, AssignedFailure<E>> {
        population(self.len(), limits)?;
        admit(self.storage_bytes(), limits)?;
        values.validate_with(self.read(), children, &mut before)?;
        let id = self.import(limits, |store| {
            store.construct_assigned_with(
                descriptor,
                values.slots,
                children,
                term_limits,
                &mut before,
            )
        })?;
        before().map_err(Failure::Stopped)?;
        Ok(TermKey {
            scope: self.read().0.vocabulary_scope(),
            id,
        })
    }

    /// Discover one complete assigned atom through the shared checked AVL path.
    /// Slot order and repetitions are preserved; no owned Atom or Value is built.
    /// Predicate declaration and assignment must belong to this vocabulary.
    /// The immutable slot projection serves lookup and canonical admission
    /// without an argument vector, including for a vacant row. Its fixed header
    /// and prepared-result slot count throughout validation and insertion;
    /// borrowed caller arrays remain excluded. Every argument obeys `term_limits`.
    /// # Errors
    /// Refuses scope, slot, arity, logical/storage bounds or caller work. No discovery
    /// position is published after refusal; admitted components/capacity may remain.
    pub fn insert_assigned_with<E>(
        &mut self,
        predicate: &DeclaredPredicate,
        values: AssignmentSlice<'_>,
        arguments: &[usize],
        term_limits: TermLimits,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, AssignedFailure<E>> {
        before().map_err(Failure::Stopped)?;
        let (predicate, signature) = self
            .read()
            .resolve_predicate(predicate)
            .map_err(|error| AssignedFailure::Assignment(AssignmentError::Read(error)))?;
        if signature.arity() != arguments.len() {
            return Err(Failure::Catalog(super::super::Error::Shape).into());
        }
        self.insert_projected_with(
            predicate,
            values,
            ProjectionSource::Slots(arguments),
            term_limits,
            limits,
            before,
        )
    }

    /// Discover a canonical pattern under one scoped assignment without copying
    /// its constants or constructing a second assignment. The pattern's signed
    /// predicate and constants must already belong to this vocabulary and its
    /// accessible prefix. Variables, constants and repeated arguments keep their
    /// pattern order. An empty assignment still authenticates its vocabulary.
    ///
    /// The same immutable argument projection serves checked lookup and row
    /// publication without an argument vector. Argument reads can resolve
    /// immutable template segment metadata; cached logical measures also include
    /// segment-directory lookup. Its fixed header and prepared-result slot count
    /// in `max_bytes` before fallible argument checks, along with simultaneous
    /// discovery/canonical growth. Borrowed inputs remain separate. Per-argument
    /// logical limits apply even when discovery already exists.
    ///
    /// # Errors
    /// Refuses uninterned/foreign input, inaccessible IDs, absent variables,
    /// logical/storage bounds or caller work. No failed operation publishes a
    /// discovery position; complete canonical components/capacities may remain.
    pub fn insert_pattern_with<E>(
        &mut self,
        pattern: PatternRef<'_>,
        values: AssignmentSlice<'_>,
        term_limits: TermLimits,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, AssignedFailure<E>> {
        let read = storage::Read::from(&*self.store);
        let predicate = projected_predicate(read, pattern, &mut before)?;
        self.insert_projected_with(
            predicate,
            values,
            ProjectionSource::Pattern(pattern.terms()),
            term_limits,
            limits,
            before,
        )
    }

    fn insert_projected_with<'a, E>(
        &mut self,
        predicate: storage::PredicateId,
        values: AssignmentSlice<'a>,
        arguments: ProjectionSource<'a>,
        term_limits: TermLimits,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, AssignedFailure<E>> {
        let projected = Projected {
            predicate,
            values: values.slots,
            arguments,
        };
        self.admit_entry(Projected::HEADER_BYTES, limits)?;
        let read = storage::Read::from(&*self.store);
        projected_scope(read, values, &mut before)?;
        for column in 0..arguments.len() {
            before().map_err(Failure::Stopped)?;
            let id = projected_argument(read, values, arguments.at(column), &mut before)?;
            self.store
                .check_projected_term_with(id, term_limits, &mut before)
                .map_err(|error| match error {
                    storage::Failure::Storage(error) => Failure::Catalog(error),
                    storage::Failure::Stopped(error) => Failure::Stopped(error),
                })?;
        }
        // Shared borrows keep this completely validated projection unchanged
        // through exact preparation, semantic placement and row publication.
        let query = Query::Projected(&projected);
        let Identity::Local(prepared) = query.identity_with(self.store, &mut before)? else {
            unreachable!("validated projection has local canonical coordinates");
        };
        self.reborrow()
            .entry_prepared(query, Some(prepared), limits, &mut before)?
            .insert_with(limits, before)
            .map_err(Into::into)
    }
}

fn projected_predicate<E>(
    read: storage::Read<'_>,
    pattern: PatternRef<'_>,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<storage::PredicateId, AssignedFailure<E>> {
    before().map_err(Failure::Stopped)?;
    let (source, predicate) =
        pattern
            .predicate()
            .canonical()
            .ok_or(AssignedFailure::Assignment(AssignmentError::Read(
                ReadError::Uninterned,
            )))?;
    before().map_err(Failure::Stopped)?;
    if !read.same_vocabulary(source) {
        return Err(AssignedFailure::Assignment(AssignmentError::Read(
            ReadError::ForeignCatalog,
        )));
    }
    before().map_err(Failure::Stopped)?;
    if !read.contains_predicate(predicate) {
        return Err(AssignedFailure::Assignment(AssignmentError::Read(
            ReadError::OutsidePrefix,
        )));
    }
    Ok(predicate)
}

fn projected_scope<E>(
    read: storage::Read<'_>,
    values: AssignmentSlice<'_>,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), AssignedFailure<E>> {
    before().map_err(Failure::Stopped)?;
    if !read.accepts_vocabulary_scope(values.scope) {
        return Err(AssignedFailure::Assignment(AssignmentError::Read(
            ReadError::ForeignCatalog,
        )));
    }
    Ok(())
}

fn projected_argument<E>(
    read: storage::Read<'_>,
    values: AssignmentSlice<'_>,
    argument: TemplateTerm<'_>,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<storage::TermId, AssignedFailure<E>> {
    before().map_err(Failure::Stopped)?;
    let id = match argument {
        TemplateTerm::Variable(slot) => values
            .slots
            .get(slot)
            .ok_or(AssignedFailure::Assignment(AssignmentError::Slot {
                slot,
                len: values.len(),
            }))?
            .ok_or(AssignedFailure::Assignment(AssignmentError::Unbound {
                slot,
            }))?,
        TemplateTerm::Constant(term) => {
            let (source, id) =
                term.canonical()
                    .ok_or(AssignedFailure::Assignment(AssignmentError::Read(
                        ReadError::Uninterned,
                    )))?;
            before().map_err(Failure::Stopped)?;
            if !read.same_vocabulary(source) {
                return Err(AssignedFailure::Assignment(AssignmentError::Read(
                    ReadError::ForeignCatalog,
                )));
            }
            id
        }
    };
    before().map_err(Failure::Stopped)?;
    if !read.contains_term(id) {
        return Err(AssignedFailure::Assignment(AssignmentError::Read(
            ReadError::OutsidePrefix,
        )));
    }
    Ok(id)
}

#[cfg(test)]
#[path = "terms/tests.rs"]
mod tests;
