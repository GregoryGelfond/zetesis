//! Evaluated arguments and finite intervals generate local choice alternatives.
//!
//! Each expression or interval receives an independent local slot; the shared
//! dependency scheduler binds its inputs from outer and element conditions.
//! The streamed cursor retains one choice group without allocating an interval
//! or Cartesian product. Original conditions, signs and origins remain intact.

use themelios_program::program::{Arguments, DefaultNegation, Literal, LiteralInner};
use zetesis_core::AtomPattern;

use crate::diagnostic::unsupported;
use crate::formula_ir::{Compiler, LiteralIr, Variables};
use crate::{FormulaFailure, ProfileFeature};

impl Compiler<'_> {
    pub(super) fn choice_head(
        &mut self,
        literal: &Literal,
        variables: &mut Variables,
        condition: &mut Vec<LiteralIr>,
    ) -> Result<AtomPattern, FormulaFailure> {
        if literal.negation != DefaultNegation::None {
            return Err(unsupported(ProfileFeature::NegatedHead, self.location).into());
        }
        let LiteralInner::Atom(atom) = &literal.inner else {
            return self.head(literal, variables);
        };
        let Arguments::Single(arguments) = &atom.get().arguments else {
            return self.head(literal, variables);
        };
        if !self.needs_head_generation(arguments)? {
            return self.head(literal, variables);
        }
        self.generated_head_arguments(arguments, variables)?;
        self.generated_head(literal, variables, condition)
    }
}
