//! One typed payload authority for compiled directives, selectors and queries.

use std::convert::Infallible;
use std::fmt;
use std::sync::Arc;

use zetesis_core::catalog::{
    AssignmentError, AssignmentFailure, AssignmentSlice, CatalogRead, DeclaredConstructor,
    Error as StorageError, Limits, PredicateRef, TermKey, TermRef, Vocabulary, VocabularyBuilder,
    VocabularyFailure,
};
use zetesis_core::{
    TemplateCatalogFailure, TemplateComponents, TemplateComponentsRef, TemplateTerm, ValueNodeRef,
};

/// Independent named capacity for one compiled metadata bundle. This is not
/// cumulative source text, observation output, or process resident memory.
#[derive(Clone, Copy, Debug)]
pub struct MetadataStorageLimits {
    /// Canonical vocabulary and component capacities, including their named
    /// construction and publication overlap. Source topology, compiler maps and
    /// temporary coordinate vectors retain their independent logical admission
    /// limits; allocator bookkeeping and process resident memory are excluded.
    pub max_bytes: usize,
}
impl Default for MetadataStorageLimits {
    fn default() -> Self {
        Self {
            max_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Typed refusal while constructing or publishing compiled metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetadataStorageError {
    /// Canonical component identity, capacity, allocation or failed transaction.
    Components(TemplateCatalogFailure),
    /// Scoped child/constant metadata or canonical term construction refused.
    Assignment(AssignmentError),
}
impl fmt::Display for MetadataStorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Components(error) => error.fmt(formatter),
            Self::Assignment(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for MetadataStorageError {}
impl From<TemplateCatalogFailure> for MetadataStorageError {
    fn from(error: TemplateCatalogFailure) -> Self {
        Self::Components(error)
    }
}
impl From<StorageError> for MetadataStorageError {
    fn from(error: StorageError) -> Self {
        Self::Components(error.into())
    }
}

/// Local scalar occurrence, meaningful only with its enclosing metadata owner.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Scalar(usize);
/// Local constructor occurrence; its name belongs only to the vocabulary.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Constructor(usize);
/// Local signed-predicate occurrence, not a semantic ordering key.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Predicate(usize);

#[derive(Debug)]
pub(crate) struct MetadataVocabulary {
    vocabulary: Vocabulary,
    components: TemplateComponents,
}
impl MetadataVocabulary {
    pub(crate) fn read_with<E>(
        &self,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Read<'_>, TemplateCatalogFailure<E>> {
        let catalog = self.vocabulary.read();
        self.components
            .bind_with(catalog, before)
            .map(|components| Read {
                catalog,
                components,
            })
    }
}

/// A checked pairing of one immutable vocabulary prefix and its occurrences.
#[derive(Clone, Copy)]
pub(crate) struct Read<'a> {
    catalog: CatalogRead<'a>,
    components: TemplateComponentsRef<'a>,
}
impl<'a> Read<'a> {
    pub(crate) fn catalog(self) -> CatalogRead<'a> {
        self.catalog
    }
    pub(crate) fn scalar(self, scalar: Scalar) -> Option<TermRef<'a>> {
        match self.components.term(scalar.0)? {
            TemplateTerm::Constant(value) => Some(value),
            TemplateTerm::Variable(_) => None,
        }
    }
    pub(crate) fn constructor(self, constructor: Constructor) -> Option<ValueNodeRef<'a>> {
        self.components.constructor(constructor.0)
    }
    pub(crate) fn constructor_declaration(
        self,
        constructor: Constructor,
    ) -> Option<DeclaredConstructor> {
        self.components.declared_constructor(constructor.0)
    }
    pub(crate) fn predicate(self, predicate: Predicate) -> Option<PredicateRef<'a>> {
        self.components.predicate(predicate.0)
    }
}

/// The bundle's sole unpublished vocabulary/component authority. Vocabulary and
/// component capacities share one allowance; caller topology and temporary
/// coordinate vectors retain their distinct logical limits. Complete nodes may
/// remain after refusal; components
/// retain their own poison state and are never published while incomplete.
#[derive(Debug)]
pub(crate) struct Authority {
    builder: VocabularyBuilder,
    components: TemplateComponents,
    limits: MetadataStorageLimits,
    peak: u128,
}
const HEADER: usize =
    size_of::<Authority>() - size_of::<VocabularyBuilder>() - size_of::<TemplateComponents>();
impl Authority {
    pub(crate) fn new(
        limits: MetadataStorageLimits,
        external: u128,
    ) -> Result<Self, MetadataStorageError> {
        let outer = external + HEADER as u128;
        let builder = VocabularyBuilder::new(remaining(limits, outer)?)
            .map_err(|error| storage(error, outer, limits))?;
        let components = TemplateComponents::new(
            builder.read(),
            remaining(limits, external + HEADER as u128 + builder.storage_bytes())?,
        )
        .map_err(|error| {
            component(
                error,
                external + HEADER as u128 + builder.storage_bytes(),
                limits,
            )
        })?;
        let mut result = Self {
            builder,
            components,
            limits,
            peak: 0,
        };
        result.observe(external)?;
        Ok(result)
    }
    pub(crate) fn bytes(&self) -> u128 {
        HEADER as u128 + self.builder.storage_bytes() + self.components.storage_bytes()
    }
    pub(crate) fn read(&self) -> Result<Read<'_>, MetadataStorageError> {
        let catalog = self.builder.read();
        let components = self.components.bind_with(catalog, READY)?;
        Ok(Read {
            catalog,
            components,
        })
    }
    pub(crate) fn catalog(&self) -> CatalogRead<'_> {
        self.builder.read()
    }
    pub(crate) fn observe(&mut self, external: u128) -> Result<(), MetadataStorageError> {
        self.peak = self.peak.max(external + self.bytes());
        check(self.limits, external + self.bytes())
    }
    fn writer(&mut self, external: u128) -> Result<u128, MetadataStorageError> {
        let outer = external + HEADER as u128 + self.components.storage_bytes();
        self.builder.restart_storage_peak();
        self.builder
            .ceiling(remaining(self.limits, outer)?)
            .map_err(|error| storage(error, outer, self.limits))?;
        Ok(outer)
    }
    fn written<T>(
        &mut self,
        result: Result<T, VocabularyFailure<Infallible>>,
        outer: u128,
    ) -> Result<T, MetadataStorageError> {
        self.peak = self.peak.max(outer + self.builder.storage_peak_bytes());
        result.map_err(|error| match error {
            VocabularyFailure::Storage(error) => storage(error, outer, self.limits),
            VocabularyFailure::Stopped(impossible) => match impossible {},
        })
    }
    fn append<T>(
        &mut self,
        external: u128,
        action: impl FnOnce(
            &mut TemplateComponents,
            CatalogRead<'_>,
            usize,
        ) -> Result<T, TemplateCatalogFailure>,
    ) -> Result<T, MetadataStorageError> {
        let outer = external + HEADER as u128 + self.builder.storage_bytes();
        let available = remaining(self.limits, outer)?;
        self.components.restart_storage_peak();
        let result = action(&mut self.components, self.builder.read(), available);
        self.peak = self.peak.max(outer + self.components.storage_peak_bytes());
        let value = result.map_err(|error| component(error, outer, self.limits))?;
        check(self.limits, outer + self.components.storage_peak_bytes())?;
        Ok(value)
    }
    pub(crate) fn construct(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        values: AssignmentSlice<'_>,
        children: &[usize],
        logical: Limits,
        external: u128,
    ) -> Result<TermKey, MetadataStorageError> {
        let outer = self.writer(external)?;
        let result = self
            .builder
            .construct_term_with(descriptor, values, children, logical, READY);
        self.peak = self.peak.max(outer + self.builder.storage_peak_bytes());
        result.map_err(|error| match error {
            AssignmentFailure::Assignment(AssignmentError::Storage(error)) => {
                storage(error, outer, self.limits)
            }
            AssignmentFailure::Assignment(error) => MetadataStorageError::Assignment(error),
            AssignmentFailure::Stopped(impossible) => match impossible {},
        })
    }
    pub(crate) fn scalar(
        &mut self,
        key: &TermKey,
        external: u128,
    ) -> Result<Scalar, MetadataStorageError> {
        self.append(external, |components, read, available| {
            let term = read.term(key).map_err(TemplateCatalogFailure::Read)?;
            components
                .append_terms_with(read, [TemplateTerm::Constant(term)], available, READY)
                .map(|range| Scalar(range.start))
        })
    }
    pub(crate) fn predicate(
        &mut self,
        predicate: PredicateRef<'_>,
        external: u128,
    ) -> Result<Predicate, MetadataStorageError> {
        let outer = self.writer(external)?;
        let result = self.builder.declare_predicate_with(predicate, READY);
        let predicate = self.written(result, outer)?;
        self.append(external, |components, read, available| {
            components
                .append_predicate_with(read, &predicate, available, READY)
                .map(Predicate)
        })
    }
    pub(crate) fn constructor(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        external: u128,
    ) -> Result<Constructor, MetadataStorageError> {
        let outer = self.writer(external)?;
        let result = self.builder.declare_constructor_with(descriptor, READY);
        let constructor = self.written(result, outer)?;
        self.declared_constructor(&constructor, external)
    }
    pub(crate) fn declared_constructor(
        &mut self,
        constructor: &DeclaredConstructor,
        external: u128,
    ) -> Result<Constructor, MetadataStorageError> {
        self.append(external, |components, read, available| {
            components
                .append_constructor_with(read, constructor, available, READY)
                .map(Constructor)
        })
    }
    pub(crate) fn negate(
        &mut self,
        constructor: Constructor,
    ) -> Result<Constructor, MetadataStorageError> {
        let read = self.read()?;
        let descriptor = read.constructor(constructor).ok_or(StorageError::Shape)?;
        let ValueNodeRef::Function { sign, .. } = descriptor else {
            return Err(StorageError::Shape.into());
        };
        let sign = match sign {
            zetesis_core::Sign::Positive => zetesis_core::Sign::Negative,
            zetesis_core::Sign::Negative => zetesis_core::Sign::Positive,
        };
        let declaration = read
            .constructor_declaration(constructor)
            .ok_or(StorageError::Shape)?;
        let declaration = declaration.with_sign(sign).ok_or(StorageError::Shape)?;
        self.declared_constructor(&declaration, 0)
    }
    pub(crate) fn finish(
        mut self,
        external: u128,
    ) -> Result<Arc<MetadataVocabulary>, MetadataStorageError> {
        self.observe(external)?;
        // The writer had only its remaining allowance during admission. Its
        // final operation checks the complete simultaneous publication owner.
        self.builder.ceiling(self.limits.max_bytes)?;
        let metadata = external
            + HEADER as u128
            + self.components.storage_bytes()
            + (size_of::<MetadataVocabulary>()
                - size_of::<Vocabulary>()
                - size_of::<TemplateComponents>()) as u128;
        let vocabulary =
            self.builder
                .finish_with(metadata, READY)
                .map_err(|error| match error {
                    VocabularyFailure::Storage(error) => MetadataStorageError::from(error),
                    VocabularyFailure::Stopped(impossible) => match impossible {},
                })?;
        self.components.bind_with(vocabulary.read(), READY)?;
        Ok(Arc::new(MetadataVocabulary {
            vocabulary,
            components: self.components,
        }))
    }
}

const READY: fn() -> Result<(), Infallible> = || Ok(());
fn check(limits: MetadataStorageLimits, observed: u128) -> Result<(), MetadataStorageError> {
    if observed > limits.max_bytes as u128 {
        Err(StorageError::Storage {
            required: observed,
            limit: limits.max_bytes,
        }
        .into())
    } else {
        Ok(())
    }
}
fn remaining(limits: MetadataStorageLimits, external: u128) -> Result<usize, MetadataStorageError> {
    check(limits, external)?;
    Ok(limits.max_bytes
        - usize::try_from(external).expect("external storage checked against usize ceiling"))
}
fn storage(
    error: StorageError,
    outer: u128,
    limits: MetadataStorageLimits,
) -> MetadataStorageError {
    match error {
        StorageError::Storage { required, .. } => StorageError::Storage {
            required: outer + required,
            limit: limits.max_bytes,
        }
        .into(),
        error => error.into(),
    }
}
fn component(
    error: TemplateCatalogFailure,
    outer: u128,
    limits: MetadataStorageLimits,
) -> MetadataStorageError {
    match error {
        TemplateCatalogFailure::Storage(error) => storage(error, outer, limits),
        error => error.into(),
    }
}
