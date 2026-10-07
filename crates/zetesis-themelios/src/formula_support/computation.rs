//! Typed term computation uses the support vocabulary, independently of truth.

use crate::formula_support::GroundingWork;
mod atoms;
mod static_components;
#[cfg(test)]
mod tests;

use crate::ProgramSite;
use zetesis_core::ValueNodeRef;
use zetesis_core::atom_interner::{AssignedFailure, TermLookup};
use zetesis_core::catalog::{
    AssignmentError, AssignmentFailure, AssignmentSlice, CatalogRead, Error, TermKey, TermRef,
};

use super::{
    Counters, Support,
    relations::SupportAppend,
    storage::{Scope, StorageLease},
};
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
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        let storage = self.storage(lease, location)?;
        ceiling(
            FormulaResource::SupportBytes,
            storage.live_bytes(),
            limits.max_support_bytes as u128,
            location,
        )?;
        storage.allowance(limits, location)
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
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        self.storage(lease, location)?
            .observed(previous, header, limits, counters, location)
    }

    /// Record a component owner's explicit capacity peak, counting all other
    /// current workspace and canonical storage once.
    pub(crate) fn storage_peak(
        &self,
        lease: &StorageLease,
        component_peak: u128,
        limits: &FormulaLimits,
        counters: &Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        self.storage(lease, location)?
            .peak(component_peak, limits, counters, location)
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
        location: ProgramSite,
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
        location: ProgramSite,
    ) -> Result<T, FormulaFailure> {
        self.check_lease(lease, location)?;
        // Successful operations and non-storage refusals require no capacity
        // traversal. Preserve that fast path while sharing byte-error mapping.
        if !matches!(
            &result,
            Err(AssignmentFailure::Assignment(AssignmentError::Storage(
                Error::Storage { .. }
            )))
        ) {
            return result.map_err(|error| crate::formula_binding::failure(error, location));
        }
        self.storage(lease, location)?
            .result(result, limits, location)
    }

    /// Named support storage outside the entire shared workspace. Authentication
    /// precedes capacity reads; a reservation cannot borrow another owner's lease.
    pub(super) fn external_bytes(
        &self,
        lease: &StorageLease,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        self.check_lease(lease, location)?;
        Ok(self.support.live_bytes() - self.support.workspace().bytes())
    }

    fn storage<'b>(
        &'b self,
        lease: &'b StorageLease,
        location: ProgramSite,
    ) -> Result<Scope<'b>, FormulaFailure> {
        let external = self.external_bytes(lease, location)?;
        self.support
            .workspace()
            .scope(lease, external)
            .map_err(|error| {
                crate::formula_binding::assignment(AssignmentError::Read(error), location)
            })
    }

    fn check_lease(
        &self,
        lease: &StorageLease,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
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
        location: ProgramSite,
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

    /// Find an exact constructor instance without changing the vocabulary.
    /// Absence is a normal negative query; all ownership and resource failures
    /// remain typed. The mutable borrow ends the lookup before any later append.
    pub(super) fn find_constructed(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        values: AssignmentSlice<'_>,
        children: &[usize],
        work: GroundingWork<'_>,
    ) -> Result<Option<TermKey>, FormulaFailure> {
        let live = self.support.live_bytes() as u128;
        match &mut self.terms {
            Terms::Append(append) => {
                let mut lookup = append.term_lookup();
                let outer = live - lookup.storage_bytes() + size_of::<TermLookup<'_>>() as u128;
                find_constructed(&mut lookup, outer, descriptor, values, children, work)
            }
            Terms::Frozen(lookup) => {
                let outer = live - lookup.storage_bytes();
                find_constructed(lookup, outer, descriptor, values, children, work)
            }
        }
    }

    pub(crate) fn number(
        &mut self,
        value: i32,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
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

/// The same immutable lookup and failure mapping serves append and frozen lanes.
fn find_constructed(
    lookup: &mut TermLookup<'_>,
    outer: u128,
    descriptor: ValueNodeRef<'_>,
    values: AssignmentSlice<'_>,
    children: &[usize],
    work: GroundingWork<'_>,
) -> Result<Option<TermKey>, FormulaFailure> {
    let GroundingWork {
        limits,
        counters,
        location,
    } = work;
    let checked = super::relations::owner_limits(limits, outer, location)?;
    lookup.restart_storage_peak();
    let result = lookup.find_constructed_with(
        descriptor,
        values,
        children,
        zetesis_core::catalog::Limits {
            max_nodes: usize::MAX,
            max_depth: usize::MAX,
            max_bytes: usize::MAX,
        },
        checked,
        || counters.work(limits, location),
    );
    counters.record(Event::SupportPeakBytes(outer + lookup.storage_peak()));
    result.map_err(|error| match error {
        AssignedFailure::Assignment(error) => crate::formula_binding::assignment(error, location),
        AssignedFailure::Interner(error) => {
            super::relations::atom_failure(error, limits, outer, location)
        }
    })
}
