//! Finite universal conditionals retain each original condition implication.

use themelios_base::span::Location;
use themelios_program::program::DefaultNegation;
use zetesis_core::Value;
use zetesis_ferraris::Node;

use crate::FormulaFailure;
use crate::formula_conditional_ir::{ConditionalIr, Consequent};
use crate::formula_ground::Builder;
use crate::formula_support::{Join, Support};

impl Builder<'_> {
    pub(super) fn conditional(
        &mut self,
        conditional: &ConditionalIr,
        assignment: &[Value],
        support: &Support,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        let mut result = 1;
        let mut bindings = Join::new(
            &conditional.condition,
            assignment,
            conditional.variables,
            support,
            self.budget,
            location,
        )?;
        while let Some(binding) =
            bindings.next(self.limits, self.budget, &mut self.counters, location)?
        {
            self.work(location)?;
            let condition = self.body(&conditional.condition, &binding, location, support)?;
            let consequent = match &conditional.consequent {
                Consequent::Atoms(negation, alternatives) => {
                    let mut disjunction = 0;
                    for alternative in alternatives {
                        let mut rows = Join::new(
                            &alternative.bindings,
                            &binding,
                            alternative.variables,
                            support,
                            self.budget,
                            location,
                        )?;
                        while let Some(row) =
                            rows.next(self.limits, self.budget, &mut self.counters, location)?
                        {
                            self.work(location)?;
                            let mut value = self.atom(&alternative.atom, &row, location)?;
                            if *negation != DefaultNegation::None {
                                value = self.neg(value, location)?;
                            }
                            if *negation == DefaultNegation::NotNot {
                                value = self.neg(value, location)?;
                            }
                            disjunction = self.or(disjunction, value, location)?;
                        }
                    }
                    disjunction
                }
                Consequent::Guard(guard) => usize::from(guard.evaluate(
                    &binding,
                    self.limits,
                    self.budget,
                    &mut self.counters,
                    location,
                )?),
            };
            let implication = self.node(Node::Implies(condition, consequent), location)?;
            result = self.and(result, implication, location)?;
        }
        // Only successful exhaustion establishes vacuity. Resource errors above
        // propagate instead of returning the conjunction of a partial prefix.
        Ok(result)
    }
}
