//! Finite consequent alternatives share value evaluation and structural matching.
//!
//! Relational witnesses retain their complete captured atom. Generated values
//! use the shared interval fold; each alternative keeps its own local scope.

use themelios_program::program::{Arguments, Atom, DefaultNegation, Relation};
use themelios_program::term::{Term, Variable};
use zetesis_core::{AtomPattern, Predicate, Term as CoreTerm};

use crate::formula::ceiling;
use crate::formula_conditional_ir::{Alternative, ConsequentOperand};
use crate::formula_ir::{Compiler, Expression, LiteralIr, Operation, Variables};
use crate::{AdmissionFailure, ExpansionResource, FormulaFailure, FormulaResource};

impl Compiler<'_> {
    pub(super) fn consequent_alternative(
        &mut self,
        atom: &Atom,
        negation: DefaultNegation,
        condition: &Variables,
    ) -> Result<Alternative, FormulaFailure> {
        let Arguments::Single(arguments) = &atom.arguments else {
            unreachable!("conditional pools were selected")
        };
        let work = arguments.iter().flat_map(Term::subterms).count() as u128;
        let mut local =
            self.alternative_scope(work, std::mem::size_of::<Alternative>(), condition)?;
        let mut bindings = Vec::new();
        let missing = arguments
            .iter()
            .flat_map(Term::subterms)
            .any(|term| match term {
                Term::Variable(Variable::Anonymous) => true,
                Term::Variable(Variable::Named(name)) => !local.named.contains_key(name.as_str()),
                _ => false,
            });
        let operand = if let Some(LiteralIr::ProjectedAtom(_, projection)) =
            self.projected_atom(atom, negation, &mut local, &mut bindings)?
        {
            ConsequentOperand::Projection(projection)
        } else if missing && negation == DefaultNegation::None {
            ConsequentOperand::Atom(self.positive_atom_key(atom, &mut local, &mut bindings)?)
        } else {
            ConsequentOperand::Atom(self.consequent_atom(
                atom,
                arguments,
                &mut local,
                &mut bindings,
            )?)
        };
        self.bindings(&mut bindings, &mut local)?;
        self.variable_limit(&local)?;
        local.safety(self.location)?;
        Ok(Alternative {
            operand,
            bindings,
            variables: local.count,
        })
    }

    fn consequent_atom(
        &mut self,
        atom: &Atom,
        arguments: &[Term],
        local: &mut Variables,
        bindings: &mut Vec<LiteralIr>,
    ) -> Result<AtomPattern, FormulaFailure> {
        ceiling(
            FormulaResource::Arity,
            arguments.len() as u128,
            self.options.core_limits.max_predicate_arity as u128,
            self.location,
        )?;
        let mut terms = Vec::new();
        for term in arguments {
            if matches!(term, Term::Symbolic(_) | Term::Variable(_)) {
                let value = self.objective_term(term, local)?;
                if let CoreTerm::Constant(value) = &value {
                    self.value(value)?;
                }
                terms.push(value);
            } else {
                let value = self.ranged_expression(term, local, bindings)?;
                let target = self.consequent_slot(local)?;
                bindings.push(LiteralIr::Compare(
                    Expression {
                        nodes: vec![Operation::Variable(target)],
                    },
                    Relation::Eq,
                    value,
                ));
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
        AtomPattern::new(predicate, terms).map_err(|error| {
            AdmissionFailure::Construction {
                error,
                location: self.location,
            }
            .into()
        })
    }

    /// Retain the complete capture independently of its matching instruction.
    /// Capture slots represent matched arguments; closed constants retain their
    /// complete values. Charge term/payload visits, predicate text, term cells
    /// and selected value payload before cloning. Structured payload is charged
    /// conservatively even when its immutable allocation is shared by the clone,
    /// following the existing value-copy budget. Allocator metadata is excluded.
    pub(super) fn consequent_capture(
        &mut self,
        pattern: &AtomPattern,
    ) -> Result<AtomPattern, FormulaFailure> {
        self.budget.charge(
            ExpansionResource::TermWork,
            pattern.terms().len() as u128
                + pattern
                    .terms()
                    .iter()
                    .map(|term| match term {
                        CoreTerm::Constant(zetesis_core::Value::Structured(value)) => {
                            value.nodes().len() as u128
                        }
                        _ => 0,
                    })
                    .sum::<u128>(),
            self.location,
        )?;
        let bytes = pattern.predicate().name().len() as u128
            + std::mem::size_of_val(pattern.terms()) as u128
            + pattern
                .terms()
                .iter()
                .map(|term| match term {
                    CoreTerm::Constant(value) => crate::formula_ir::value_bytes(value),
                    CoreTerm::Variable(_) => 0,
                })
                .sum::<u128>();
        self.budget
            .charge(ExpansionResource::ScalarBytes, bytes, self.location)?;
        Ok(pattern.clone())
    }

    pub(super) fn consequent_slot(
        &self,
        variables: &mut Variables,
    ) -> Result<usize, FormulaFailure> {
        ceiling(
            FormulaResource::Variables,
            variables.count as u128 + 1,
            self.options.core_limits.max_variables_per_template as u128,
            self.location,
        )?;
        Ok(variables.slot(&Variable::Anonymous))
    }
}
