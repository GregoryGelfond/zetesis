//! Evaluated finite head arguments reuse the ordinary binding scheduler.
//!
//! Disjuncts share the rule's outer bindings: independent intervals produce a
//! Cartesian family of rules, each retaining its original disjunction. Choice
//! intervals instead contribute local alternatives to one group. Neither path
//! materializes a Cartesian product or shifts a disjunctive head.

use std::collections::BTreeSet;

use themelios_program::program::{Arguments, DefaultNegation, Literal, LiteralInner};
use themelios_program::symbol::Symbol;
use themelios_program::term::{Term, Variable};

use crate::diagnostic::unsupported;
use crate::formula::ceiling;
use crate::formula_ir::{
    Compiler, HeadIr, HeadLiteral, HeadOperand, LiteralIr, LocalFamily, Variables,
};
use crate::{ExpansionResource, FormulaFailure, FormulaResource, ProfileFeature};

impl Compiler<'_> {
    /// Declare original ordinary/disjunctive head names without generating values.
    /// Choice element names keep their separately scoped existing interpretation.
    pub(super) fn head_globals(
        &mut self,
        head: &themelios_program::program::Head,
        variables: &mut Variables,
    ) -> Result<(), FormulaFailure> {
        use themelios_program::program::Head;
        match head {
            Head::Literal(literal) => self.head_global_literal(literal, variables),
            Head::Disjunction(disjunction) => {
                for element in disjunction.elements() {
                    self.disjunct_globals(
                        element.get().literal(),
                        element.get().condition(),
                        variables,
                    )?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub(super) fn head_global_literal(
        &mut self,
        literal: &Literal,
        variables: &mut Variables,
    ) -> Result<(), FormulaFailure> {
        let LiteralInner::Atom(atom) = &literal.inner else {
            return Ok(());
        };
        let Arguments::Single(arguments) = &atom.get().arguments else {
            return Err(unsupported(ProfileFeature::PooledArguments, self.location).into());
        };
        for term in arguments.iter().flat_map(Term::subterms) {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            if let Term::Variable(variable @ Variable::Named(_)) = term {
                variables.slot(variable);
                self.variable_limit(variables)?;
            }
        }
        Ok(())
    }

    /// Plan source element conditions before generating the element's atom.
    /// Tuple measures remain in their existing source-validation scope.
    pub(super) fn element_head(
        &mut self,
        literal: &Literal,
        variables: &mut Variables,
        condition: &mut Vec<LiteralIr>,
    ) -> Result<(HeadLiteral, usize), FormulaFailure> {
        self.bindings(condition, variables)?;
        variables.safety(self.location)?;
        let body_variables = variables.count;
        let mut values = Vec::new();
        let head = self.head_literal(literal, variables, &mut values)?;
        self.bindings(&mut values, variables)?;
        variables.safety(self.location)?;
        condition.extend(values);
        Ok((head, body_variables))
    }

    /// Lower ordinary heads after the source body frame is complete. Fresh
    /// value targets belong exclusively to the later head-generation stage.
    pub(super) fn ordinary_head(
        &mut self,
        head: &themelios_program::program::Head,
        variables: &mut Variables,
        values: &mut Vec<LiteralIr>,
    ) -> Result<Option<HeadIr>, FormulaFailure> {
        use themelios_program::program::Head;
        Ok(match head {
            Head::Falsum => Some(HeadIr::Normal(None)),
            Head::Verum => Some(self.verum_head()?),
            Head::Literal(literal)
                if literal.negation == DefaultNegation::None
                    && matches!(literal.inner, LiteralInner::Atom(_)) =>
            {
                Some(HeadIr::Normal(Some(
                    self.generated_head(literal, variables, values)?,
                )))
            }
            Head::Literal(literal) => {
                ceiling(
                    FormulaResource::DisjunctionElements,
                    1,
                    self.limits.max_disjunction_elements as u128,
                    self.location,
                )?;
                // The singleton retains its signed literal in the original
                // implication. Neither default-negation sign supplies support.
                Some(HeadIr::Disjunction(vec![
                    self.head_literal(literal, variables, values)?,
                ]))
            }
            Head::Disjunction(disjunction) => {
                let mut heads = Vec::new();
                let mut elements = Vec::new();
                let outer = variables.clone();
                for (index, element) in disjunction.elements().enumerate() {
                    ceiling(
                        FormulaResource::DisjunctionElements,
                        heads.len() as u128 + elements.len() as u128 + 1,
                        self.limits.max_disjunction_elements as u128,
                        self.location,
                    )?;
                    if self.true_head_condition(element.get().condition())? {
                        // Unconditional generators retain the outer Cartesian family.
                        heads.push(self.head_literal(
                            element.get().literal(),
                            variables,
                            values,
                        )?);
                    } else {
                        self.conditional_disjunct(
                            element.get().literal(),
                            element.get().condition(),
                            &outer,
                            &mut elements,
                            LocalFamily(index),
                        )?;
                    }
                }
                ceiling(
                    FormulaResource::DisjunctionElements,
                    heads.len() as u128 + elements.len() as u128,
                    self.limits.max_disjunction_elements as u128,
                    self.location,
                )?;
                Some(if elements.is_empty() {
                    HeadIr::Disjunction(heads)
                } else {
                    HeadIr::ConditionalDisjunction {
                        ordinary: heads,
                        elements,
                    }
                })
            }
            Head::Choice(_) | Head::Aggregate(_) => None,
            Head::TheoryAtom(_) => {
                return Err(unsupported(ProfileFeature::Head, self.location).into());
            }
        })
    }

    pub(super) fn verum_head(&self) -> Result<HeadIr, FormulaFailure> {
        ceiling(
            FormulaResource::DisjunctionElements,
            1,
            self.limits.max_disjunction_elements as u128,
            self.location,
        )?;
        Ok(HeadIr::Disjunction(vec![HeadLiteral {
            negation: DefaultNegation::None,
            operand: HeadOperand::Boolean(true),
        }]))
    }

    pub(super) fn head_literal(
        &mut self,
        literal: &Literal,
        variables: &mut Variables,
        body: &mut Vec<LiteralIr>,
    ) -> Result<HeadLiteral, FormulaFailure> {
        if matches!(literal.inner, LiteralInner::True | LiteralInner::False) {
            return Ok(HeadLiteral {
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
        Ok(HeadLiteral {
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
                    crate::compile::validate_scalar(symbol, self.location)?;
                }
                Term::Interval { lower, upper } => {
                    self.head_slot(variables, fresh)?;
                    for bound in [lower, upper] {
                        if matches!(bound.as_ref(), Term::Symbolic(symbol) if !matches!(symbol, Symbol::Number(_)))
                        {
                            return Err(unsupported(ProfileFeature::Term, self.location).into());
                        }
                    }
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
