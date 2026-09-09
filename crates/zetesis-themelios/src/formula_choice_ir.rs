//! Original Boolean source occurrences become ordinary choice contribution keys.
//!
//! Finite-pool rewriting preserves the Boolean subsequence but reconstructs child
//! provenance. The checked source catalog supplies every parsed occurrence, and
//! this bounded reconciliation verifies complete logical equality before copying
//! its locations. Default negation is part of that logical equality.

use crate::formula_ir::Compiler;
use crate::{ExpansionResource, FormulaFailure};
use themelios_base::span::Location;
use themelios_program::program::ChoiceElement;
use themelios_program::provenance::{Origin, WithProvenance};

impl Compiler<'_> {
    /// The finite-pool rewrite changes only atomic head alternatives. Its
    /// Boolean subsequence keeps the source order and complete logical values,
    /// even though reconstructed child carriers no longer carry source keys.
    pub(super) fn boolean_occurrences(
        &mut self,
        element: &ChoiceElement,
        source: Option<&WithProvenance<ChoiceElement>>,
    ) -> Result<Vec<Location>, FormulaFailure> {
        let failure = || FormulaFailure::ChoiceSource {
            location: self.location,
        };
        let source = source.ok_or_else(failure)?;
        // Reserve a conservative source-byte comparison bound before comparing
        // complete logical elements. No Cartesian occurrence lookup is retained.
        self.budget.charge(
            ExpansionResource::TermWork,
            u128::from(self.location.span.len()) + 1,
            self.location,
        )?;
        if source.get() != element {
            return Err(failure());
        }
        let mut occurrences = Vec::new();
        for origin in source.provenance().origins() {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            if let Origin::Parsed(location) = origin {
                if location.source != self.location.source
                    || !self.location.span.contains_span(location.span)
                {
                    return Err(failure());
                }
                self.budget.charge(
                    ExpansionResource::ScalarBytes,
                    std::mem::size_of::<Location>() as u128,
                    self.location,
                )?;
                occurrences.push(*location);
            }
        }
        if occurrences.is_empty() {
            return Err(failure());
        }
        Ok(occurrences)
    }
}
