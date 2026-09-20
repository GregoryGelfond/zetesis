//! Original Boolean source occurrences become ordinary choice contribution keys.
//!
//! Choice elements compile from the retained original source. Its checked
//! provenance supplies each parsed occurrence before local pool expansion.

use crate::formula_ir::Compiler;
use crate::{ExpansionResource, FormulaFailure};
use themelios_base::span::Location;
use themelios_program::program::ChoiceElement;
use themelios_program::provenance::{Origin, WithProvenance};

impl Compiler<'_> {
    /// Retain each parsed occurrence within the original rule's source span.
    /// A generated carrier without that original source cannot supply the key.
    pub(super) fn boolean_occurrences(
        &mut self,
        source: Option<&WithProvenance<ChoiceElement>>,
    ) -> Result<Vec<Location>, FormulaFailure> {
        let failure = || FormulaFailure::ChoiceSource {
            location: self.location,
        };
        let source = source.ok_or_else(failure)?;
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
