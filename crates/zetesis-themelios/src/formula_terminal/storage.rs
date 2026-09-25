//! Reconstruction composes existing owner receipts with one leased workspace.

use themelios_base::span::Location;
use zetesis_core::atom_interner::{AtomInterner, Failure, Limits};
use zetesis_core::catalog::Error;

use crate::formula::ceiling;
use crate::formula_support::{
    Counters, GroundingWork, StorageLease, atom_failure, owner_limits, reserve_exact_scoped,
};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

pub(super) struct Work<'a> {
    pub(super) limits: &'a FormulaLimits,
    pub(super) counters: &'a mut Counters,
    pub(super) location: Location,
    /// All retained owners except this writer and the leased workspace.
    pub(super) external: u128,
}

impl Work<'_> {
    pub(super) fn permit(&mut self) -> Result<(), FormulaFailure> {
        self.counters.work(self.limits, self.location)
    }

    fn outside(&self) -> Result<u128, FormulaFailure> {
        self.external
            .checked_add(self.counters.workspace_bytes() as u128)
            .ok_or_else(|| overflow(self.location))
    }

    pub(super) fn remaining(&self, owner_bytes: u128) -> Result<usize, FormulaFailure> {
        let live = self
            .outside()?
            .checked_add(owner_bytes)
            .ok_or_else(|| overflow(self.location))?;
        ceiling(
            FormulaResource::SupportBytes,
            live,
            self.limits.max_support_bytes as u128,
            self.location,
        )?;
        Ok(self.limits.max_support_bytes - usize::try_from(live).expect("admitted usize ceiling"))
    }

    pub(super) fn observe(&self, owner_peak: u128) -> Result<(), FormulaFailure> {
        let live = self
            .outside()?
            .checked_add(owner_peak)
            .ok_or_else(|| overflow(self.location))?;
        ceiling(
            FormulaResource::SupportBytes,
            live,
            self.limits.max_support_bytes as u128,
            self.location,
        )
    }

    /// The owner authenticates its own closed prefix. The caller supplies only
    /// the remaining named storage allowance, never an alternative term owner.
    pub(super) fn writer(
        &mut self,
        base: &zetesis_core::atom_interner::ClosedCatalog,
    ) -> Result<AtomInterner, FormulaFailure> {
        self.permit()?;
        let outside = self.outside()?;
        let remaining = self.remaining(0)?;
        let atom_bound = self.limits.max_atom_storage_bytes < remaining;
        let remaining = remaining.min(self.limits.max_atom_storage_bytes);
        AtomInterner::for_closed_catalog(base, remaining).map_err(|error| {
            let failure = Failure::<FormulaFailure>::Catalog(error);
            self.failure(failure, outside, atom_bound)
        })
    }

    /// Check the same actual post-operation peak on success and refusal.
    /// A refused proposed allocation is never reported as retained capacity.
    pub(super) fn atoms<T>(
        &mut self,
        writer: &mut AtomInterner,
        operation: impl FnOnce(
            &mut AtomInterner,
            Limits,
            &mut Counters,
        ) -> Result<T, Failure<FormulaFailure>>,
    ) -> Result<T, FormulaFailure> {
        let outside = self.outside()?;
        let mut limits = owner_limits(self.limits, outside, self.location)?;
        let atom_bound = (self.limits.max_atom_storage_bytes as u128) < limits.max_bytes;
        limits.max_bytes = limits
            .max_bytes
            .min(self.limits.max_atom_storage_bytes as u128);
        writer.restart_storage_peak();
        let result = operation(writer, limits, self.counters);
        let observed = self.observe(writer.storage_peak_bytes());
        let value = result.map_err(|error| self.failure(error, outside, atom_bound))?;
        observed?;
        Ok(value)
    }

    fn failure(
        &self,
        error: Failure<FormulaFailure>,
        outside: u128,
        atom_bound: bool,
    ) -> FormulaFailure {
        match error {
            Failure::Bytes { required, .. } | Failure::Catalog(Error::Storage { required, .. }) => {
                if atom_bound {
                    FormulaFailure::Limit {
                        resource: FormulaResource::AtomStorageBytes,
                        observed: required,
                        limit: self.limits.max_atom_storage_bytes as u128,
                        location: self.location,
                    }
                } else {
                    outside.checked_add(required).map_or_else(
                        || overflow(self.location),
                        |observed| FormulaFailure::Limit {
                            resource: FormulaResource::SupportBytes,
                            observed,
                            limit: self.limits.max_support_bytes as u128,
                            location: self.location,
                        },
                    )
                }
            }
            error => atom_failure(error, self.limits, outside, self.location),
        }
    }

    pub(super) fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        lease: &mut StorageLease,
        header_and_other_bytes: usize,
        writer: &AtomInterner,
    ) -> Result<(), FormulaFailure> {
        let external = self
            .external
            .checked_add(writer.storage_bytes())
            .ok_or_else(|| overflow(self.location))?;
        reserve_exact_scoped(
            values,
            additional,
            lease,
            header_and_other_bytes,
            external,
            GroundingWork::new(self.limits, self.counters, self.location),
        )
    }
}

pub(super) fn overflow(location: Location) -> FormulaFailure {
    FormulaFailure::AtomCatalog {
        error: Error::Overflow,
        location,
    }
}
