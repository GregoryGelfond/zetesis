//! Scoped component metadata over an existing canonical source vocabulary.

use std::{convert::Infallible, fmt, sync::Arc};

use crate::catalog::{CatalogRead, Error, ReadError};
use crate::{AtomCatalog, FilterRef, PatternRef, TemplateTerm};

use super::components::{Metadata, check, next};
use super::{Data, RowData, Source, TemplateCatalog};

/// Canonical component metadata could not be completed or bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemplateCatalogFailure<E = Infallible> {
    /// A constant, signature, constructor or binding uses an incompatible prefix.
    Read(ReadError),
    /// Named metadata capacity or allocation could not be admitted.
    Storage(Error),
    /// The caller refused before the next metadata operation.
    Stopped(E),
    /// An earlier mutation failed; incomplete components cannot be exposed.
    Incomplete,
}
impl<E: fmt::Display> fmt::Display for TemplateCatalogFailure<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(formatter),
            Self::Storage(error) => error.fmt(formatter),
            Self::Stopped(error) => error.fmt(formatter),
            Self::Incomplete => formatter.write_str("canonical component metadata is incomplete"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for TemplateCatalogFailure<E> {}
impl<E> From<Error> for TemplateCatalogFailure<E> {
    fn from(error: Error) -> Self {
        Self::Storage(error)
    }
}
impl<E> From<ReadError> for TemplateCatalogFailure<E> {
    fn from(error: ReadError) -> Self {
        Self::Read(error)
    }
}

/// Ordered template metadata naming one existing canonical vocabulary.
///
/// This owner imports no values or signatures. Constants and predicates must
/// already belong to each supplied reader. It retains one identity witness and
/// local metadata, never a writer, snapshot, or per-cell reference-counted key.
/// Publication binds all rows to one exact supplied immutable atom catalog.
/// Neither rows nor that catalog establish logical support or variable safety.
///
/// Metadata allowances cover this header, actual row/argument capacities and
/// old/replacement allocation overlap. Shared source storage, caller input and
/// allocator/Arc bookkeeping are excluded; source and caller storage must be
/// charged separately by the caller.
/// Every append failure prevents publication, while retained capacity and the
/// largest actual overlap remain inspectable for the caller's storage ledger.
/// A caller panic also poisons publication. After a caught panic, receipts may
/// conservatively retain charges for abandoned private buffers.
#[derive(Debug)]
pub struct TemplateCatalogSelection {
    metadata: Metadata,
    rows: Vec<RowData>,
    complete: usize,
}

impl TemplateCatalogSelection {
    /// Start an empty selection carrying this vocabulary's identity witness.
    /// Even an empty selection must later bind to that same vocabulary.
    ///
    /// # Errors
    /// Refuses a metadata allowance smaller than the empty owner header.
    pub fn new(
        read: CatalogRead<'_>,
        max_metadata_bytes: usize,
    ) -> Result<Self, TemplateCatalogFailure> {
        Ok(Self {
            metadata: Metadata::new(read, size_of::<Self>(), max_metadata_bytes)?,
            rows: Vec::new(),
            complete: 0,
        })
    }

    /// Number of completed appends. An interrupted final row is never included;
    /// after a caller panic the poisoned owner cannot expose its earlier rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.complete
    }
    /// Whether no complete row has been admitted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.complete == 0
    }
    /// Current header and metadata capacity, including a retained partial row.
    /// Canonical source payload and indexes are not owned or included here.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        self.metadata.bytes
    }
    /// Greatest actual retained or replacement-overlap metadata envelope since
    /// construction or the latest `restart_storage_peak`. Refused proposals that
    /// allocated nothing do not increase this receipt.
    #[must_use]
    pub fn storage_peak_bytes(&self) -> u128 {
        self.metadata.peak
    }

    /// Start a new measurement interval at current retained metadata. Call before
    /// composing an operation's peak with other storage whose lifetime may have
    /// changed since the previous operation. No allocation or payload read occurs.
    pub fn restart_storage_peak(&mut self) {
        self.metadata.peak = self.metadata.bytes;
    }

    /// Named overlap required while replacing this selection's header with the
    /// published component header. Shared metadata buffers are counted once.
    /// This is a proposed envelope, not an observed allocation receipt: record
    /// it as an actual peak only after `finish_with` successfully publishes.
    #[must_use]
    pub fn publication_peak_bytes(&self) -> u128 {
        self.metadata.bytes + size_of::<Data>() as u128
    }

    /// Append an original-order row of already canonical fields. Variables and
    /// repeated occurrences retain their exact supplied positions. Charges precede
    /// input iterator advances, canonical field reads, metadata copies, existing
    /// cell relocation and allocation. Caller iterator work is not inspected.
    /// Successful append returns this selection's local row coordinate.
    ///
    /// # Errors
    /// Refuses ingress constants/signatures, foreign owners, inaccessible input
    /// prefixes, metadata limits, allocation or caller work. Every failure poisons
    /// publication; no incomplete logical row can subsequently escape.
    pub fn append_with<'a, E>(
        &mut self,
        read: CatalogRead<'_>,
        terms: impl IntoIterator<Item = TemplateTerm<'a>>,
        patterns: impl IntoIterator<Item = PatternRef<'a>>,
        filters: impl IntoIterator<Item = FilterRef<'a>>,
        max_metadata_bytes: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, TemplateCatalogFailure<E>> {
        self.metadata.begin(read, max_metadata_bytes, &mut before)?;
        let position = self.append(
            read,
            terms,
            patterns,
            filters,
            max_metadata_bytes,
            &mut before,
        )?;
        self.metadata.complete();
        Ok(position)
    }

    fn append<'a, E>(
        &mut self,
        read: CatalogRead<'_>,
        terms: impl IntoIterator<Item = TemplateTerm<'a>>,
        patterns: impl IntoIterator<Item = PatternRef<'a>>,
        filters: impl IntoIterator<Item = FilterRef<'a>>,
        limit: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<usize, TemplateCatalogFailure<E>> {
        self.metadata.reserve(&mut self.rows, 1, limit, before)?;
        // Attach the empty row before growing nested buffers, so actual capacities
        // survive ordinary refusal and remain represented by the shared ledger.
        before().map_err(TemplateCatalogFailure::Stopped)?;
        self.rows.push(RowData::default());
        let row = self.rows.last_mut().expect("just appended row");
        self.metadata
            .terms(&mut row.terms, read, terms, limit, before)?;
        let mut patterns = patterns.into_iter();
        while let Some(pattern) = next(&mut patterns, before)? {
            self.metadata
                .pattern(&mut row.patterns, read, pattern, limit, before)?;
        }
        let mut filters = filters.into_iter();
        while let Some(filter) = next(&mut filters, before)? {
            self.metadata
                .filter(&mut row.filters, read, filter, limit, before)?;
        }
        before().map_err(TemplateCatalogFailure::Stopped)?;
        let position = self.complete;
        self.complete += 1;
        Ok(position)
    }

    /// Bind all complete metadata to this exact final source catalog. The source
    /// may be a later compatible prefix; its occurrences are not template rows.
    /// No snapshot is rebuilt and no payload or lookup index is copied.
    /// The metadata limit admits the old header and new publication envelope
    /// together. Arc envelope allocation keeps Rust's infallible boundary.
    ///
    /// # Errors
    /// Refuses a prior failed append, foreign vocabulary, missing referenced
    /// identity, publication allowance or caller stop. No partial catalog escapes.
    pub fn finish_with<E>(
        self,
        source: AtomCatalog,
        max_metadata_bytes: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<TemplateCatalog, TemplateCatalogFailure<E>> {
        self.metadata.bind(source.read(), &mut before)?;
        check(self.publication_peak_bytes(), max_metadata_bytes)?;
        before().map_err(TemplateCatalogFailure::Stopped)?;
        let metadata_bytes =
            self.metadata.bytes - size_of::<Self>() as u128 + size_of::<Data>() as u128;
        Ok(TemplateCatalog(Arc::new(Data {
            source: Source::Selected(source),
            rows: self.rows,
            metadata_bytes,
        })))
    }
}

#[cfg(test)]
mod tests;
