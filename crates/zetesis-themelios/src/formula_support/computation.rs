//! Typed term computation uses the support vocabulary, independently of truth.

use crate::formula_support::GroundingWork;
mod atoms;
mod static_components;
#[cfg(test)]
mod tests;

use themelios_base::span::Location;
use zetesis_core::ValueNodeRef;
use zetesis_core::atom_interner::{AssignedFailure, TermLookup};
use zetesis_core::catalog::{
    AssignmentError, AssignmentFailure, AssignmentSlice, CatalogRead, Error, TermKey, TermRef,
};

use super::{Counters, Support, relations::SupportAppend, storage::StorageLease};
use crate::formula::ceiling;
use crate::grounding_observer::Event;
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

/// A query's mutable term capability and immutable relation membership are
/// separate. Frames retain scoped IDs; each operation borrows a current reader
/// only until it has resolved its inputs. Admitting a term changes no truth.
pub(crate) struct Computation<'a, 'source> {
    terms: Terms<'a, 'source>,
    support: &'a Support<'source>,
}

enum Terms<'a, 'source> {
    Append(&'a mut SupportAppend<'source>),
    Frozen(TermLookup<'source>),
}

impl<'a, 'source> Computation<'a, 'source> {
    pub(crate) fn new(
        append: &'a mut SupportAppend<'source>,
        support: &'a Support<'source>,
    ) -> Self {
        Self {
            terms: Terms::Append(append),
            support,
        }
    }

    pub(crate) fn frozen(lookup: TermLookup<'source>, support: &'a Support<'source>) -> Self {
        Self {
            terms: Terms::Frozen(lookup),
            support,
        }
    }

    pub(crate) fn read(&self) -> CatalogRead<'_> {
        match &self.terms {
            Terms::Append(append) => append.read(),
            Terms::Frozen(lookup) => lookup.read(),
        }
    }

    pub(crate) fn lease(&self) -> StorageLease {
        self.support.workspace().lease()
    }

    /// The returned allowance applies to one leased container. Every other
    /// live lease, relation/query allocation and canonical byte remains charged.
    pub(crate) fn allowance(
        &self,
        lease: &StorageLease,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        self.check_lease(lease, location)?;
        let total = self.support.live_bytes();
        ceiling(
            FormulaResource::SupportBytes,
            total as u128,
            limits.max_support_bytes as u128,
            location,
        )?;
        Ok(limits.max_support_bytes - (total - lease.bytes()))
    }

    /// Record actual capacity even after an operation refused. Growing a buffer
    /// temporarily retains its previous capacity; headers count once.
    pub(crate) fn storage_observed(
        &self,
        lease: &StorageLease,
        previous: usize,
        header: usize,
        limits: &FormulaLimits,
        counters: &Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        self.check_lease(lease, location)?;
        let old = if lease.bytes() > previous {
            previous.saturating_sub(header)
        } else {
            0
        };
        let peak = self.support.live_bytes() as u128 + old as u128;
        counters.record(Event::SupportPeakBytes(peak));
        ceiling(
            FormulaResource::SupportBytes,
            peak,
            limits.max_support_bytes as u128,
            location,
        )
    }

    /// Record a component owner's explicit capacity peak, counting all other
    /// current workspace and canonical storage once.
    pub(crate) fn storage_peak(
        &self,
        lease: &StorageLease,
        component_peak: u128,
        limits: &FormulaLimits,
        counters: &Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        self.check_lease(lease, location)?;
        let peak = (self.support.live_bytes() - lease.bytes()) as u128 + component_peak;
        counters.record(Event::SupportPeakBytes(peak));
        ceiling(
            FormulaResource::SupportBytes,
            peak,
            limits.max_support_bytes as u128,
            location,
        )
    }

    /// Planner scratch is local to one preparation call. Its cumulative
    /// capacity includes any retained result until that result moves into the
    /// join's lease. Check proposals without recording unallocated capacity;
    /// compose successful allocations with all currently live support storage.
    pub(super) fn preparation_capacity(
        &self,
        capacity: super::order::Capacity,
        limits: &FormulaLimits,
        counters: &Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let (bytes, allocated) = match capacity {
            super::order::Capacity::Requested(bytes) => (bytes, false),
            super::order::Capacity::Allocated(bytes) => (bytes, true),
        };
        let total = self.support.live_bytes() as u128 + bytes;
        if allocated {
            counters.record(Event::SupportPeakBytes(total));
        }
        ceiling(
            FormulaResource::SupportBytes,
            total,
            limits.max_support_bytes as u128,
            location,
        )
    }

    pub(crate) fn storage_result<T>(
        &self,
        result: Result<T, AssignmentFailure<FormulaFailure>>,
        lease: &StorageLease,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<T, FormulaFailure> {
        self.check_lease(lease, location)?;
        result.map_err(|error| match error {
            AssignmentFailure::Assignment(AssignmentError::Storage(Error::Storage {
                required,
                ..
            })) => {
                let observed = self.support.live_bytes() as u128 - lease.bytes() as u128 + required;
                FormulaFailure::Limit {
                    resource: FormulaResource::SupportBytes,
                    observed,
                    limit: limits.max_support_bytes as u128,
                    location,
                }
            }
            error => crate::formula_binding::failure(error, location),
        })
    }

    fn check_lease(&self, lease: &StorageLease, location: Location) -> Result<(), FormulaFailure> {
        if self.support.workspace().owns(lease) {
            Ok(())
        } else {
            Err(crate::formula_binding::assignment(
                AssignmentError::Read(zetesis_core::catalog::ReadError::ForeignCatalog),
                location,
            ))
        }
    }

    pub(crate) fn import(
        &mut self,
        value: TermRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<TermKey, FormulaFailure> {
        match &mut self.terms {
            Terms::Append(append) => append.import(
                value,
                self.support.workspace_bytes(),
                limits,
                counters,
                location,
            ),
            Terms::Frozen(lookup) => {
                let outer = self.support.live_bytes() as u128 - lookup.storage_bytes();
                let checked = super::relations::owner_limits(limits, outer, location)?;
                lookup.restart_storage_peak();
                let result = lookup.find_term_with(
                    value,
                    zetesis_core::catalog::Limits {
                        max_nodes: usize::MAX,
                        max_depth: usize::MAX,
                        max_bytes: usize::MAX,
                    },
                    checked,
                    || counters.work(limits, location),
                );
                counters.record(Event::SupportPeakBytes(outer + lookup.storage_peak()));
                result
                    .map_err(|error| {
                        super::relations::atom_failure(error, limits, outer, location)
                    })?
                    .ok_or(FormulaFailure::UnadmittedTerm { location })
            }
        }
    }

    pub(crate) fn construct(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        values: AssignmentSlice<'_>,
        children: &[usize],
        term_limits: zetesis_core::catalog::Limits,
        work: GroundingWork<'_>,
    ) -> Result<TermKey, FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        match &mut self.terms {
            Terms::Append(append) => append.construct(
                descriptor,
                values,
                children,
                term_limits,
                self.support.workspace_bytes(),
                GroundingWork::new(limits, counters, location),
            ),
            Terms::Frozen(lookup) => {
                let outer = self.support.live_bytes() as u128 - lookup.storage_bytes();
                let checked = super::relations::owner_limits(limits, outer, location)?;
                lookup.restart_storage_peak();
                let result = lookup.find_constructed_with(
                    descriptor,
                    values,
                    children,
                    term_limits,
                    checked,
                    || counters.work(limits, location),
                );
                counters.record(Event::SupportPeakBytes(outer + lookup.storage_peak()));
                result
                    .map_err(|error| match error {
                        AssignedFailure::Assignment(error) => {
                            crate::formula_binding::assignment(error, location)
                        }
                        AssignedFailure::Interner(error) => {
                            super::relations::atom_failure(error, limits, outer, location)
                        }
                    })?
                    .ok_or(FormulaFailure::UnadmittedTerm { location })
            }
        }
    }

    pub(crate) fn number(
        &mut self,
        value: i32,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<TermKey, FormulaFailure> {
        let empty = self.read().assignment();
        self.construct(
            ValueNodeRef::Number(value),
            empty.as_slice(),
            &[],
            zetesis_core::catalog::Limits::default(),
            GroundingWork::new(limits, counters, location),
        )
    }
}
