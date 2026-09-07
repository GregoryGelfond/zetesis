//! Scalar/range bindings and evaluated heads lower to scoped value slots.

use themelios_program::program::{Arguments, Literal, LiteralInner, Relation};
use themelios_program::term::{Term, Variable};
use zetesis_core::{AtomPattern, Predicate, Term as CoreTerm};

use crate::diagnostic::unsupported;
use crate::formula_ir::{Compiler, Expression, LiteralIr, Operation, Variables};
use crate::{AdmissionFailure, FormulaFailure, ProfileFeature};

impl Compiler<'_> {
    pub(super) fn binding_comparison(
        &mut self,
        left: &Term,
        relation: Relation,
        right: &Term,
        variables: &mut Variables,
    ) -> Result<LiteralIr, FormulaFailure> {
        let range = match (left, right) {
            (Term::Variable(target), Term::Interval { lower, upper })
            | (Term::Interval { lower, upper }, Term::Variable(target))
                if relation == Relation::Eq =>
            {
                Some((target, lower, upper))
            }
            _ => None,
        };
        if let Some((target, lower, upper)) = range {
            let target = variables.slot(target);
            self.variable_limit(variables)?;
            return Ok(LiteralIr::Range {
                target,
                lower: self.expression(lower, variables)?,
                upper: self.expression(upper, variables)?,
                binder: false,
            });
        }
        self.comparison(left, relation, right, variables)
    }

    pub(super) fn generated_head(
        &mut self,
        literal: &Literal,
        variables: &mut Variables,
        body: &mut Vec<LiteralIr>,
    ) -> Result<AtomPattern, FormulaFailure> {
        use themelios_program::program::DefaultNegation;
        if literal.negation != DefaultNegation::None {
            return Err(unsupported(ProfileFeature::NegatedHead, self.location).into());
        }
        let LiteralInner::Atom(atom) = &literal.inner else {
            return Err(unsupported(ProfileFeature::Head, self.location).into());
        };
        self.generated_head_atom(atom.get(), variables, body)
    }

    /// Bind arguments independently of the enclosing head literal's polarity.
    pub(super) fn generated_head_atom(
        &mut self,
        atom: &themelios_program::program::Atom,
        variables: &mut Variables,
        body: &mut Vec<LiteralIr>,
    ) -> Result<AtomPattern, FormulaFailure> {
        let Arguments::Single(arguments) = &atom.arguments else {
            return Err(unsupported(ProfileFeature::PooledArguments, self.location).into());
        };
        crate::formula::ceiling(
            crate::FormulaResource::Arity,
            arguments.len() as u128,
            self.options.core_limits.max_predicate_arity as u128,
            self.location,
        )?;
        let mut terms = Vec::new();
        for term in arguments {
            if matches!(term, Term::Variable(_) | Term::Symbolic(_)) {
                terms.push(self.objective_term(term, variables)?);
            } else {
                let target = variables.slot(&Variable::Anonymous);
                self.variable_limit(variables)?;
                let binding = if let Term::Interval { lower, upper } = term {
                    LiteralIr::Range {
                        target,
                        lower: self.expression(lower, variables)?,
                        upper: self.expression(upper, variables)?,
                        binder: false,
                    }
                } else {
                    LiteralIr::Compare(
                        Expression {
                            nodes: vec![Operation::Variable(target)],
                        },
                        Relation::Eq,
                        self.expression(term, variables)?,
                    )
                };
                body.push(binding);
                terms.push(CoreTerm::Variable(target));
            }
        }
        let predicate = Predicate::with_sign(
            atom.name.as_str(),
            arguments.len(),
            crate::coherence::core_sign(atom.sign),
        )
        .map_err(|error| AdmissionFailure::Construction {
            error,
            location: self.location,
        })?;
        let pattern =
            AtomPattern::new(predicate, terms).map_err(|error| AdmissionFailure::Construction {
                error,
                location: self.location,
            })?;
        self.pattern_domain(&pattern)?;
        Ok(pattern)
    }
}
