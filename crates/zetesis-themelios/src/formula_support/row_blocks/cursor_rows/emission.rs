//! Formula emission retains every complete witness, including existing heads.

use super::{Computation, Counters, CursorRows, Join, RuleIr};
use crate::{FormulaFailure, FormulaLimits, JoinStrategy};

impl<'cursor, 'source> CursorRows<'cursor, 'source> {
    /// The shared initial checks stay authoritative. The sole extra capability
    /// is a family owner proved unused by the checked positive-flat grammar.
    /// Table caches retain their one-owner entry allowance on the serial path.
    pub(crate) fn prepare_emission<'plan>(
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
        Self::prepare_kind(
            rule,
            cursor,
            computation,
            limits,
            counters,
            super::Kind::Emission,
        )
    }
}

#[cfg(test)]
mod tests;
