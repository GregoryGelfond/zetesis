//! Separate outer scopes and independently bound universal-local instances.

use themelios_program::program::{
    BodyElement, ConditionalLiteral, DefaultNegation, Literal, LiteralInner, Rule,
};
use themelios_program::term::Term;
use zetesis_core::AtomPattern;

use crate::formula_guard::Guard;
use crate::formula_ir::{Compiler, LiteralIr, Projection, Variables};
use crate::{AdmissionFailure, ExpansionResource, FormulaFailure, InputLimit};

pub(crate) struct ConditionalIr {
    pub consequent: Consequent,
    pub condition: Vec<LiteralIr>,
    pub variables: usize,
}

/// Local alternatives never bind the outer rule or generate positive support.
pub(crate) enum Consequent {
    Atoms(DefaultNegation, Vec<Alternative>),
    Guard(Guard),
}

pub(crate) struct Alternative {
    pub operand: ConsequentOperand,
    /// Data instructions and optional positive witnesses, scoped to this alternative.
    pub bindings: Vec<LiteralIr>,
    pub variables: usize,
}

/// Source alternatives and anonymous witnesses have distinct quantifiers.
/// Projection completes the witness disjunction before default negation;
/// the caller disjoins the resulting signed source alternatives afterwards.
pub(crate) enum ConsequentOperand {
    Atom(AtomPattern),
    Projection(Projection),
}

impl Compiler<'_> {
    pub(super) fn conditional_syntax(
        &mut self,
        conditional: &ConditionalLiteral,
    ) -> Result<(), FormulaFailure> {
        self.conditional_terms(&conditional.literal)?;
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
            self.conditional_terms(literal.get())?;
        }
        // Only occurrences outside conditionals establish global names. A
        // consequent-only positive witness belongs to its own local scope.
        Ok(())
    }

    fn conditional_terms(&mut self, literal: &Literal) -> Result<(), FormulaFailure> {
        self.budget
            .charge(ExpansionResource::TermWork, 1, self.location)?;
        let mut scan = |term: &Term| -> Result<(), FormulaFailure> {
            for _ in term.subterms() {
                self.budget
                    .charge(ExpansionResource::TermWork, 1, self.location)?;
            }
            Ok(())
        };
        match &literal.inner {
            LiteralInner::Atom(atom) => {
                for term in atom.get().argument_terms() {
                    scan(term)?;
                }
            }
            LiteralInner::Comparison(comparison) => {
                scan(comparison.get().first())?;
                for (_, term) in comparison.get().steps() {
                    scan(term)?;
                }
            }
            LiteralInner::True | LiteralInner::False => {}
        }
        Ok(())
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
            self.bindings(&mut condition, &mut local)?;
            self.variable_limit(&local)?;
            local.safety(self.location)?;
            // A positive consequent must not repair an unsafe condition.
            let consequent = self.conditional_consequent(&conditional.literal, &mut local)?;
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
            let mut alternatives = Vec::new();
            for atom in self.conditional_atoms(atom.get())? {
                alternatives.push(self.consequent_alternative(
                    &atom,
                    literal.negation,
                    variables,
                )?);
            }
            return Ok(Consequent::Atoms(literal.negation, alternatives));
        }
        let guard = if let LiteralInner::Comparison(comparison) = &literal.inner {
            self.comparison_guard(comparison.get(), literal.negation, variables)?
        } else {
            self.literal(literal, variables)?
        };
        let LiteralIr::Guard(guard) = guard else {
            unreachable!("non-atom conditional consequent is a ground guard")
        };
        variables.safety(self.location)?;
        Ok(Consequent::Guard(guard))
    }
}
