//! Finite universal conditionals retain each original condition implication.
//!
//! A consequent is the disjunction of its alternatives over the values they
//! take. An alternative that divides by zero takes none, so a consequent
//! whose every alternative does is false, whatever its sign: the implication
//! then requires its condition to be false. clingo grounds the same instance
//! as `#false : condition`. A condition that divides by zero has no instance
//! at all.

use crate::formula_binding::Binding;

use crate::ProgramSite;
use themelios_program::program::DefaultNegation;
use zetesis_ferraris::NodeView as Node;

use crate::formula_conditional_ir::{
    Alternative, ConditionalIr, Consequent, ConsequentOperand, GuardAlternative,
};
use crate::formula_ground::{Builder, FALSUM, VERUM, boolean};
use crate::formula_guard::Guard;
use crate::formula_support::{Context, Evaluation, Ir, Join, Support};
use crate::{ExpansionFailure, FormulaFailure};

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
                Consequent::Guard(guard) => boolean(self.guard_holds(guard, &binding, location)?),
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
    ) -> Result<usize, FormulaFailure> {
        let mut disjunction = FALSUM;
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
        }
        Ok(disjunction)
    }

    fn guard_consequent(
        &mut self,
        alternatives: Ir<'_, 'source, [GuardAlternative]>,
        binding: &Binding,
        support: &Support<'source>,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        let mut value = false;
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
                value |= self.guard_holds(&alternative.guard, &row, location)?;
            }
        }
        Ok(boolean(value))
    }

    /// Whether `guard` holds under `binding`. A guard that divides by zero
    /// does not hold; the family scan reports that zero divisor as its
    /// statement's warning. Any other failure is returned.
    fn guard_holds(
        &mut self,
        guard: &Guard,
        binding: &Binding,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        let mut evaluation = Evaluation::default();
        match guard.evaluate_in(
            binding,
            &mut evaluation,
            self.computation,
            self.limits,
            self.counters,
            location,
        ) {
            Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. }))
                if evaluation.zero_divisor() =>
            {
                Ok(false)
            }
            result => result,
        }
    }
}
