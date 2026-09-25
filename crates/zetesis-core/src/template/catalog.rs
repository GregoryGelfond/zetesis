//! Canonical storage for ordered template components, without logical policy.

use std::{convert::Infallible, sync::Arc};

use crate::catalog::storage::{Failure, FrozenVocabulary, PredicateId, Read, Store, TermId};
use crate::catalog::{AtomRef, Error, Limits, PredicateRef, TermRef};
use crate::{AtomCatalog, FilterRef, Filters, PatternRef, PatternTerms, Patterns, TemplateTerm};

mod selection;
pub use selection::{TemplateCatalogFailure, TemplateCatalogSelection};
mod components;
pub(crate) use components::{FilterData, PatternData, RowData, TermData};
pub use components::{TemplateComponents, TemplateComponentsRef};
#[derive(Debug)]
struct Data {
    source: Source,
    rows: Vec<RowData>,
    metadata_bytes: u128,
}

#[derive(Debug)]
enum Source {
    Frozen(FrozenVocabulary),
    Selected(AtomCatalog),
}
impl Source {
    fn read(&self) -> Read<'_> {
        match self {
            Self::Frozen(base) => Read::from(base),
            Self::Selected(catalog) => catalog.read().storage(),
        }
    }
    fn bytes(&self) -> u128 {
        match self {
            Self::Frozen(base) => base.retained_bytes(),
            Self::Selected(catalog) => catalog.storage().bytes,
        }
    }
}

/// Ordered template components over one canonical term/predicate vocabulary.
///
/// Rows and repeated occurrences retain source order. This storage establishes
/// no variable safety, rule support, truth, objective priority or carrier domain.
/// Semantic owners validate those policies and keep their own row topology.
/// Clones share the immutable source and component metadata without copying it.
/// The source is either an indexed closed vocabulary or an exact published atom
/// catalog. A selected catalog retains that entire prefix, including unselected atoms.
#[derive(Clone, Debug)]
pub struct TemplateCatalog(Arc<Data>);
impl TemplateCatalog {
    /// Number of admitted rows, including equal repeated descriptions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.rows.len()
    }
    /// Whether there are no component rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.rows.is_empty()
    }
    /// Borrow one complete component row by its local occurrence position.
    #[must_use]
    pub fn at(&self, row: usize) -> Option<TemplateRow<'_>> {
        self.0.rows.get(row).map(|data| TemplateRow {
            read: self.0.source.read(),
            data,
        })
    }
    /// Named retained base and metadata capacity, excluding external metadata
    /// registered by the semantic owner during admission and Arc counters.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        self.0.source.bytes() + self.0.metadata_bytes
    }
    /// Component header and actual row/argument/filter capacities, excluding
    /// every allocation retained by the source vocabulary or atom catalog.
    /// Constant time; this amount is included in `storage_bytes`.
    #[must_use]
    pub fn metadata_bytes(&self) -> u128 {
        self.0.metadata_bytes
    }

    /// Indexed closed vocabulary included in `storage_bytes`. Selected catalogs
    /// return zero because they retain a read snapshot without lookup indexes.
    /// A tuple writer created from a closed catalog shares this exact allocation.
    #[must_use]
    pub fn shared_vocabulary_bytes(&self) -> u128 {
        match &self.0.source {
            Source::Frozen(base) => base.retained_bytes(),
            Source::Selected(_) => 0,
        }
    }
    /// Immutable prefix allocations, excluding lookup indexes and any selected
    /// source occurrence map. Closed catalogs share these with their tuple writers;
    /// selected catalogs share these with their exact `source_catalog` snapshot.
    /// Subtract only after establishing that exact sharing. Visits segment metadata.
    #[must_use]
    pub fn shared_payload_bytes(&self) -> u128 {
        match &self.0.source {
            Source::Frozen(base) => base.payload_bytes(),
            Source::Selected(catalog) => catalog.shared_snapshot_bytes(),
        }
    }
    /// The exact atom catalog supplied when publishing selected template metadata.
    /// Closed standalone catalogs have no source atom occurrences and return None.
    #[must_use]
    pub fn source_catalog(&self) -> Option<&AtomCatalog> {
        match &self.0.source {
            Source::Frozen(_) => None,
            Source::Selected(catalog) => Some(catalog),
        }
    }
    pub(crate) fn read(&self) -> Read<'_> {
        self.0.source.read()
    }
    pub(crate) fn vocabulary(&self) -> Option<&FrozenVocabulary> {
        match &self.0.source {
            Source::Frozen(base) => Some(base),
            Source::Selected(_) => None,
        }
    }
    pub(crate) fn data(&self, row: usize) -> Option<&RowData> {
        self.0.rows.get(row)
    }
}

/// Borrowed original-order terms, patterns and filters in one admitted row.
#[derive(Clone, Copy, Debug)]
pub struct TemplateRow<'a> {
    read: Read<'a>,
    data: &'a RowData,
}
impl<'a> TemplateRow<'a> {
    /// Scalar fields, including variable slots and repeated constants.
    #[must_use]
    pub fn terms(self) -> PatternTerms<'a> {
        PatternTerms::admitted(self.read, &self.data.terms)
    }
    /// Relational pattern occurrences, without a support or gate interpretation.
    #[must_use]
    pub fn patterns(self) -> Patterns<'a> {
        Patterns::admitted(self.read, &self.data.patterns)
    }
    /// Exact equality/inequality filter occurrences.
    #[must_use]
    pub fn filters(self) -> Filters<'a> {
        Filters::admitted(self.read, &self.data.filters)
    }
}

/// Fallible construction of one shared canonical template authority.
///
/// Inputs are borrowed only while importing and are never retained. No implicit
/// depth limit is imposed on already validated values. The explicit byte limit
/// covers canonical storage/indexes, component capacities, import scratch and
/// publication overlap together. Caller ingress and allocator/Arc bookkeeping
/// are excluded. An interrupted append poisons publication; no partial row can
/// escape through `finish`. Admission has no execution work-quota contract.
#[derive(Debug)]
pub struct TemplateCatalogBuilder {
    store: Store,
    rows: Vec<RowData>,
    metadata: u128,
    external: u128,
    limit: usize,
    failed: Option<Error>,
}
impl TemplateCatalogBuilder {
    /// Start a catalog under an inclusive named storage allowance.
    ///
    /// # Errors
    /// Refuses even the empty authority when its named envelope cannot fit.
    pub fn new(max_bytes: usize) -> Result<Self, Error> {
        let mut builder = Self {
            store: Store::new(max_bytes),
            rows: Vec::new(),
            metadata: size_of::<Data>() as u128,
            external: 0,
            limit: max_bytes,
            failed: None,
        };
        builder.limit_store()?;
        Ok(builder)
    }
    /// Admit simultaneously live metadata belonging to the semantic owner.
    /// Before an external reserve, supply old capacity plus proposed replacement;
    /// after reservation, replace it with actual retained capacity. This does not
    /// take ownership or infer how the caller allocated those buffers.
    ///
    /// # Errors
    /// Refuses combined storage before updating the admitted external envelope.
    pub fn set_external_bytes(&mut self, bytes: u128) -> Result<(), Error> {
        self.check(self.store.current_bytes() + self.metadata + bytes)?;
        self.external = bytes;
        self.limit_store()
    }
    /// Current named authority, component and admitted external storage.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        self.store.current_bytes() + self.metadata + self.external
    }

    /// Import a complete row without interpreting its fields as a logical rule.
    /// Repeated occurrences and variable numbers remain unchanged. A returned
    /// position identifies this catalog's occurrence, not a global identity.
    ///
    /// # Errors
    /// Refuses canonical import, arithmetic, allocation or the combined byte
    /// allowance. After a failed append, this builder cannot publish a catalog.
    pub fn append<'a>(
        &mut self,
        terms: impl IntoIterator<Item = TemplateTerm<'a>>,
        patterns: impl IntoIterator<Item = PatternRef<'a>>,
        filters: impl IntoIterator<Item = FilterRef<'a>>,
    ) -> Result<usize, Error> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        let result = self.append_row(terms, patterns, filters);
        if let Err(error) = &result {
            self.failed = Some(error.clone());
        }
        result
    }
    fn append_row<'a>(
        &mut self,
        terms: impl IntoIterator<Item = TemplateTerm<'a>>,
        patterns: impl IntoIterator<Item = PatternRef<'a>>,
        filters: impl IntoIterator<Item = FilterRef<'a>>,
    ) -> Result<usize, Error> {
        let mut rows = std::mem::take(&mut self.rows);
        let reserved = self.reserve(&mut rows, 1);
        self.rows = rows;
        reserved?;
        let mut row = RowData::default();
        for term in terms {
            self.reserve(&mut row.terms, 1)?;
            row.terms.push(self.term(term)?);
        }
        for pattern in patterns {
            self.reserve(&mut row.patterns, 1)?;
            let predicate = self.predicate(pattern.predicate())?;
            let mut terms = Vec::new();
            self.reserve(&mut terms, pattern.terms().len())?;
            for term in pattern.terms() {
                terms.push(self.term(term)?);
            }
            row.patterns.push(PatternData { predicate, terms });
        }
        for filter in filters {
            self.reserve(&mut row.filters, 1)?;
            let (left, right) = filter.terms();
            row.filters.push(FilterData {
                equal: filter.is_equality(),
                left: self.term(left)?,
                right: self.term(right)?,
            });
        }
        let position = self.rows.len();
        self.rows.push(row);
        Ok(position)
    }
    /// Include a closed input atom's vocabulary without retaining an atom row,
    /// pattern occurrence, fact or domain-membership assertion. A tuple writer
    /// can later reuse this frozen base for the atom's actual query metadata.
    ///
    /// # Errors
    /// Same canonical and byte refusals as append; failure prevents publication.
    pub fn include_atom_vocabulary(&mut self, atom: AtomRef<'_>) -> Result<(), Error> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        let result: Result<(), Error> = (|| {
            self.predicate(atom.predicate())?;
            for value in atom.values() {
                self.term(TemplateTerm::Constant(value))?;
            }
            Ok(())
        })();
        if let Err(error) = &result {
            self.failed = Some(error.clone());
        }
        result
    }
    /// Publish immutable rows and their single vocabulary; consumes the writer.
    ///
    /// # Errors
    /// Refuses a prior incomplete append or publication storage. Arc envelope
    /// allocation retains Rust's existing infallible boundary.
    pub fn finish(mut self) -> Result<TemplateCatalog, Error> {
        if let Some(error) = self.failed {
            return Err(error);
        }
        self.store.ceiling(self.limit)?;
        let vocabulary = self
            .store
            .freeze_vocabulary_with(self.metadata + self.external, || Ok::<(), Infallible>(()))
            .map_err(|failure| failure_error(failure, 0, self.limit))?;
        Ok(TemplateCatalog(Arc::new(Data {
            source: Source::Frozen(vocabulary),
            rows: self.rows,
            metadata_bytes: self.metadata,
        })))
    }

    pub(crate) fn data(&self, row: usize) -> Option<&RowData> {
        self.rows.get(row)
    }
    pub(crate) fn read(&self) -> Read<'_> {
        Read::from(&self.store)
    }
    fn term(&mut self, term: TemplateTerm<'_>) -> Result<TermData, Error> {
        match term {
            TemplateTerm::Variable(variable) => Ok(TermData::Variable(variable)),
            TemplateTerm::Constant(value) => {
                let limits = Limits {
                    max_nodes: usize::MAX,
                    max_depth: usize::MAX,
                    max_bytes: usize::MAX,
                };
                self.store
                    .import_term_with(value, limits, || Ok::<(), Infallible>(()))
                    .map(TermData::Constant)
                    .map_err(|failure| {
                        failure_error(failure, self.metadata + self.external, self.limit)
                    })
            }
        }
    }
    fn predicate(&mut self, predicate: PredicateRef<'_>) -> Result<PredicateId, Error> {
        self.store
            .import_predicate_with(predicate, || Ok::<(), Infallible>(()))
            .map_err(|failure| failure_error(failure, self.metadata + self.external, self.limit))
    }
    fn reserve<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<(), Error> {
        let required = values
            .len()
            .checked_add(additional)
            .ok_or(Error::Overflow)?;
        if required <= values.capacity() {
            return Ok(());
        }
        let target = required
            .max(values.capacity().checked_mul(2).ok_or(Error::Overflow)?)
            .max(4);
        self.check(self.storage_bytes() + target as u128 * size_of::<T>() as u128)?;
        let old = values.capacity() as u128 * size_of::<T>() as u128;
        values
            .try_reserve_exact(target - values.len())
            .map_err(|_| Error::Allocation)?;
        let actual = values.capacity() as u128 * size_of::<T>() as u128;
        self.check(self.storage_bytes() + actual)?;
        self.metadata += actual - old;
        self.limit_store()
    }
    fn check(&self, required: u128) -> Result<(), Error> {
        if required > self.limit as u128 {
            Err(Error::Storage {
                required,
                limit: self.limit,
            })
        } else {
            Ok(())
        }
    }
    fn limit_store(&mut self) -> Result<(), Error> {
        let metadata = self.metadata + self.external;
        let available = (self.limit as u128)
            .checked_sub(metadata)
            .and_then(|bytes| usize::try_from(bytes).ok())
            .ok_or(Error::Storage {
                required: self.store.current_bytes() + metadata,
                limit: self.limit,
            })?;
        self.store
            .ceiling(available)
            .map_err(|error| storage_error(error, metadata, self.limit))
    }
}
fn failure_error(failure: Failure<Infallible>, metadata: u128, limit: usize) -> Error {
    match failure {
        Failure::Storage(error) => storage_error(error, metadata, limit),
        Failure::Stopped(never) => match never {},
    }
}
fn storage_error(error: Error, metadata: u128, limit: usize) -> Error {
    match error {
        Error::Storage { required, .. } => Error::Storage {
            required: required + metadata,
            limit,
        },
        other => other,
    }
}

pub(crate) fn term(read: Read<'_>, id: TermId) -> TermRef<'_> {
    TermRef::new(read, id).expect("admitted template term belongs to its immutable prefix")
}
pub(crate) fn predicate(read: Read<'_>, id: PredicateId) -> PredicateRef<'_> {
    PredicateRef::new(read, id)
        .expect("admitted template predicate belongs to its immutable prefix")
}
