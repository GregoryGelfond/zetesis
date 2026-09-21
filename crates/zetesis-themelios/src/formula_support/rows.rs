//! A staging token locates a complete frame before the consumer borrows it.
//!
//! Returning a reference from the traversal's mutation loop would keep that
//! loop borrowed. Instead the loop reports the frame location, and `next_row`
//! creates its sole `Row`/`Binding` view after mutation has stopped. The token
//! owns no second copy of current slots and never escapes the join module.

use zetesis_core::Value;

use crate::formula_binding::Binding;

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
    pub(super) fn binding<'a>(&'a self, current: &'a [Option<Value>]) -> Binding<'a> {
        Binding::borrowed(match self {
            Self::Current => current,
            Self::Owned(binding) => binding.slots(),
        })
    }

    pub(super) fn into_binding(self, current: &[Option<Value>]) -> Binding<'_> {
        match self {
            Self::Current => Binding::borrowed(current),
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
mod tests;
