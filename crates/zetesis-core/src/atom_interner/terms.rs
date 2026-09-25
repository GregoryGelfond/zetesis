//! Scoped generated-term admission through the existing canonical authority.

use super::{
    AtomAppender, AtomInterner, CatalogRead, DeclaredPredicate, Failure, Limits, Query, TermLimits,
    admit, fmt, population, storage, store_failure,
};
use crate::ValueNodeRef;
use crate::catalog::{AssignmentError, AssignmentFailure, AssignmentSlice, TermKey, TermRef};

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
        population(self.len(), limits)?;
        admit(self.storage_bytes(), limits)?;
        values.validate_with(self.read(), arguments, &mut before)?;
        before().map_err(Failure::Stopped)?;
        let predicate = self
            .read()
            .predicate(predicate)
            .map_err(|error| AssignedFailure::Assignment(AssignmentError::Read(error)))?;
        if predicate.arity() != arguments.len() {
            return Err(Failure::Catalog(super::super::Error::Shape).into());
        }
        let predicate = self
            .read()
            .selected_predicate(predicate)
            .map_err(|error| AssignedFailure::Assignment(AssignmentError::Read(error)))?;
        self.store
            .check_assigned_with(values.slots, arguments, term_limits, &mut before)
            .map_err(|error| match error {
                storage::Failure::Storage(error) => Failure::Catalog(error),
                storage::Failure::Stopped(error) => Failure::Stopped(error),
            })?;
        self.reborrow()
            .entry(
                Query::Assigned {
                    predicate,
                    values,
                    slots: arguments,
                    limits: term_limits,
                },
                limits,
                &mut before,
            )?
            .insert_with(limits, before)
            .map_err(Into::into)
    }
}

#[cfg(test)]
#[path = "terms/tests.rs"]
mod tests;
