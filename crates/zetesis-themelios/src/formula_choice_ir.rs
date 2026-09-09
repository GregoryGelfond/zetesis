//! Evaluated arguments and finite intervals generate local choice alternatives.
//!
//! Each expression or interval receives an independent local slot; the shared
//! dependency scheduler binds its inputs from outer and element conditions.
//! The streamed cursor retains one choice group without allocating an interval
//! or Cartesian product. Original conditions, signs and origins remain intact.

use crate::diagnostic::unsupported;
use crate::formula_ir::{Compiler, HeadOperand, LiteralIr, Variables};
use crate::{ExpansionResource, FormulaFailure, ProfileFeature};
use themelios_base::span::Location;
use themelios_program::program::{
    Arguments, ChoiceElement, DefaultNegation, Literal, LiteralInner,
};
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

    pub(super) fn choice_head(
        &mut self,
        literal: &Literal,
        variables: &mut Variables,
        condition: &mut Vec<LiteralIr>,
    ) -> Result<HeadOperand, FormulaFailure> {
        if literal.negation != DefaultNegation::None {
            return Err(unsupported(ProfileFeature::NegatedHead, self.location).into());
        }
        if matches!(literal.inner, LiteralInner::True | LiteralInner::False) {
            return Ok(HeadOperand::Boolean(matches!(
                literal.inner,
                LiteralInner::True
            )));
        }
        let LiteralInner::Atom(atom) = &literal.inner else {
            return self.head(literal, variables).map(HeadOperand::Atom);
        };
        let Arguments::Single(arguments) = &atom.get().arguments else {
            return self.head(literal, variables).map(HeadOperand::Atom);
        };
        if !self.needs_head_generation(arguments)? {
            return self.head(literal, variables).map(HeadOperand::Atom);
        }
        self.generated_arguments(arguments, variables)?;
        self.generated_head(literal, variables, condition)
            .map(HeadOperand::Atom)
    }
}
