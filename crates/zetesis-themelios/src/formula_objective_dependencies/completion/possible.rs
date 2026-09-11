//! One completed possible relation is the authoritative conservative carrier.
//! Membership covers possible truth and never establishes realizability.

use super::{Activity, Completion, Context};
use crate::formula_support::{self, Support};
use crate::{ExpansionResource, FormulaFailure};
use zetesis_core::Atom;

impl Completion {
    pub(super) fn possible(
        &mut self,
        predicate: &zetesis_core::Predicate,
        support: &Support<'_>,
        temporary: usize,
        context: &mut Context<'_>,
    ) -> Result<(), FormulaFailure> {
        context.work()?;
        for row in support.rows(predicate) {
            context.work()?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(predicate.arity())
                .map_err(|_| context.allocation())?;
            for value in formula_support::row_values(row) {
                context.work()?;
                values.push(formula_support::copy(
                    value,
                    context.budget,
                    context.location,
                )?);
            }
            context.budget.charge(
                ExpansionResource::ScalarBytes,
                predicate.name().len() as u128,
                context.location,
            )?;
            let atom = Atom::new(predicate.clone(), values).expect("completed support arity");
            if !self.atoms.contains_key(&atom) {
                context.entries(temporary.saturating_add(self.atoms.len()).saturating_add(1))?;
                self.atoms.insert(atom, Activity::Optional);
            }
        }
        Ok(())
    }
}
