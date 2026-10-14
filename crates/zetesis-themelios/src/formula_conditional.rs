//! Finite universal conditionals retain each original condition implication.

use crate::formula_binding::Binding;

use crate::ProgramSite;
use themelios_program::program::DefaultNegation;
use zetesis_ferraris::NodeView as Node;

use crate::FormulaFailure;
use crate::formula_conditional_ir::{
    Alternative, ConditionalIr, Consequent, ConsequentOperand, GuardAlternative,
};
use crate::formula_ground::{Builder, FALSUM, VERUM, boolean};
use crate::formula_support::family::Evidence;
use crate::formula_support::{Context, Ir, Join, Support};

/// Arithmetic omission removes the whole local implication. A defined false
/// consequent, including an empty witness disjunction, remains a formula value.
enum ConsequentInstance {
    Defined(usize),
    Omitted,
}

impl ConsequentInstance {
    fn alternatives(value: usize, evidence: &Evidence) -> Self {
        if evidence.zero.is_some() && !evidence.defined {
            Self::Omitted
        } else {
            Self::Defined(value)
        }
    }
}

impl<'source> Builder<'_, '_, 'source> {
    pub(super) fn conditional(
        &mut self,
        conditional: Ir<'_, 'source, ConditionalIr>,
        assignment: &Binding,
        support: &Support<'source>,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        let mut result = VERUM;
        let mut bindings = Join::local(
            conditional.map(|conditional| conditional.condition.as_slice()),
            assignment,
            conditional.variables,
            support,
            self.budget,
            Context::new(&*self.computation, self.limits, self.counters, location),
        )?;
        if let Consequent::Guard(guard) = &conditional.consequent {
            bindings.check_guard(guard);
        }
        while let Some(binding) = bindings.next(
            self.computation,
            self.limits,
            self.budget,
            self.counters,
            location,
        )? {
            self.work(location)?;
            let condition = self.body_ir(
                conditional
                    .map(|conditional| conditional.condition.as_slice())
                    .iter(),
                &binding,
                location,
                support,
            )?;
            let consequent = match &conditional.consequent {
                Consequent::Atoms(negation, _) => self.atom_consequent(
                    *negation,
                    conditional.map(|conditional| {
                        let Consequent::Atoms(_, alternatives) = &conditional.consequent else {
                            unreachable!("atom consequent")
                        };
                        alternatives.as_slice()
                    }),
                    &binding,
                    support,
                    location,
                )?,
                Consequent::Guards(_) => self.guard_consequent(
                    conditional.map(|conditional| {
                        let Consequent::Guards(alternatives) = &conditional.consequent else {
                            unreachable!("guard consequent")
                        };
                        alternatives.as_slice()
                    }),
                    &binding,
                    support,
                    location,
                )?,
                Consequent::Guard(guard) => ConsequentInstance::Defined(boolean(guard.evaluate(
                    &binding,
                    self.computation,
                    self.limits,
                    self.counters,
                    location,
                )?)),
            };
            let ConsequentInstance::Defined(consequent) = consequent else {
                continue;
            };
            let implication = self.node(Node::Implies(condition, consequent), location)?;
            result = self.and(result, implication, location)?;
        }
        // Only successful exhaustion establishes vacuity. Resource errors above
        // propagate instead of returning the conjunction of a partial prefix.
        Ok(result)
    }

    fn atom_consequent(
        &mut self,
        negation: DefaultNegation,
        alternatives: Ir<'_, 'source, [Alternative]>,
        binding: &Binding,
        support: &Support<'source>,
        location: ProgramSite,
    ) -> Result<ConsequentInstance, FormulaFailure> {
        let mut disjunction = FALSUM;
        let mut evidence = Evidence::default();
        for alternative in alternatives.iter() {
            let mut rows = Join::local(
                alternative.map(|alternative| alternative.bindings.as_slice()),
                binding,
                alternative.variables,
                support,
                self.budget,
                Context::new(&*self.computation, self.limits, self.counters, location),
            )?;
            while let Some(row) = rows.next(
                self.computation,
                self.limits,
                self.budget,
                self.counters,
                location,
            )? {
                self.work(location)?;
                let mut value = match &alternative.operand {
                    ConsequentOperand::Atom(atom) => self.atom(*atom, &row, location)?,
                    ConsequentOperand::Projection(_) => self.project(
                        alternative.map(|alternative| {
                            let ConsequentOperand::Projection(projection) = &alternative.operand
                            else {
                                unreachable!("projection operand")
                            };
                            projection
                        }),
                        &row,
                        support,
                        location,
                    )?,
                };
                if negation != DefaultNegation::None {
                    value = self.neg(value, location)?;
                }
                if negation == DefaultNegation::NotNot {
                    value = self.neg(value, location)?;
                }
                disjunction = self.or(disjunction, value, location)?;
            }
            evidence.merge(rows.take_family());
        }
        Ok(ConsequentInstance::alternatives(disjunction, &evidence))
    }

    fn guard_consequent(
        &mut self,
        alternatives: Ir<'_, 'source, [GuardAlternative]>,
        binding: &Binding,
        support: &Support<'source>,
        location: ProgramSite,
    ) -> Result<ConsequentInstance, FormulaFailure> {
        let mut value = false;
        let mut evidence = Evidence::default();
        for alternative in alternatives.iter() {
            let mut rows = Join::local(
                alternative.map(|alternative| alternative.bindings.as_slice()),
                binding,
                alternative.variables,
                support,
                self.budget,
                Context::new(&*self.computation, self.limits, self.counters, location),
            )?;
            rows.check_guard(&alternative.guard);
            while let Some(row) = rows.next(
                self.computation,
                self.limits,
                self.budget,
                self.counters,
                location,
            )? {
                // Every value alternative is evaluated, including those after
                // a true result: errors cannot disappear.
                self.work(location)?;
                value |= alternative.guard.evaluate(
                    &row,
                    self.computation,
                    self.limits,
                    self.counters,
                    location,
                )?;
            }
            evidence.merge(rows.take_family());
        }
        Ok(ConsequentInstance::alternatives(boolean(value), &evidence))
    }
}
