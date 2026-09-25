//! Independent occurrences over the same schema used by template rows.

use super::TemplateCatalogFailure;
use crate::catalog::storage::{PredicateId, Read, TermId};
use crate::catalog::{
    CatalogRead, ConstructorData, DeclaredConstructor, DeclaredPredicate, PredicateRef,
};
use crate::{FilterRef, Filters, PatternRef, PatternTerms, Patterns, TemplateTerm, ValueNodeRef};
use std::ops::Range;

mod metadata;
pub(super) use metadata::{Metadata, check, next};

#[derive(Clone, Copy, Debug)]
pub(crate) enum TermData {
    Variable(usize),
    Constant(TermId),
}
#[derive(Debug)]
pub(crate) struct PatternData {
    pub(crate) predicate: PredicateId,
    pub(crate) terms: Vec<TermData>,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct FilterData {
    pub(crate) equal: bool,
    pub(crate) left: TermData,
    pub(crate) right: TermData,
}
#[derive(Debug, Default)]
pub(crate) struct RowData {
    pub(crate) terms: Vec<TermData>,
    pub(crate) patterns: Vec<PatternData>,
    pub(crate) filters: Vec<FilterData>,
}

/// Independent ordered term, predicate, pattern, filter and constructor occurrences over
/// one canonical vocabulary. This owner retains only IDs and variable slots,
/// with one vocabulary witness; it retains no payload, reader or snapshot.
/// Binding borrows an explicit compatible prefix covering all referenced IDs,
/// including constructor text admitted without any ground term.
///
/// Returned positions and ranges are local occurrence coordinates, meaningful
/// only with this owner. Equal descriptions may occur repeatedly. These are not
/// transferable canonical identity keys. No rule, membership, domain, support or
/// variable-safety policy is implied by storing a component.
///
/// The byte allowance includes this header, actual component vector capacities
/// and old/replacement allocation overlap. Shared source storage, caller inputs
/// and allocator bookkeeping are separate. Every failed or panicking mutation
/// poisons subsequent mutation and binding. Actual retained capacity remains
/// inspectable on refusal; caught unwind receipts can conservatively retain
/// charges for an abandoned private buffer.
#[derive(Debug)]
pub struct TemplateComponents {
    metadata: Metadata,
    data: RowData,
    predicates: Vec<PredicateId>,
    constructors: Vec<ConstructorData>,
}
impl TemplateComponents {
    /// Start an empty component owner carrying this vocabulary's scope.
    /// # Errors
    /// Refuses an allowance smaller than the named empty header.
    pub fn new(
        read: CatalogRead<'_>,
        max_metadata_bytes: usize,
    ) -> Result<Self, TemplateCatalogFailure> {
        Ok(Self {
            metadata: Metadata::new(read, size_of::<Self>(), max_metadata_bytes)?,
            data: RowData::default(),
            predicates: Vec::new(),
            constructors: Vec::new(),
        })
    }
    /// Current named header and actual component capacities; source excluded.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        self.metadata.bytes
    }
    /// Greatest actual metadata envelope, including replacement overlap.
    #[must_use]
    pub fn storage_peak_bytes(&self) -> u128 {
        self.metadata.peak
    }
    /// Reset the peak receipt to current retained metadata; performs no reads.
    pub fn restart_storage_peak(&mut self) {
        self.metadata.peak = self.metadata.bytes;
    }

    /// Append ordered scalar occurrences, preserving variables and repetitions.
    /// Permits precede iterator advances, canonical reads, copies and allocation.
    /// # Errors
    /// Refuses failed state, foreign or missing-prefix identities, ingress values,
    /// storage or caller work. Any failure poisons the owner.
    pub fn append_terms_with<'a, E>(
        &mut self,
        read: CatalogRead<'_>,
        terms: impl IntoIterator<Item = TemplateTerm<'a>>,
        max_metadata_bytes: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Range<usize>, TemplateCatalogFailure<E>> {
        self.metadata.begin(read, max_metadata_bytes, &mut before)?;
        let start = self.data.terms.len();
        self.metadata.terms(
            &mut self.data.terms,
            read,
            terms,
            max_metadata_bytes,
            &mut before,
        )?;
        before().map_err(TemplateCatalogFailure::Stopped)?;
        self.metadata.complete();
        Ok(start..self.data.terms.len())
    }

    /// Retain one declared signed predicate without a fabricated atom or pattern.
    /// Repeated declarations remain distinct local occurrences of the same identity.
    /// # Errors
    /// Refuses foreign or inaccessible declarations, named storage or caller work.
    /// Any refusal or panic poisons subsequent mutation and binding.
    pub fn append_predicate_with<E>(
        &mut self,
        read: CatalogRead<'_>,
        predicate: &DeclaredPredicate,
        max_metadata_bytes: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, TemplateCatalogFailure<E>> {
        self.metadata.begin(read, max_metadata_bytes, &mut before)?;
        before().map_err(TemplateCatalogFailure::Stopped)?;
        let predicate = read.predicate(predicate)?;
        before().map_err(TemplateCatalogFailure::Stopped)?;
        let id = read.selected_predicate(predicate)?;
        self.metadata
            .reserve(&mut self.predicates, 1, max_metadata_bytes, &mut before)?;
        before().map_err(TemplateCatalogFailure::Stopped)?;
        let position = self.predicates.len();
        self.metadata.predicate(id);
        self.predicates.push(id);
        self.metadata.complete();
        Ok(position)
    }

    /// Append one signed predicate with its ordered argument descriptions.
    /// # Errors
    /// Refuses incompatible or missing identities, storage or caller work.
    /// Any refusal or panic poisons the owner, including after argument growth.
    pub fn append_pattern_with<E>(
        &mut self,
        read: CatalogRead<'_>,
        pattern: PatternRef<'_>,
        max_metadata_bytes: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, TemplateCatalogFailure<E>> {
        self.metadata.begin(read, max_metadata_bytes, &mut before)?;
        let position = self.data.patterns.len();
        self.metadata.pattern(
            &mut self.data.patterns,
            read,
            pattern,
            max_metadata_bytes,
            &mut before,
        )?;
        before().map_err(TemplateCatalogFailure::Stopped)?;
        self.metadata.complete();
        Ok(position)
    }

    /// Append one ordered equality or inequality description.
    /// # Errors
    /// Refuses incompatible or missing identities, storage or caller work;
    /// any failure poisons subsequent mutation and binding.
    pub fn append_filter_with<E>(
        &mut self,
        read: CatalogRead<'_>,
        filter: FilterRef<'_>,
        max_metadata_bytes: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, TemplateCatalogFailure<E>> {
        self.metadata.begin(read, max_metadata_bytes, &mut before)?;
        let position = self.data.filters.len();
        self.metadata.filter(
            &mut self.data.filters,
            read,
            filter,
            max_metadata_bytes,
            &mut before,
        )?;
        before().map_err(TemplateCatalogFailure::Stopped)?;
        self.metadata.complete();
        Ok(position)
    }

    /// Retain a declared constructor under this owner's single scope witness.
    /// The local occurrence stores only its text ID, sign and arity.
    /// # Errors
    /// Refuses foreign or inaccessible text, storage or caller work; any failure
    /// poisons subsequent mutation and binding.
    pub fn append_constructor_with<E>(
        &mut self,
        read: CatalogRead<'_>,
        constructor: &DeclaredConstructor,
        max_metadata_bytes: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, TemplateCatalogFailure<E>> {
        self.metadata.begin(read, max_metadata_bytes, &mut before)?;
        before().map_err(TemplateCatalogFailure::Stopped)?;
        let data = read.selected_constructor(constructor)?;
        self.metadata
            .reserve(&mut self.constructors, 1, max_metadata_bytes, &mut before)?;
        before().map_err(TemplateCatalogFailure::Stopped)?;
        let position = self.constructors.len();
        self.metadata.constructor(data);
        self.constructors.push(data);
        self.metadata.complete();
        Ok(position)
    }

    /// Borrow all occurrences after constant-time scope and maximum-ID extent
    /// checks. No row scan, payload copy or snapshot publication occurs. The view
    /// borrows both this owner and the supplied immutable or live read prefix.
    /// # Errors
    /// Refuses poisoned state, foreign vocabulary, an older required prefix or
    /// caller work. A binding refusal does not mutate the owner.
    pub fn bind_with<'a, E>(
        &'a self,
        read: CatalogRead<'a>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<TemplateComponentsRef<'a>, TemplateCatalogFailure<E>> {
        self.metadata.bind(read, &mut before)?;
        Ok(TemplateComponentsRef {
            read: read.storage(),
            data: &self.data,
            predicates: &self.predicates,
            constructors: &self.constructors,
        })
    }
}

/// A validated borrowed component prefix. Accessors use owner-local occurrence
/// positions and return None for an absent position; no identity is fabricated.
#[derive(Clone, Copy, Debug)]
pub struct TemplateComponentsRef<'a> {
    read: Read<'a>,
    data: &'a RowData,
    predicates: &'a [PredicateId],
    constructors: &'a [ConstructorData],
}
impl<'a> TemplateComponentsRef<'a> {
    /// Borrow one standalone predicate by its local occurrence position.
    #[must_use]
    pub fn predicate(self, position: usize) -> Option<PredicateRef<'a>> {
        self.predicates
            .get(position)
            .and_then(|id| PredicateRef::new(self.read, *id))
    }

    /// Scalar occurrences in supplied order, including repeated identities.
    #[must_use]
    pub fn terms(self) -> PatternTerms<'a> {
        PatternTerms::admitted(self.read, &self.data.terms)
    }
    /// Pattern occurrences in supplied order.
    #[must_use]
    pub fn patterns(self) -> Patterns<'a> {
        Patterns::admitted(self.read, &self.data.patterns)
    }
    /// Filter occurrences in supplied order.
    #[must_use]
    pub fn filters(self) -> Filters<'a> {
        Filters::admitted(self.read, &self.data.filters)
    }
    /// Borrow one scalar occurrence by local position.
    #[must_use]
    pub fn term(self, position: usize) -> Option<TemplateTerm<'a>> {
        self.terms().get(position)
    }
    /// Borrow one pattern occurrence by local position.
    #[must_use]
    pub fn pattern(self, position: usize) -> Option<PatternRef<'a>> {
        self.patterns().get(position)
    }
    /// Borrow one filter occurrence by local position.
    #[must_use]
    pub fn filter(self, position: usize) -> Option<FilterRef<'a>> {
        self.filters().get(position)
    }
    /// Borrow one declared constructor shape by local position.
    #[must_use]
    pub fn constructor(self, position: usize) -> Option<ValueNodeRef<'a>> {
        self.constructors
            .get(position)
            .map(|data| data.descriptor(self.read))
    }

    /// Retain one constructor's identity-only declaration by local occurrence
    /// position. The bound prefix already covers its name. The returned handle
    /// clones the vocabulary witness, never the text or reader, and can outlive
    /// this read borrow before a subsequent writer operation. Resolution still
    /// requires a compatible live prefix.
    #[must_use]
    pub fn declared_constructor(self, position: usize) -> Option<DeclaredConstructor> {
        self.constructors
            .get(position)
            .copied()
            .map(|data| DeclaredConstructor::new(self.read, data))
    }
}

#[cfg(test)]
mod tests;
