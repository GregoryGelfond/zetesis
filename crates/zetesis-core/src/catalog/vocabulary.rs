//! Neutral term, predicate and constructor ownership over the canonical store.

use super::{
    AssignmentError, AssignmentFailure, AssignmentSlice, CatalogRead, DeclaredConstructor,
    DeclaredPredicate, Error, Limits, PredicateRef, TermKey, TermRef, storage,
};
use crate::ValueNodeRef;

/// Storage or the caller's exact stop value from vocabulary admission.
pub use super::storage::Failure as VocabularyFailure;

/// One append authority for typed vocabulary, without atom discovery or truth.
/// The existing store supplies normalization, exact collision checks, iterative
/// ingress and actual named-capacity accounting. No source payload is retained.
#[derive(Debug)]
pub struct VocabularyBuilder {
    store: storage::Store,
}

impl VocabularyBuilder {
    /// Create an empty vocabulary under an inclusive named-storage allowance.
    /// Fixed Box/Arc envelopes follow the store's infallible allocation contract.
    /// # Errors
    /// Refuses a ceiling smaller than the named empty authority.
    pub fn new(max_storage_bytes: usize) -> Result<Self, Error> {
        let mut store = storage::Store::new(max_storage_bytes);
        store.ceiling(max_storage_bytes)?;
        Ok(Self { store })
    }

    /// Borrow the current exact prefix. This borrow prevents concurrent append.
    #[must_use]
    pub fn read(&self) -> CatalogRead<'_> {
        CatalogRead((&self.store).into())
    }

    /// Named current capacity, including the writer, indexes and complete nodes.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        self.store.current_bytes()
    }

    /// Greatest actual capacity and replacement overlap since the last restart.
    #[must_use]
    pub fn storage_peak_bytes(&self) -> u128 {
        self.store.peak_bytes()
    }

    /// Start a fresh operation receipt at current retained capacity.
    pub fn restart_storage_peak(&mut self) {
        self.store.restart_peak();
    }

    /// Change the writer's allowance when a caller's simultaneous metadata grows.
    /// A refused smaller allowance remains installed; capacity is never hidden.
    /// # Errors
    /// Refuses a ceiling below the actual retained named capacity.
    pub fn ceiling(&mut self, max_storage_bytes: usize) -> Result<(), Error> {
        self.store.ceiling(max_storage_bytes)
    }

    /// Import a typed value through the common metered canonical importer.
    /// Keys retain identity only; resolve them through a compatible live reader.
    /// # Errors
    /// Refuses logical/storage bounds or caller work. Complete imported components
    /// and observed reservations may remain after refusal, never a partial node.
    pub fn import_term_with<E>(
        &mut self,
        term: TermRef<'_>,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<TermKey, VocabularyFailure<E>> {
        let id = self.store.import_term_with(term, limits, &mut before)?;
        before().map_err(VocabularyFailure::Stopped)?;
        Ok(TermKey {
            scope: self.read().0.vocabulary_scope(),
            id,
        })
    }

    /// Construct one term node over child slots from this vocabulary. Child
    /// payload stays in the common store; repeated slots count as repeated
    /// logical occurrences. Scalars require no child slots. Temporary child IDs
    /// count against both the logical byte limit and the writer's storage ceiling.
    ///
    /// # Errors
    /// Refuses foreign or unavailable child slots, logical/storage bounds, or
    /// caller work. Complete admitted nodes and reservation capacity may remain
    /// after refusal; a partial node is never returned.
    pub fn construct_term_with<E>(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        values: AssignmentSlice<'_>,
        children: &[usize],
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<TermKey, AssignmentFailure<E>> {
        values.validate_with(self.read(), children, &mut before)?;
        let id = self
            .store
            .construct_assigned_with(descriptor, values.slots, children, limits, &mut before)
            .map_err(|error| match error {
                VocabularyFailure::Storage(error) => {
                    AssignmentFailure::Assignment(AssignmentError::Storage(error))
                }
                VocabularyFailure::Stopped(error) => AssignmentFailure::Stopped(error),
            })?;
        before().map_err(AssignmentFailure::Stopped)?;
        Ok(TermKey {
            scope: self.read().0.vocabulary_scope(),
            id,
        })
    }

    /// Declare a signed predicate without fabricating an atom or membership.
    /// # Errors
    /// Refuses storage, shape, allocation or the caller's work boundary.
    pub fn declare_predicate_with<E>(
        &mut self,
        predicate: PredicateRef<'_>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<DeclaredPredicate, VocabularyFailure<E>> {
        let id = self.store.import_predicate_with(predicate, &mut before)?;
        before().map_err(VocabularyFailure::Stopped)?;
        Ok(DeclaredPredicate::new(self.read().0, id))
    }

    /// Declare a real function/tuple shape, retaining its name only in text storage.
    /// Positive nullary functions remain shapes until a ground term is constructed.
    /// # Errors
    /// Refuses a nonconstructor descriptor, storage or caller work.
    pub fn declare_constructor_with<E>(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<DeclaredConstructor, VocabularyFailure<E>> {
        let data = self
            .store
            .declare_constructor_with(descriptor, &mut before)?;
        before().map_err(VocabularyFailure::Stopped)?;
        Ok(DeclaredConstructor::new(self.read().0, data))
    }

    /// Seal this vocabulary, moving its payload and indexes without copying them.
    /// `external_metadata_bytes` is admitted together with all publication
    /// envelopes and replacement capacity. Set the ceiling to the complete owner
    /// allowance first if prior operations deducted that external metadata.
    /// # Errors
    /// Refuses combined publication storage or caller work. No partial owner escapes.
    pub fn finish_with<E>(
        self,
        external_metadata_bytes: u128,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Vocabulary, VocabularyFailure<E>> {
        self.store
            .freeze_vocabulary_with(external_metadata_bytes, before)
            .map(|storage| Vocabulary { storage })
    }
}

/// Closed canonical typed vocabulary. Clone shares immutable payload and lookup
/// indexes; it does not create a writer or extend this identity's prefix.
#[derive(Clone, Debug)]
pub struct Vocabulary {
    storage: storage::FrozenVocabulary,
}
impl Vocabulary {
    /// Actual peak of the successful publication, including supplied external
    /// metadata and the writer/publication overlap. Earlier import history is
    /// excluded; combine this with the builder's separately retained peak receipt.
    #[must_use]
    pub fn publication_peak_bytes(&self) -> u128 {
        self.storage.publication_peak()
    }
    /// Borrow the complete closed term/predicate/text prefix.
    #[must_use]
    pub fn read(&self) -> CatalogRead<'_> {
        CatalogRead((&self.storage).into())
    }

    /// Named retained header, payload, directories and lookup indexes. Arc
    /// bookkeeping is excluded. Sum once for this shared owner, not per clone.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        size_of::<Self>() as u128 + self.storage.retained_bytes()
    }
}

#[cfg(test)]
mod tests;
