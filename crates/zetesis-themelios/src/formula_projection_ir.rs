//! Anonymous default-negated arguments are existential projection positions.

use themelios_program::program::{Arguments, Atom, DefaultNegation};
use themelios_program::term::{Term, Variable};
use zetesis_core::{Predicate, Term as CoreTerm};

use crate::formula::ceiling;
use crate::formula_ir::{Compiler, LiteralIr, Projection, Variables};
use crate::{AdmissionFailure, FormulaFailure, FormulaResource};

impl Compiler<'_> {
    pub(super) fn projected_atom(
        &mut self,
        atom: &Atom,
        negation: DefaultNegation,
        variables: &mut Variables,
    ) -> Result<Option<LiteralIr>, FormulaFailure> {
        // clingo projects anonymous arguments of unsigned default-negated
        // predicates, but treats signed occurrences as ordinary unsafe slots.
        // Let the normal literal path perform the latter safety check.
        if negation == DefaultNegation::None
            || atom.sign == themelios_program::symbol::Sign::Negative
        {
            return Ok(None);
        }
        let Arguments::Single(arguments) = &atom.arguments else {
            return Ok(None);
        };
        if !arguments
            .iter()
            .any(|term| matches!(term, Term::Variable(Variable::Anonymous)))
        {
            return Ok(None);
        }
        ceiling(
            FormulaResource::Arity,
            arguments.len() as u128,
            self.options.core_limits.max_predicate_arity as u128,
            self.location,
        )?;
        let mut terms = Vec::new();
        for term in arguments {
            if matches!(term, Term::Variable(Variable::Anonymous)) {
                terms.push(None);
            } else {
                let term = self.objective_term(term, variables)?;
                if let CoreTerm::Constant(value) = &term {
                    self.value(value)?;
                }
                terms.push(Some(term));
            }
        }
        let predicate = Predicate::with_sign(
            atom.name.as_str(),
            terms.len(),
            crate::coherence::core_sign(atom.sign),
        )
        .map_err(|error| AdmissionFailure::Construction {
            error,
            location: self.location,
        })?;
        Ok(Some(LiteralIr::ProjectedAtom(
            negation,
            Projection { predicate, terms },
        )))
    }
}
