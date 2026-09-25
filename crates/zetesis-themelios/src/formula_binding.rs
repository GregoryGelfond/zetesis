//! Scoped assignments retain only canonical IDs and explicit absence.

use themelios_base::span::Location;
use zetesis_core::catalog::{
    AssignmentError, AssignmentFailure, AssignmentSlice, CatalogRead, TermAssignment, TermKey,
    TermRef,
};
use zetesis_core::{BindingView, TemplateTerm};

use crate::formula_support::{Computation, Counters, StorageLease};
use crate::{FormulaFailure, FormulaLimits};

/// Variable coordinates belong to the compiled source scope. Term coordinates
/// belong to one canonical vocabulary, retained once per owned frame. A prefix
/// borrows that same metadata; neither form owns logical payload.
#[derive(Debug)]
pub(crate) struct Binding<'a> {
    slots: Slots<'a>,
}

#[derive(Debug)]
enum Slots<'a> {
    Owned {
        values: TermAssignment,
        lease: StorageLease,
    },
    Borrowed(AssignmentSlice<'a>),
}

impl Binding<'static> {
    pub(crate) fn new(
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        let values = computation.read().assignment();
        let mut lease = computation.lease();
        lease.observe(size_of::<Self>(), location)?;
        computation.storage_observed(&lease, 0, size_of::<Self>(), limits, counters, location)?;
        Ok(Self {
            slots: Slots::Owned { values, lease },
        })
    }

    /// The source and destination coexist in the same workspace ledger.
    /// Copying copies IDs only; failure still records the reserved capacity.
    pub(crate) fn copy_slots(
        source: AssignmentSlice<'_>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        let mut result = Self::new(computation, limits, counters, location)?;
        result.change_storage(
            computation,
            limits,
            counters,
            location,
            |values, max_bytes, before| values.copy_from_with(source, max_bytes, before),
        )?;
        Ok(result)
    }

    fn owned(&mut self) -> &mut TermAssignment {
        match &mut self.slots {
            Slots::Owned { values, .. } => values,
            Slots::Borrowed(_) => {
                unreachable!("retained bindings are constructed with owned metadata")
            }
        }
    }

    /// An unevaluated head suffix may not exist yet. Clearing it is a no-op;
    /// an existing output becomes absent before the cursor backtracks.
    pub(crate) fn clear(
        &mut self,
        variable: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        if variable >= self.len() {
            return Ok(());
        }
        self.owned()
            .clear_with(variable, || counters.work(limits, location))
            .map_err(|error| failure(error, location))
    }

    /// Prepermit a complete undo before changing any slot. A stopped undo
    /// leaves both the assignment and its trail available for the next call.
    pub(crate) fn clear_trail(
        &mut self,
        trail: &mut Vec<usize>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        for &slot in trail.iter() {
            counters.work(limits, location)?;
            self.slots()
                .is_bound(slot)
                .map_err(|error| assignment(error, location))?;
        }
        for slot in trail.drain(..) {
            self.owned()
                .clear_with(slot, || Ok::<_, FormulaFailure>(()))
                .map_err(|error| failure(error, location))?;
        }
        Ok(())
    }

    pub(crate) fn swap(
        &mut self,
        left: usize,
        right: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        self.owned()
            .swap_with(left, right, || counters.work(limits, location))
            .map_err(|error| failure(error, location))
    }

    pub(crate) fn set(
        &mut self,
        variable: usize,
        value: &TermKey,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        self.owned()
            .set_with(variable, value, || counters.work(limits, location))
            .map_err(|error| failure(error, location))
    }

    pub(crate) fn extend_scope(
        &mut self,
        end: usize,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        if end <= self.len() {
            return Ok(());
        }
        self.change_storage(
            computation,
            limits,
            counters,
            location,
            |values, max_bytes, before| values.resize_with(end, max_bytes, before),
        )
    }

    pub(crate) fn truncate(
        &mut self,
        end: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        self.owned()
            .truncate_with(end, || counters.work(limits, location))
            .map_err(|error| failure(error, location))
    }

    /// Roll back provisional ID slots after a refused publication. Capacity
    /// and its lease remain live; no semantic payload is dropped or copied.
    pub(crate) fn discard_suffix(&mut self, end: usize) {
        self.owned()
            .truncate_with(end, || Ok::<_, std::convert::Infallible>(()))
            .expect("rollback restores an earlier frame extent");
    }

    fn change_storage(
        &mut self,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
        operation: impl FnOnce(
            &mut TermAssignment,
            usize,
            &mut dyn FnMut() -> Result<(), FormulaFailure>,
        ) -> Result<(), AssignmentFailure<FormulaFailure>>,
    ) -> Result<(), FormulaFailure> {
        let Slots::Owned { values, lease } = &mut self.slots else {
            unreachable!("retained bindings own their metadata");
        };
        let header = size_of::<Self>();
        let extra = header - size_of::<TermAssignment>();
        let limit = computation.allowance(lease, limits, location)?;
        let previous = lease.bytes();
        let result = operation(values, limit.saturating_sub(extra), &mut || {
            counters.work(limits, location)
        })
        .map_err(|error| add_header(error, extra));
        lease.observe(extra + values.retained_bytes(), location)?;
        let observed =
            computation.storage_observed(lease, previous, header, limits, counters, location);
        computation.storage_result(result, lease, limits, location)?;
        observed
    }
}

impl Binding<'_> {
    pub(crate) fn borrowed(slots: AssignmentSlice<'_>) -> Binding<'_> {
        Binding {
            slots: Slots::Borrowed(slots),
        }
    }

    /// The compiler-owned body boundary is within this frame. The core slice
    /// still checks that invariant before constructing the restricted view.
    pub(crate) fn prefix(&self, end: usize) -> Binding<'_> {
        Binding::borrowed(
            self.slots()
                .prefix(end)
                .expect("admitted body scope is within its frame"),
        )
    }

    pub(crate) fn len(&self) -> usize {
        self.slots().len()
    }

    pub(crate) fn slots(&self) -> AssignmentSlice<'_> {
        match &self.slots {
            Slots::Owned { values, .. } => values.as_slice(),
            Slots::Borrowed(slots) => *slots,
        }
    }

    pub(crate) fn is_bound(
        &self,
        variable: usize,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        self.slots()
            .is_bound(variable)
            .map_err(|error| assignment(error, location))
    }

    pub(crate) fn key(
        &self,
        variable: usize,
        location: Location,
    ) -> Result<TermKey, FormulaFailure> {
        self.slots()
            .key(variable)
            .map_err(|error| match error {
                AssignmentError::Slot { .. } => {
                    FormulaFailure::UnsafeVariable { variable, location }
                }
                error => assignment(error, location),
            })?
            .ok_or(FormulaFailure::UnsafeVariable { variable, location })
    }

    pub(crate) fn read<'read>(
        &self,
        variable: usize,
        read: CatalogRead<'read>,
        location: Location,
    ) -> Result<TermRef<'read>, FormulaFailure> {
        let key = self.key(variable, location)?;
        read.term(&key)
            .map_err(|error| assignment(AssignmentError::Read(error), location))
    }

    pub(crate) fn view<'a>(
        &'a self,
        read: CatalogRead<'a>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<BindingView<'a>, FormulaFailure> {
        self.slots()
            .bind_with(read, || counters.work(limits, location))
            .map_err(|error| failure(error, location))
    }

    pub(crate) fn resolve<'read>(
        &self,
        term: TemplateTerm<'read>,
        read: CatalogRead<'read>,
        location: Location,
    ) -> Result<TermRef<'read>, FormulaFailure> {
        match term {
            TemplateTerm::Constant(value) => Ok(value),
            TemplateTerm::Variable(variable) => self.read(variable, read, location),
        }
    }

    pub(crate) fn copied(
        &self,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Binding<'static>, FormulaFailure> {
        Binding::copy_slots(self.slots(), computation, limits, counters, location)
    }
}

fn add_header(
    error: AssignmentFailure<FormulaFailure>,
    extra: usize,
) -> AssignmentFailure<FormulaFailure> {
    match error {
        AssignmentFailure::Assignment(AssignmentError::Storage(
            zetesis_core::catalog::Error::Storage { required, limit },
        )) => AssignmentFailure::Assignment(AssignmentError::Storage(
            zetesis_core::catalog::Error::Storage {
                required: required + extra as u128,
                limit: limit + extra,
            },
        )),
        error => error,
    }
}

pub(crate) fn assignment(error: AssignmentError, location: Location) -> FormulaFailure {
    FormulaFailure::TermAssignment { error, location }
}

pub(crate) fn failure(
    error: AssignmentFailure<FormulaFailure>,
    location: Location,
) -> FormulaFailure {
    match error {
        AssignmentFailure::Assignment(error) => assignment(error, location),
        AssignmentFailure::Stopped(error) => error,
    }
}

#[cfg(test)]
mod tests;
