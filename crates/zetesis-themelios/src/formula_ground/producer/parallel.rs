//! Initial formula emission reuses the same staged witness stream as support.
//!
//! The worker purpose retains every activation. All canonical nodes, origins,
//! atom identity and reached-subbody reuse stay in the existing serial owner.

#[cfg(test)]
mod tests;

use super::AtomPattern;
use super::{Body, Builder, Context, FormulaFailure, Join, RowHead, RuleIr, emit_row};
use crate::formula_support::{
    Executor,
    row_blocks::{
        coordinator,
        cursor_rows::CursorRows,
        witness_rows::{Replay, WitnessPlan},
    },
};

pub(super) fn rule<'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &'source RuleIr,
    outer: &mut Join<'_, 'source>,
    pattern: AtomPattern,
    executor: &Executor,
) -> Result<bool, FormulaFailure> {
    let Some(rows) = CursorRows::prepare_emission(
        rule,
        outer,
        builder.computation,
        builder.limits,
        builder.counters,
    )?
    else {
        return Ok(false);
    };
    let support = builder.computation.query_support();
    let plan = WitnessPlan::for_emission(
        rows.descriptor(),
        support,
        builder.computation,
        builder.limits,
        builder.counters,
    )?;
    let Some(mut completed) = coordinator::stage_emission_witnesses(
        executor,
        &plan,
        support,
        builder.budget,
        Context::new(
            builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        ),
    )?
    else {
        return Ok(false);
    };
    completed.attach_reservation(Context::new(
        &*builder.computation,
        builder.limits,
        builder.counters,
        rule.location,
    ))?;
    let mut replay = Replay::new(
        &plan,
        support,
        Context::new(
            &*builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        ),
    )?;
    let mut prepared: Option<(RowHead<'source>, Body)> = None;
    for batch in completed.drain() {
        let mut selected = replay.batch(&batch, builder.limits, builder.counters)?;
        while let Some(row) = selected.next(Context::new(
            &*builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        ))? {
            emit_row(&mut prepared, pattern, &row, builder, rule)?;
        }
    }
    Ok(true)
}
