//! A staging token locates a complete frame before the consumer borrows it.
//!
//! Returning a reference from the traversal's mutation loop would keep that
//! loop borrowed. Instead the loop reports the frame location, and `next_row`
//! creates its sole `Row`/`Binding` view after mutation has stopped. The token
//! owns no second copy of current slots and never escapes the join module.

use crate::ProgramSite;

use super::{Computation, Counters, Join, Row};
use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::{FormulaFailure, FormulaLimits};

/// A necessary condition on an original positive support occurrence. The join
/// consults it before binding or evaluating scalar expressions. It must charge
/// its work before execution through the supplied counters, preserving failure
/// prefixes. `false` skips only this row; an error stops the enclosing scan.
///
/// Row positions are predicate-local support coordinates. Any interpretation
/// in another catalog must authenticate that mapping; no equality of positions
/// is implied. Unmapped rows may be retained conservatively. Only already
/// admitted witness scans attach this filter, never source-family validation.
pub(crate) trait RowFilter {
    fn permits(
        &self,
        row: zetesis_core::relation::Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure>;
}

/// One existing join cursor with a selected-row-only consumer interface. Its
/// private cursor cannot export partial arithmetic evidence as a whole family.
pub(crate) struct FilteredRows<'a, 'source> {
    join: Join<'a, 'source>,
}

impl<'a, 'source> FilteredRows<'a, 'source> {
    pub(super) fn new(join: Join<'a, 'source>) -> Self {
        Self { join }
    }

    pub(crate) fn next_row(
        &mut self,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<Row<'_>>, FormulaFailure> {
        self.join
            .next_row(computation, limits, budget, counters, location)
    }
}

/// The current generator may drain independently while a base row is suspended.
#[derive(Clone, Copy)]
pub(super) enum Advance {
    Base,
    Current,
}

#[derive(Clone, Copy)]
pub(super) enum Ownership {
    /// The consumer finishes reading before the next traversal step.
    Lend,
    /// A generator or owning adapter retains the completed binding.
    Own,
}

pub(super) enum Frame {
    /// `Join::values` is still complete; its final-depth undo is suspended.
    Current,
    Owned(Binding<'static>),
}

impl Frame {
    pub(super) fn binding<'a>(&'a self, current: &'a Binding) -> Binding<'a> {
        Binding::borrowed(match self {
            Self::Current => current.slots(),
            Self::Owned(binding) => binding.slots(),
        })
    }

    pub(super) fn into_binding<'a>(self, current: &'a Binding<'_>) -> Binding<'a> {
        match self {
            Self::Current => Binding::borrowed(current.slots()),
            Self::Owned(binding) => binding,
        }
    }

    /// Owning adapters and generated continuations request `Ownership::Own`
    /// before completion, preserving copy admission before scalar filtering.
    pub(super) fn into_owned(self) -> Binding<'static> {
        match self {
            Self::Owned(binding) => binding,
            Self::Current => unreachable!("owning continuation requested an owned frame"),
        }
    }
}

pub(super) struct Staged {
    pub(super) frame: Frame,
    pub(super) passes: bool,
}

#[cfg(test)]
mod filtering;
#[cfg(test)]
mod tests;
