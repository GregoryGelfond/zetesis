//! Erase only total, explicitly true Boolean disjunct conditions.
//!
//! Empty conditions and conjunctions of true Boolean literals introduce no
//! bindings or semantic atoms. Erasure preserves each signed head occurrence
//! and the existing Cartesian family of whole rules. False Boolean conditions,
//! comparisons and atom conditions retain their explicit refusal until their
//! separate conditional-disjunction expansion contract is implemented.

use themelios_program::program::{Condition, DefaultNegation, LiteralInner};

use crate::diagnostic::unsupported;
use crate::formula_ir::Compiler;
use crate::{AdmissionFailure, ExpansionResource, FormulaFailure, InputLimit, ProfileFeature};

impl Compiler<'_> {
    pub(super) fn true_head_condition(
        &mut self,
        condition: &Condition,
    ) -> Result<(), FormulaFailure> {
        for (index, literal) in condition.literals().enumerate() {
            if index >= self.options.max_body_elements {
                return Err(AdmissionFailure::Limit {
                    resource: InputLimit::BodyElements,
                    limit: self.options.max_body_elements,
                    observed: index + 1,
                    location: self.location,
                }
                .into());
            }
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            let literal = literal.get();
            if !matches!(
                (&literal.inner, literal.negation),
                (
                    LiteralInner::True,
                    DefaultNegation::None | DefaultNegation::NotNot
                ) | (LiteralInner::False, DefaultNegation::Not)
            ) {
                return Err(
                    unsupported(ProfileFeature::ConditionalDisjunction, self.location).into(),
                );
            }
        }
        Ok(())
    }
}
