//! Scoped assignments retain absence independently of every ASP value.

use std::borrow::Cow;

use themelios_base::span::Location;
use zetesis_core::{Atom, AtomPattern, Term, Value};

use crate::expansion::Budget;
use crate::formula_support::{Counters, copy};
use crate::{FormulaFailure, FormulaLimits};

/// Slots retain source variable identity across local and component scopes.
/// Owned frames can be extended; a body-prefix view borrows its parent's slots.
/// All reads are checked even when the compiler has established scope safety.
#[derive(Default, Debug, PartialEq, Eq)]
pub(crate) struct Binding<'a> {
    slots: Cow<'a, [Option<Value>]>,
}

// Every production static constructor owns its slots. The only borrowed
// static frame is Default's empty slice; to_mut cannot clone logical values.
impl Binding<'static> {
    pub(crate) fn copy_slots(
        source: &[Option<Value>],
        limits: &FormulaLimits,
        counters: &mut Counters,
        budget: &mut Budget,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        let mut slots = Vec::new();
        reserve(&mut slots, source.len(), location)?;
        for slot in source {
            counters.work(limits, location)?;
            slots.push(
                slot.as_ref()
                    .map(|value| copy(value, budget, location))
                    .transpose()?,
            );
        }
        Ok(Self {
            slots: Cow::Owned(slots),
        })
    }

    /// An unevaluated head suffix may not exist yet. Clearing such a slot is
    /// a no-op; an existing output becomes absent before backtracking.
    pub(crate) fn clear(&mut self, variable: usize) {
        if let Some(slot) = self.slots.to_mut().get_mut(variable) {
            *slot = None;
        }
    }

    pub(crate) fn set(
        &mut self,
        variable: usize,
        value: Value,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let slot = self
            .slots
            .to_mut()
            .get_mut(variable)
            .ok_or(FormulaFailure::UnsafeVariable { variable, location })?;
        *slot = Some(value);
        Ok(())
    }

    pub(crate) fn extend_scope(
        &mut self,
        end: usize,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        if end > self.len() {
            let slots = self.slots.to_mut();
            reserve(slots, end, location)?;
            slots.resize(end, None);
        }
        Ok(())
    }
}

impl Binding<'_> {
    /// Read the current immutable frame without copying its typed payload.
    pub(crate) fn borrowed(slots: &[Option<Value>]) -> Binding<'_> {
        Binding {
            slots: Cow::Borrowed(slots),
        }
    }

    /// The compiler-owned body boundary must be within this frame.
    pub(crate) fn prefix(&self, end: usize) -> Binding<'_> {
        Binding {
            slots: Cow::Borrowed(&self.slots[..end]),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.slots.len()
    }

    pub(crate) fn slots(&self) -> &[Option<Value>] {
        &self.slots
    }

    pub(crate) fn read(
        &self,
        variable: usize,
        location: Location,
    ) -> Result<&Value, FormulaFailure> {
        self.slots
            .get(variable)
            .and_then(Option::as_ref)
            .ok_or(FormulaFailure::UnsafeVariable { variable, location })
    }

    pub(crate) fn resolve<'a>(
        &'a self,
        term: &'a Term,
        location: Location,
    ) -> Result<&'a Value, FormulaFailure> {
        match term {
            Term::Constant(value) => Ok(value),
            Term::Variable(variable) => self.read(*variable, location),
        }
    }

    pub(crate) fn instantiate(
        &self,
        pattern: &AtomPattern,
        location: Location,
    ) -> Result<Atom, FormulaFailure> {
        pattern
            .key(self.slots())
            .map(zetesis_core::AtomKey::to_atom)
            .map_err(|error| FormulaFailure::UnsafeVariable {
                variable: error.variable,
                location,
            })
    }

    pub(crate) fn copied(
        &self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        budget: &mut Budget,
        location: Location,
    ) -> Result<Binding<'static>, FormulaFailure> {
        Binding::copy_slots(self.slots(), limits, counters, budget, location)
    }
}

/// Reserve a binding frame's slots. A frame is transient: it holds one
/// rule's variables while a binding is joined and is released with it, so
/// its capacity is bounded by the rule's variable count times the join depth
/// and is not charged to the cumulative scalar allowance, which counts
/// retained payload. Failure never publishes a partial binding.
fn reserve(
    slots: &mut Vec<Option<Value>>,
    end: usize,
    location: Location,
) -> Result<(), FormulaFailure> {
    if end <= slots.capacity() {
        return Ok(());
    }
    slots
        .try_reserve_exact(end - slots.len())
        .map_err(|_| FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Allocation,
            location,
        })
}

#[cfg(test)]
pub(crate) fn complete(values: impl IntoIterator<Item = Value>) -> Binding<'static> {
    Binding {
        slots: Cow::Owned(values.into_iter().map(Some).collect()),
    }
}

#[cfg(test)]
mod tests;
