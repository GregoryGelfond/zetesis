//! Evaluated finite head arguments reuse the ordinary binding scheduler.
//!
//! Disjuncts share the rule's outer bindings: independent intervals produce a
//! Cartesian family of rules, each retaining its original disjunction. Choice
//! intervals instead contribute local alternatives to one group. Neither path
//! materializes a Cartesian product or shifts a disjunctive head.

use std::collections::BTreeSet;

use themelios_program::program::{Arguments, Literal, LiteralInner};
use themelios_program::symbol::Symbol;
use themelios_program::term::{Term, Variable};

use crate::diagnostic::unsupported;
use crate::formula::ceiling;
use crate::formula_ir::{Compiler, DisjunctIr, HeadOperand, LiteralIr, Variables};
use crate::{ExpansionResource, FormulaFailure, FormulaResource, ProfileFeature};

impl Compiler<'_> {
    pub(super) fn disjunction_head(
        &mut self,
        literal: &Literal,
        variables: &mut Variables,
        body: &mut Vec<LiteralIr>,
    ) -> Result<DisjunctIr, FormulaFailure> {
        if matches!(literal.inner, LiteralInner::True | LiteralInner::False) {
            return Ok(DisjunctIr {
                negation: literal.negation,
                operand: HeadOperand::Boolean(matches!(literal.inner, LiteralInner::True)),
            });
        }
        let LiteralInner::Atom(atom) = &literal.inner else {
            return Err(unsupported(ProfileFeature::Head, self.location).into());
        };
        let Arguments::Single(arguments) = &atom.get().arguments else {
            return Err(unsupported(ProfileFeature::PooledArguments, self.location).into());
        };
        let pattern = if self.needs_head_generation(arguments)? {
            self.generated_arguments(arguments, variables)?;
            self.generated_atom(atom.get(), variables, body)?
        } else {
            self.atom(atom.get(), variables, false)?
        };
        Ok(DisjunctIr {
            negation: literal.negation,
            operand: HeadOperand::Atom(pattern),
        })
    }

    pub(super) fn needs_head_generation(
        &mut self,
        arguments: &[Term],
    ) -> Result<bool, FormulaFailure> {
        ceiling(
            FormulaResource::Arity,
            arguments.len() as u128,
            self.options.core_limits.max_predicate_arity as u128,
            self.location,
        )?;
        self.budget.charge(
            ExpansionResource::TermWork,
            arguments.len() as u128,
            self.location,
        )?;
        Ok(arguments
            .iter()
            .any(|term| !matches!(term, Term::Variable(_) | Term::Symbolic(_))))
    }

    /// Check every synthetic slot and distinct input name before lowering.
    /// This checks storage and syntax, not safety: the shared scheduler alone
    /// establishes whether ordinary outer/local conditions bind each input.
    pub(super) fn generated_arguments(
        &mut self,
        arguments: &[Term],
        variables: &Variables,
    ) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::Arity,
            arguments.len() as u128,
            self.options.core_limits.max_predicate_arity as u128,
            self.location,
        )?;
        let mut fresh = 0;
        let mut named = BTreeSet::new();
        for term in arguments {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            if !matches!(term, Term::Variable(_) | Term::Symbolic(_)) {
                self.head_slot(variables, &mut fresh)?;
            }
            if let Term::Interval { lower, upper } = term {
                for bound in [lower, upper] {
                    self.head_expression(bound, variables, &mut named, &mut fresh)?;
                    // Preserve the existing explicit refusal of closed
                    // nonnumeric bounds; bound variables are evaluated later.
                    if matches!(bound.as_ref(), Term::Symbolic(symbol) if !matches!(symbol, Symbol::Number(_)))
                    {
                        return Err(unsupported(ProfileFeature::Term, self.location).into());
                    }
                }
            } else {
                self.head_expression(term, variables, &mut named, &mut fresh)?;
            }
        }
        Ok(())
    }

    fn head_expression<'a>(
        &mut self,
        term: &'a Term,
        variables: &Variables,
        named: &mut BTreeSet<&'a str>,
        fresh: &mut u128,
    ) -> Result<(), FormulaFailure> {
        for term in term.subterms() {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            match term {
                Term::Variable(Variable::Anonymous) => self.head_slot(variables, fresh)?,
                Term::Variable(Variable::Named(name)) => {
                    if !variables.named.contains_key(name.as_str())
                        && !named.contains(name.as_str())
                    {
                        self.head_slot(variables, fresh)?;
                        named.insert(name.as_str());
                    }
                }
                Term::Symbolic(symbol) => {
                    let bytes = crate::structural_value::symbol_bytes(symbol);
                    self.budget
                        .charge(ExpansionResource::ScalarBytes, bytes, self.location)?;
                    crate::compile::scalar(symbol, self.location)?;
                }
                Term::UnaryOperation { .. }
                | Term::BinaryOperation { .. }
                | Term::Absolute(_)
                | Term::Function { .. }
                | Term::Tuple(_) => {}
                _ => return Err(unsupported(ProfileFeature::Term, self.location).into()),
            }
        }
        Ok(())
    }

    fn head_slot(&self, variables: &Variables, fresh: &mut u128) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::Variables,
            variables.count as u128 + *fresh + 1,
            self.options.core_limits.max_variables_per_template as u128,
            self.location,
        )?;
        *fresh += 1;
        Ok(())
    }
}
