//! Separate outer scopes and independently bound universal-local instances.

use std::collections::BTreeMap;

use themelios_program::program::{
    BodyElement, ConditionalLiteral, DefaultNegation, Literal, LiteralInner, Rule,
};
use themelios_program::term::{Term, Variable};
use zetesis_core::AtomPattern;

use crate::formula_guard::Guard;
use crate::formula_ir::{Compiler, LiteralIr, Variables};
use crate::{AdmissionFailure, ExpansionResource, FormulaFailure, InputLimit};

pub(crate) struct ConditionalIr {
    pub consequent: Consequent,
    pub condition: Vec<LiteralIr>,
    pub variables: usize,
}

/// The consequent is a test, never a source of local bindings or support.
pub(crate) enum Consequent {
    Atom(DefaultNegation, AtomPattern),
    Guard(Guard),
}

impl Compiler<'_> {
    pub(super) fn conditional_globals(
        &mut self,
        conditional: &ConditionalLiteral,
        variables: &mut Variables,
    ) -> Result<(), FormulaFailure> {
        let head = self.conditional_names(&conditional.literal)?;
        let mut condition = BTreeMap::new();
        for (index, literal) in conditional.condition.literals().enumerate() {
            if index >= self.options.max_body_elements {
                return Err(AdmissionFailure::Limit {
                    resource: InputLimit::BodyElements,
                    limit: self.options.max_body_elements,
                    observed: index + 1,
                    location: self.location,
                }
                .into());
            }
            condition.extend(self.conditional_names(literal.get())?);
        }
        // This slice reserves consequent-only names in the outer environment
        // and requires an independent admitted binding. Full clingo can admit
        // additional forms using the consequent's grounding information; their
        // refusal here is a profile boundary, not a clingo unsafety conclusion.
        // Names shared with C may also occur in an outer ordinary/head/guard.
        for (name, variable) in head {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            if !condition.contains_key(name) {
                self.budget.charge(
                    ExpansionResource::ScalarBytes,
                    name.len() as u128,
                    self.location,
                )?;
                variables.slot(variable);
                self.variable_limit(variables)?;
            }
        }
        Ok(())
    }

    fn conditional_names<'a>(
        &mut self,
        literal: &'a Literal,
    ) -> Result<BTreeMap<&'a str, &'a Variable>, FormulaFailure> {
        self.budget
            .charge(ExpansionResource::TermWork, 1, self.location)?;
        let mut names = BTreeMap::new();
        let mut term_names = |term: &'a Term| -> Result<(), FormulaFailure> {
            for term in term.subterms() {
                self.budget
                    .charge(ExpansionResource::TermWork, 1, self.location)?;
                if let Term::Variable(variable @ Variable::Named(name)) = term {
                    names.insert(name.as_str(), variable);
                }
            }
            Ok(())
        };
        match &literal.inner {
            LiteralInner::Atom(atom) => {
                for term in atom.get().argument_terms() {
                    term_names(term)?;
                }
            }
            LiteralInner::Comparison(comparison) => {
                term_names(comparison.get().first())?;
                for (_, term) in comparison.get().steps() {
                    term_names(term)?;
                }
            }
            LiteralInner::True | LiteralInner::False => {}
        }
        Ok(names)
    }

    pub(super) fn body_conditionals(
        &mut self,
        rule: &Rule,
        variables: &Variables,
        body: &mut Vec<LiteralIr>,
    ) -> Result<(), FormulaFailure> {
        for element in rule.body().get().elements() {
            let BodyElement::Conditional(conditional) = element.get() else {
                continue;
            };
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            let mut local = variables.clone();
            let condition = &conditional.condition;
            let mut condition = self.condition(condition, &mut local)?;
            let consequent = self.conditional_consequent(&conditional.literal, &mut local)?;
            self.bindings(&mut condition, &mut local)?;
            self.variable_limit(&local)?;
            local.safety(self.location)?;
            body.push(LiteralIr::Conditional(ConditionalIr {
                consequent,
                condition,
                variables: local.count,
            }));
        }
        Ok(())
    }

    fn conditional_consequent(
        &mut self,
        literal: &Literal,
        variables: &mut Variables,
    ) -> Result<Consequent, FormulaFailure> {
        if let LiteralInner::Atom(atom) = &literal.inner {
            return Ok(Consequent::Atom(
                literal.negation,
                self.atom(atom.get(), variables, false)?,
            ));
        }
        let guard = if let LiteralInner::Comparison(comparison) = &literal.inner {
            self.comparison_guard(comparison.get(), literal.negation, variables)?
        } else {
            self.literal(literal, variables)?
        };
        let LiteralIr::Guard(guard) = guard else {
            unreachable!("non-atom conditional consequent is a ground guard")
        };
        Ok(Consequent::Guard(guard))
    }
}
