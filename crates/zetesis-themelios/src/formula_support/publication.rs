//! Final views share one sealed source vocabulary and retain only occurrence maps.

use themelios_base::span::Location;
use zetesis_core::AtomCatalog;

use super::{CompletedCatalog, Counters, SourceSelection, StorageLease};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

/// Keep earlier output maps charged while subsequent views are constructed.
/// This preparation lease ends when all finished outputs leave source admission;
/// it neither mutates their shared payload nor becomes a checker's workspace.
pub(crate) struct Publication<'a> {
    source: &'a mut CompletedCatalog,
    lease: StorageLease,
}

impl<'a> Publication<'a> {
    /// Source identity and workspace ownership are independent. Authenticate
    /// both receipts before using the shared total to admit any replacement.
    pub(crate) fn check_lease(
        &self,
        lease: &StorageLease,
        counters: &Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        if !counters.accounting.workspace.owns(&self.lease)
            || !counters.accounting.workspace.owns(lease)
        {
            return Err(crate::formula_binding::assignment(
                zetesis_core::catalog::AssignmentError::Read(
                    zetesis_core::catalog::ReadError::ForeignCatalog,
                ),
                location,
            ));
        }
        Ok(())
    }

    fn current_bytes(
        &self,
        counters: &Counters,
        location: Location,
    ) -> Result<u128, FormulaFailure> {
        self.check_lease(&self.lease, counters, location)?;
        Ok(self.source.catalog.bytes(location)? as u128
            + counters.accounting.workspace.bytes() as u128)
    }

    /// Canonical/support storage outside this account's complete workspace.
    pub(crate) fn source_bytes(&self, location: Location) -> Result<usize, FormulaFailure> {
        self.source.catalog.bytes(location)
    }

    pub(crate) fn remaining_bytes(
        &self,
        limits: &FormulaLimits,
        counters: &Counters,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        let bytes = self.current_bytes(counters, location)?;
        crate::formula::ceiling(
            FormulaResource::SupportBytes,
            bytes,
            limits.max_support_bytes as u128,
            location,
        )?;
        Ok(limits.max_support_bytes - usize::try_from(bytes).expect("admitted usize ceiling"))
    }

    /// Record a new envelope while its input header and transferred buffer are
    /// still retained. The constructor must have admitted this overlap first.
    pub(crate) fn observe_peak(
        &self,
        extra: u128,
        limits: &FormulaLimits,
        counters: &Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let bytes = self.current_bytes(counters, location)? + extra;
        counters.record(crate::grounding_observer::Event::SupportPeakBytes(bytes));
        crate::formula::ceiling(
            FormulaResource::SupportBytes,
            bytes,
            limits.max_support_bytes as u128,
            location,
        )
    }

    /// Keep already admitted output storage live until all source outputs finish.
    /// Transferring its receipt neither copies payload nor invents growth overlap.
    pub(crate) fn retain(
        &mut self,
        lease: StorageLease,
        limits: &FormulaLimits,
        counters: &Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        self.check_lease(&lease, counters, location)?;
        self.remaining_bytes(limits, counters, location)?;
        lease.transfer_to(&mut self.lease, location)
    }

    pub(crate) fn new(
        source: &'a mut CompletedCatalog,
        counters: &Counters,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        let mut lease = counters.accounting.workspace.lease();
        lease.observe(size_of::<Self>(), location)?;
        Ok(Self { source, lease })
    }

    /// Transfer only retained output receipts. This publication coordinator no
    /// longer exists after the move; its caller admits its own fixed envelope.
    pub(crate) fn into_lease(mut self, location: Location) -> Result<StorageLease, FormulaFailure> {
        self.lease
            .observe(self.lease.bytes() - size_of::<Self>(), location)?;
        Ok(self.lease)
    }

    /// Supplied occurrence order is preserved exactly. A selected view cannot
    /// change support membership; its logical population has its own count bound.
    pub(crate) fn atoms(
        &mut self,
        selection: SourceSelection,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<AtomCatalog, FormulaFailure> {
        self.check_lease(selection.storage_lease(), counters, location)?;
        let catalog = self.source.catalog.publish_selection(
            selection.scope(),
            selection.positions(),
            counters.accounting.workspace.bytes(),
            limits,
            counters,
            location,
        )?;
        let bytes = self.lease.bytes() as u128 + catalog.publication_bytes();
        let bytes = usize::try_from(bytes).map_err(|_| FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            observed: bytes,
            limit: limits.max_support_bytes as u128,
            location,
        })?;
        self.lease.observe(bytes, location)?;
        // The publication operation admitted this map while the selection was
        // still live. Drop that selection only after recording its replacement.
        drop(selection);
        Ok(catalog)
    }
}
