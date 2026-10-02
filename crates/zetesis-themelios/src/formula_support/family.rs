//! Evidence over complete substitutions, independent of relational join order.
//!
//! Presence, rather than counts, is sufficient: a defined false instance is a
//! witness, an independently excluded erroneous instance is not, and a missing
//! relational extension contributes nothing. A cursor over a partial support
//! round may collect evidence but must never finalize it.

use themelios_base::span::Location;
use themelios_program::term::BinaryOp;

use super::Counters;
use crate::formula::ceiling;
use crate::formula_ir::{Expression, LiteralIr, Operation};
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource, FormulaWarning};

#[derive(Default)]
pub(crate) struct Evidence {
    pub(crate) defined: bool,
    pub(crate) zero: Option<ExpansionFailure>,
}

impl Evidence {
    pub(crate) fn merge(&mut self, other: Self) {
        self.defined |= other.defined;
        if self.zero.is_none() {
            self.zero = other.zero;
        }
    }

    /// Only a traversal of the completed original family may call this.
    pub(crate) fn finish(self) -> Result<bool, FormulaFailure> {
        match self.zero {
            Some(error) if !self.defined => Err(error.into()),
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }
}

#[derive(Default)]
pub(crate) struct Warnings {
    values: Vec<FormulaWarning>,
}

impl Warnings {
    pub(crate) fn family(
        &mut self,
        evidence: Evidence,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        if evidence.finish()? {
            self.insert(limits, counters, location)?;
        }
        Ok(())
    }

    pub(crate) fn insert(
        &mut self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let key =
            |location: Location| (location.source, location.span.start(), location.span.end());
        let mut low = 0;
        let mut high = self.values.len();
        while low < high {
            counters.work(limits, location)?;
            let middle = low + (high - low) / 2;
            match key(self.values[middle].location()).cmp(&key(location)) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Equal => return Ok(()),
                std::cmp::Ordering::Greater => high = middle,
            }
        }
        ceiling(
            FormulaResource::Warnings,
            self.values.len() as u128 + 1,
            limits.max_warnings as u128,
            location,
        )?;
        counters.charge_work((self.values.len() - low + 1) as u128, limits, location)?;
        self.values
            .try_reserve(1)
            .map_err(|_| FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Allocation,
                location,
            })?;
        self.values
            .insert(low, FormulaWarning::ZeroDivisor { location });
        Ok(())
    }

    pub(crate) fn into_values(self) -> Vec<FormulaWarning> {
        self.values
    }
}

pub(crate) fn partial(literals: &[LiteralIr]) -> bool {
    literals.iter().any(|literal| match literal {
        LiteralIr::Compare(left, _, right)
        | LiteralIr::ArgumentCheck {
            captured: left,
            value: right,
        } => expression(left) || expression(right),
        LiteralIr::TupleCompare(left, _, right) => left.iter().chain(right).any(expression),
        LiteralIr::Guard(guard) | LiteralIr::HeadGuard(guard) => {
            guard.expressions().any(expression)
        }
        LiteralIr::Bind { value, .. } => expression(value),
        LiteralIr::Aggregate(aggregate) => aggregate
            .guards
            .iter()
            .any(|guard| expression(&guard.bound)),
        LiteralIr::Range { lower, upper, .. } => expression(lower) || expression(upper),
        _ => false,
    })
}

pub(crate) fn expression(expression: &Expression) -> bool {
    expression
        .nodes
        .iter()
        .any(|node| matches!(node, Operation::Binary(BinaryOp::Div | BinaryOp::Mod, ..)))
}
