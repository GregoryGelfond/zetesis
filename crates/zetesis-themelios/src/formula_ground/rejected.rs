//! A rejected flat row needs binding validation, not a discarded formula.
//!
//! Callers have completed the row's join and authoritative source-family
//! checks. Scalar operations therefore need no second evaluation here. Atom
//! keys retain the remaining safety checks without interning atoms or making
//! formula nodes. Nested scopes and generators keep complete scoped lowering.

#[cfg(test)]
mod tests;

use super::{Binding, Builder, FormulaFailure, LiteralIr, ProgramSite};

impl Builder<'_, '_, '_> {
    /// Validate an eligible rejected row, or leave scoped validation to the
    /// caller. Eligibility reads the whole body before any key is checked, so
    /// encountering a local scope cannot leave a partially skipped family.
    /// Both passes use constant scratch and charge each visited literal.
    pub(super) fn validate_flat_body(
        &mut self,
        literals: &[LiteralIr],
        binding: &Binding<'_>,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        // Preserve a charged cancellation boundary even for an empty body.
        self.work(location)?;
        for literal in literals {
            self.work(location)?;
            if !matches!(
                literal,
                LiteralIr::Atom(..)
                    | LiteralIr::PatternAtom(_)
                    | LiteralIr::Compare(..)
                    | LiteralIr::ArgumentCheck { .. }
                    | LiteralIr::TupleCompare(..)
                    | LiteralIr::Guard(_)
                    | LiteralIr::HeadGuard(_)
            ) {
                return Ok(false);
            }
        }
        self.validate_atom_keys(literals, binding, location, |slot, location| {
            use zetesis_core::catalog::AssignmentError;
            let error = if slot < binding.len() {
                AssignmentError::Unbound { slot }
            } else {
                AssignmentError::Slot {
                    slot,
                    len: binding.len(),
                }
            };
            crate::formula_binding::assignment(error, location)
        })?;
        Ok(true)
    }

    /// The shared remaining duty for rejected flat rows. Streamed constraints
    /// already establish eligibility at admission; ordinary rejected bodies
    /// first use `validate_flat_body`. Each caller keeps its existing unbound
    /// variable failure. No head is read or evaluated here.
    pub(super) fn validate_atom_keys(
        &mut self,
        literals: &[LiteralIr],
        binding: &Binding<'_>,
        location: ProgramSite,
        unbound: impl Fn(usize, ProgramSite) -> FormulaFailure,
    ) -> Result<(), FormulaFailure> {
        validate_atom_keys(
            literals,
            binding,
            self.computation,
            self.limits,
            &mut self.counters,
            location,
            unbound,
        )
    }
}

pub(crate) fn validate_atom_keys(
    literals: &[LiteralIr],
    binding: &Binding<'_>,
    computation: &crate::formula_support::Computation<'_, '_>,
    limits: &crate::FormulaLimits,
    counters: &mut crate::formula_support::Counters,
    location: ProgramSite,
    unbound: impl Fn(usize, ProgramSite) -> FormulaFailure,
) -> Result<(), FormulaFailure> {
    for literal in literals {
        counters.work(limits, location)?;
        let pattern = match literal {
            LiteralIr::Atom(_, pattern) => pattern,
            LiteralIr::PatternAtom(pattern) => &pattern.atom,
            _ => continue,
        };
        let view = binding.view(computation.read(), limits, counters, location)?;
        let pattern = computation.static_pattern(*pattern, limits, counters, location)?;
        // A key scans argument metadata, including constants and repeated
        // variables. Account for the complete scan before starting it.
        counters.charge_work(pattern.terms().len() as u128, limits, location)?;
        pattern
            .key(view)
            .map_err(|error| unbound(error.variable, location))?;
    }
    Ok(())
}
