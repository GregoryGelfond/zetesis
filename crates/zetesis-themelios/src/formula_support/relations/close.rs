//! Consume completed support into indexed storage without retaining its truth views.

use themelios_base::span::Location;
use zetesis_core::{
    TemplateComponents,
    atom_interner::{self, ClosedCatalog},
};

use super::{SupportCatalog, atom_failure, owner_limits};
use crate::formula::ceiling;
use crate::formula_support::{CompletedCatalog, GroundingWork};
use crate::grounding_observer::Event;
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

/// Canonical storage and the original compiled occurrence metadata. Possible
/// support membership and query indexes are absent; storage presence is not truth.
pub(crate) struct ClosedSource {
    pub(crate) storage: ClosedCatalog,
    pub(crate) components: Option<TemplateComponents>,
    close_peak: u128,
}

impl ClosedSource {
    /// Admit the retained envelope after support-only metadata has been retired.
    /// The close attempt's actual peak remains part of the returned receipt.
    fn admit(
        storage: ClosedCatalog,
        components: Option<TemplateComponents>,
        source_peak: u128,
        external: u128,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<Self, SourceCloseFailure> {
        let mut closed = Self {
            storage,
            components,
            close_peak: source_peak,
        };
        let retained = closed.storage_bytes();
        let total = external
            .checked_add(retained)
            .ok_or_else(|| SourceCloseFailure {
                failure: overflow(location),
                peak_bytes: source_peak,
            })?;
        closed.close_peak = closed.close_peak.max(retained);
        ceiling(
            FormulaResource::SupportBytes,
            total,
            limits.max_support_bytes as u128,
            location,
        )
        .map_err(|failure| SourceCloseFailure {
            failure,
            peak_bytes: closed.close_peak,
        })?;
        Ok(closed)
    }

    /// Retained non-canonical header and component capacities. Excludes caller
    /// frames, output occurrence maps and every workspace lease.
    pub(crate) fn metadata_bytes(&self) -> u128 {
        (size_of::<Self>() - size_of::<ClosedCatalog>()) as u128
            + self.components.as_ref().map_or(0, |components| {
                components.storage_bytes() - size_of::<TemplateComponents>() as u128
            })
    }

    /// This returned owner's full named retained storage, including its closed
    /// canonical base exactly once. External published catalogs remain separate.
    pub(crate) fn storage_bytes(&self) -> u128 {
        self.storage.storage_bytes() + self.metadata_bytes()
    }
}

/// Original typed refusal plus actual source-only close capacity. Required but
/// unallocated proposals never increase this receipt. Callback unwind returns no
/// receipt; the cumulative counter guard still preserves accepted work.
#[derive(Debug)]
pub(crate) struct SourceCloseFailure {
    failure: FormulaFailure,
    peak_bytes: u128,
}
impl SourceCloseFailure {
    pub(crate) fn into_parts(self) -> (FormulaFailure, u128) {
        (self.failure, self.peak_bytes)
    }
}

impl CompletedCatalog {
    /// Close this exact original source owner after all output selections have
    /// been published. External bytes exclude both this catalog and the complete
    /// account workspace; both are added here. Old support indexes stay charged
    /// through the close attempt, then only compiled components survive.
    pub(crate) fn into_closed(
        self,
        external_bytes: u128,
        work: GroundingWork<'_>,
    ) -> Result<ClosedSource, SourceCloseFailure> {
        self.catalog.into_closed(external_bytes, work)
    }
}

impl SupportCatalog {
    fn into_closed(
        self,
        external_bytes: u128,
        work: GroundingWork<'_>,
    ) -> Result<ClosedSource, SourceCloseFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let source_bytes = self.bytes(location).map_err(|failure| SourceCloseFailure {
            failure,
            // No allocation has occurred. The canonical owner is still readable.
            peak_bytes: self.owner.storage_bytes(),
        })? as u128;
        let refuse = |failure| SourceCloseFailure {
            failure,
            peak_bytes: source_bytes,
        };
        counters.work(limits, location).map_err(refuse)?;
        let metadata = source_bytes - self.owner.storage_bytes();
        let external = external_bytes
            .checked_add(counters.workspace_bytes() as u128)
            .ok_or_else(|| refuse(overflow(location)))?;
        let outer = external
            .checked_add(metadata)
            .ok_or_else(|| refuse(overflow(location)))?;
        let mut checked = owner_limits(limits, outer, location).map_err(refuse)?;
        let atom_limit = limits.max_atom_storage_bytes as u128;
        let atom_bound = atom_limit < checked.max_bytes;
        checked.max_bytes = checked.max_bytes.min(atom_limit);
        let result = self
            .owner
            .into_closed_with(checked, || counters.work(limits, location));
        let (storage, owner_peak) = match result {
            Ok(storage) => {
                let peak = storage.publication_peak_bytes();
                (Ok(storage), peak)
            }
            Err(error) => {
                let (error, peak) = error.into_parts();
                let failure = match error {
                    atom_interner::Failure::Bytes { required, .. } if atom_bound => {
                        FormulaFailure::Limit {
                            resource: FormulaResource::AtomStorageBytes,
                            observed: required,
                            limit: atom_limit,
                            location,
                        }
                    }
                    other => atom_failure(other, limits, outer, location),
                };
                (Err(failure), peak)
            }
        };
        let source_peak = metadata
            .checked_add(owner_peak)
            .ok_or_else(|| refuse(overflow(location)))?
            .max(source_bytes);
        let total_peak = external
            .checked_add(source_peak)
            .ok_or_else(|| SourceCloseFailure {
                failure: overflow(location),
                peak_bytes: source_peak,
            })?;
        counters.record(Event::SupportPeakBytes(total_peak));
        let storage = storage.map_err(|failure| SourceCloseFailure {
            failure,
            peak_bytes: source_peak,
        })?;
        ceiling(
            FormulaResource::SupportBytes,
            total_peak,
            limits.max_support_bytes as u128,
            location,
        )
        .map_err(|failure| SourceCloseFailure {
            failure,
            peak_bytes: source_peak,
        })?;
        // Retire support-only metadata before admitting the returned envelope.
        // Components move unchanged; canonical payload is never recopied.
        let components = self.components;
        drop((
            self.scope,
            self.rows,
            self.pending,
            self.supported,
            self.growth,
        ));
        ClosedSource::admit(storage, components, source_peak, external, limits, location)
    }
}

fn overflow(location: Location) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Overflow,
        location,
    }
}

#[cfg(test)]
mod tests;
