//! One objective fragment's independent nested-scope validation.
//!
//! The first passing row performs the ordinary charged scan. Only successful
//! completion with no scope-bearing syntax proves that later bindings have no
//! nested tasks. A rich scope stays pending even when its first binding has no
//! descendants. The immutable source borrow cannot change beneath that proof.

use super::{Binding, Budget, Computation, Context, FormulaFailure};
use super::{LiteralIr, Scan, Support, Warnings};
use crate::formula_support::GroundingWork;

#[cfg(test)]
mod tests;

pub(in crate::formula_ground) struct Scopes<'source> {
    pending: Option<&'source [LiteralIr]>,
}

impl<'source> Scopes<'source> {
    pub(in crate::formula_ground) fn new(literals: &'source [LiteralIr]) -> Self {
        Self {
            pending: Some(literals),
        }
    }

    pub(in crate::formula_ground) fn validate(
        &mut self,
        binding: &Binding,
        support: &Support<'source>,
        budget: &mut Budget,
        warnings: &mut Warnings,
        context: Context<'_, &mut Computation<'_, 'source>>,
    ) -> Result<(), FormulaFailure> {
        let Some(literals) = self.pending else {
            return Ok(());
        };
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let mut scan = Scan {
            support,
            computation,
            limits,
            budget,
            counters,
            warnings,
            stack: Vec::new(),
        };
        let nested = scan.children(literals, binding, location)?;
        scan.run()?;
        if !nested {
            // The reference control retains the exact existing traversal;
            // neither the branch nor its state is in a production build.
            #[cfg(test)]
            if tests::reference() {
                return Ok(());
            }
            self.pending = None;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(in crate::formula_ground) fn reference_scopes<T>(
    reference: bool,
    run: impl FnOnce() -> T,
) -> T {
    tests::controlled(reference, run)
}
