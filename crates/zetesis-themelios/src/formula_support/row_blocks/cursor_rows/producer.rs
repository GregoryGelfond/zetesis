//! `SourceEligibility` alone may borrow total immutable comparison leaves.
//! Existing cursor authority and the expanding-generator gate remain mandatory.

use super::{Computation, Counters, CursorRows, Join, Kind, RuleIr};
use crate::{FormulaFailure, FormulaLimits, JoinStrategy};

impl<'cursor, 'source> CursorRows<'cursor, 'source> {
    pub(crate) fn prepare_producer<'plan>(
        rule: &'source RuleIr,
        cursor: &'cursor mut Join<'plan, 'source>,
        computation: &mut Computation<'_, 'source>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Option<Self>, FormulaFailure>
    where
        'source: 'plan,
        'plan: 'cursor,
    {
        counters.work(limits, rule.location)?;
        if computation.query_support().strategy() == JoinStrategy::Table {
            return Ok(None);
        }
        Self::prepare_kind(rule, cursor, computation, limits, counters, Kind::Producer)
    }
}

#[cfg(test)]
mod tests;
