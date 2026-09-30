//! Objective component rows share the source vocabulary and its storage ledger.

use crate::formula_support::{Context, GroundingWork};

use themelios_base::span::Location;
use zetesis_core::catalog::CatalogRead;
use zetesis_core::{
    AtomCatalog, FilterRef, PatternRef, TemplateCatalog, TemplateCatalogFailure,
    TemplateCatalogSelection, TemplateTerm,
};

use crate::formula_support::{Computation, Counters, Publication, StorageLease};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

pub(super) struct Rows {
    selection: TemplateCatalogSelection,
    lease: StorageLease,
}
impl Rows {
    pub(super) fn new(
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        let mut lease = computation.lease();
        let allowance = computation.allowance(&lease, limits, location)?;
        let extra = header_extra();
        let selection =
            TemplateCatalogSelection::new(computation.read(), allowance.saturating_sub(extra))
                .map_err(|error| FormulaFailure::TemplateCatalog {
                    error: with_header(error, extra, allowance),
                    location,
                })?;
        lease.observe(
            metadata_bytes(selection.storage_bytes() + extra as u128, limits, location)?,
            location,
        )?;
        computation.storage_peak(
            &lease,
            selection.storage_peak_bytes() + extra as u128,
            limits,
            counters,
            location,
        )?;
        Ok(Self { selection, lease })
    }
    pub(super) fn append<'a>(
        &mut self,
        read: CatalogRead<'_>,
        terms: impl IntoIterator<Item = TemplateTerm<'a>>,
        patterns: impl IntoIterator<Item = PatternRef<'a>>,
        filters: impl IntoIterator<Item = FilterRef<'a>>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<usize, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let allowance = computation.allowance(&self.lease, limits, location)?;
        let extra = header_extra();
        self.selection.restart_storage_peak();
        let result = self.selection.append_with(
            read,
            terms,
            patterns,
            filters,
            allowance.saturating_sub(extra),
            || counters.work(limits, location),
        );
        self.lease.observe(
            metadata_bytes(
                self.selection.storage_bytes() + extra as u128,
                limits,
                location,
            )?,
            location,
        )?;
        let observed = computation.storage_peak(
            &self.lease,
            self.selection.storage_peak_bytes() + extra as u128,
            limits,
            counters,
            location,
        );
        let position =
            result.map_err(|error| failure(with_header(error, extra, allowance), location))?;
        observed?;
        Ok(position)
    }
    pub(super) fn publish(
        self,
        atoms: AtomCatalog,
        publication: &mut Publication<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<TemplateCatalog, FormulaFailure> {
        let Self {
            selection,
            mut lease,
        } = self;
        publication.check_lease(&lease, counters, location)?;
        let allowance = publication
            .remaining_bytes(limits, counters, location)?
            .checked_add(lease.bytes())
            .ok_or(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                observed: u128::MAX,
                limit: limits.max_support_bytes as u128,
                location,
            })?;
        let publication_extra = selection.publication_peak_bytes() - selection.storage_bytes();
        let extra = header_extra();
        let catalog = selection
            .finish_with(atoms, allowance.saturating_sub(extra), || {
                counters.work(limits, location)
            })
            .map_err(|error| failure(with_header(error, extra, allowance), location))?;
        publication.observe_peak(publication_extra, limits, counters, location)?;
        let bytes = catalog.metadata_bytes();
        lease.observe(
            usize::try_from(bytes).map_err(|_| FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                observed: bytes,
                limit: limits.max_support_bytes as u128,
                location,
            })?,
            location,
        )?;
        publication.retain(lease, limits, counters, location)?;
        Ok(catalog)
    }
}
fn failure(error: TemplateCatalogFailure<FormulaFailure>, location: Location) -> FormulaFailure {
    let error = match error {
        TemplateCatalogFailure::Stopped(error) => return error,
        TemplateCatalogFailure::Read(error) => TemplateCatalogFailure::Read(error),
        TemplateCatalogFailure::Storage(error) => TemplateCatalogFailure::Storage(error),
        TemplateCatalogFailure::Incomplete => TemplateCatalogFailure::Incomplete,
    };
    FormulaFailure::TemplateCatalog { error, location }
}

fn metadata_bytes(
    bytes: u128,
    limits: &FormulaLimits,
    location: Location,
) -> Result<usize, FormulaFailure> {
    usize::try_from(bytes).map_err(|_| FormulaFailure::Limit {
        resource: FormulaResource::SupportBytes,
        observed: bytes,
        limit: limits.max_support_bytes as u128,
        location,
    })
}

// The core selection already counts its own header. The frontend receipt also
// retains its inline lease; publication later replaces both with catalog metadata.
fn header_extra() -> usize {
    size_of::<Rows>() - size_of::<TemplateCatalogSelection>()
}

fn with_header<E>(
    error: TemplateCatalogFailure<E>,
    extra: usize,
    allowance: usize,
) -> TemplateCatalogFailure<E> {
    match error {
        TemplateCatalogFailure::Storage(zetesis_core::catalog::Error::Storage {
            required, ..
        }) => TemplateCatalogFailure::Storage(zetesis_core::catalog::Error::Storage {
            required: required + extra as u128,
            limit: allowance,
        }),
        error => error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula_support::{SourceSelection, testing::Fixture};

    use crate::test_support::location;
    use zetesis_core::Value;

    fn live(computation: &Computation<'_, '_>, limits: &FormulaLimits) -> usize {
        let probe = computation.lease();
        limits.max_support_bytes - computation.allowance(&probe, limits, location()).unwrap()
    }

    #[test]
    fn append_combines_only_the_current_storage_interval() {
        Fixture::default().with(location(), |_, computation, counters| {
            let mut limits = FormulaLimits::default();
            let mut rows = Rows::new(computation, &limits, counters, location()).unwrap();
            for _ in 0..5 {
                rows.append(
                    computation.read(),
                    [],
                    [],
                    [],
                    Context::new(computation, &limits, counters, location()),
                )
                .unwrap();
            }
            assert!(rows.selection.storage_peak_bytes() > rows.selection.storage_bytes());
            let previous = live(computation, &limits);
            let later = Value::String("later spelling".repeat(512));
            computation
                .import((&later).into(), &limits, counters, location())
                .unwrap();
            assert!(live(computation, &limits) > previous);
            limits.max_support_bytes = live(computation, &limits);
            rows.append(
                computation.read(),
                [],
                [],
                [],
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap();
            assert_eq!(rows.selection.len(), 6);
            assert_eq!(live(computation, &limits), limits.max_support_bytes);
        });
    }

    #[test]
    fn row_receipt_includes_the_frontend_header() {
        Fixture::default().with(location(), |_, computation, counters| {
            let limits = FormulaLimits::default();
            let before = live(computation, &limits);
            let mut rows = Rows::new(computation, &limits, counters, location()).unwrap();
            assert_eq!(live(computation, &limits) - before, size_of::<Rows>());
            rows.append(
                computation.read(),
                [],
                [],
                [],
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap();
            assert_eq!(
                rows.lease.bytes() as u128,
                rows.selection.storage_bytes() + header_extra() as u128
            );
            drop(rows);
            assert_eq!(live(computation, &limits), before);
        });
    }

    #[test]
    fn publication_replaces_the_rows_header_receipt() {
        let mut fixture = Fixture::default();
        let limits = FormulaLimits::default();
        let (rows, selection) = fixture.with(location(), |_, computation, counters| {
            let mut rows = Rows::new(computation, &limits, counters, location()).unwrap();
            rows.append(
                computation.read(),
                [],
                [],
                [],
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap();
            let selection =
                SourceSelection::new(computation, &limits, counters, location()).unwrap();
            (rows, selection)
        });
        let (mut completed, mut counters) = fixture.finish(location());
        let mut publication = Publication::new(&mut completed, &counters, location()).unwrap();
        let atoms = publication
            .atoms(selection, &limits, &mut counters, location())
            .unwrap();
        let before = limits.max_support_bytes
            - publication
                .remaining_bytes(&limits, &counters, location())
                .unwrap();
        let previous = rows.lease.bytes();
        let catalog = rows
            .publish(atoms, &mut publication, &limits, &mut counters, location())
            .unwrap();
        let after = limits.max_support_bytes
            - publication
                .remaining_bytes(&limits, &counters, location())
                .unwrap();
        assert_eq!(
            after as u128,
            (before - previous) as u128 + catalog.metadata_bytes()
        );
    }
}
