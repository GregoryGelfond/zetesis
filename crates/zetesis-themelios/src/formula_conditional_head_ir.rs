//! Finite existential head instances share the ordinary local binding compiler.
//!
//! Each instance retains (condition → head) ∧ not not condition. Conditions
//! select possible bindings here; truth and reduct remain formula operations.

use themelios_program::program::{Condition, DefaultNegation, Literal, LiteralInner};
use themelios_program::term::{Term, Variable};

use crate::formula::ceiling;
use crate::formula_ir::{Compiler, HeadLiteral, LiteralIr, Variables};
use crate::{AdmissionFailure, ExpansionResource, FormulaFailure, FormulaResource, InputLimit};

pub(crate) struct ConditionalHeadIr {
    pub head: HeadLiteral,
    pub condition: Vec<LiteralIr>,
    /// Only this prefix belongs to the enclosing rule, before head generation.
    pub outer_variables: usize,
    /// Local condition slots precede the independently generated head suffix.
    pub body_variables: usize,
    pub variables: usize,
}

impl Compiler<'_> {
    pub(super) fn true_head_condition(
        &mut self,
        condition: &Condition,
    ) -> Result<bool, FormulaFailure> {
        let mut total = true;
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
            total &= matches!(
                (&literal.inner, literal.negation),
                (
                    LiteralInner::True,
                    DefaultNegation::None | DefaultNegation::NotNot
                ) | (LiteralInner::False, DefaultNegation::Not)
            );
        }
        Ok(total)
    }

    /// A name occurring only inside one condition is local. A head-only name,
    /// or one declared elsewhere in the outer rule, keeps the outer identity.
    pub(super) fn disjunct_globals(
        &mut self,
        literal: &Literal,
        condition: &Condition,
        variables: &mut Variables,
    ) -> Result<(), FormulaFailure> {
        if self.true_head_condition(condition)? {
            return self.head_global_literal(literal, variables);
        }
        for term in terms(literal).flat_map(Term::subterms) {
            self.scope_work(1)?;
            let Term::Variable(variable @ Variable::Named(name)) = term else {
                continue;
            };
            let mut local = false;
            for condition in condition.literals() {
                for term in terms(condition.get()).flat_map(Term::subterms) {
                    self.scope_work(1)?;
                    local |=
                        matches!(term, Term::Variable(Variable::Named(other)) if other == name);
                }
            }
            if !local {
                variables.slot(variable);
                self.variable_limit(variables)?;
            }
        }
        Ok(())
    }

    pub(super) fn conditional_disjunct(
        &mut self,
        literal: &Literal,
        source: &Condition,
        outer: &Variables,
        elements: &mut Vec<ConditionalHeadIr>,
    ) -> Result<(), FormulaFailure> {
        for condition in self.condition_alternatives(source)? {
            if let LiteralInner::Atom(atom) = &literal.inner {
                for alternative in self.conditional_atoms(atom.get())? {
                    let literal = Literal {
                        negation: literal.negation,
                        inner: LiteralInner::Atom(atom.clone().map(|_| alternative)),
                    };
                    self.local_disjunct(&literal, &condition, outer, elements)?;
                }
            } else {
                self.local_disjunct(literal, &condition, outer, elements)?;
            }
        }
        Ok(())
    }

    fn local_disjunct(
        &mut self,
        literal: &Literal,
        source: &Condition,
        outer: &Variables,
        elements: &mut Vec<ConditionalHeadIr>,
    ) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::DisjunctionElements,
            elements.len() as u128 + 1,
            self.limits.max_disjunction_elements as u128,
            self.location,
        )?;
        let mut local = outer.clone();
        let mut condition = self.condition(source, &mut local)?;
        // The same scheduler and safety boundary serves choice, function-head
        // and conditional-disjunct instances, including inherited proposals.
        let (head, body_variables) = self.element_head(literal, &mut local, &mut condition)?;
        self.variable_limit(&local)?;
        local.safety(self.location)?;
        elements.push(ConditionalHeadIr {
            head,
            condition,
            outer_variables: outer.count,
            body_variables,
            variables: local.count,
        });
        Ok(())
    }
}

fn terms(literal: &Literal) -> impl Iterator<Item = &Term> {
    let atom = match &literal.inner {
        LiteralInner::Atom(atom) => Some(atom.get()),
        _ => None,
    };
    let comparison = match &literal.inner {
        LiteralInner::Comparison(comparison) => Some(comparison.get()),
        _ => None,
    };
    atom.into_iter()
        .flat_map(themelios_program::program::Atom::argument_terms)
        .chain(comparison.into_iter().flat_map(|comparison| {
            std::iter::once(comparison.first()).chain(comparison.steps().map(|(_, term)| term))
        }))
}
