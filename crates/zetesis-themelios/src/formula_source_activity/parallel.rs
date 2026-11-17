//! Ordered producer-row replay inside one unchanged Activity dependency pass.
//!
//! Only positive-flat normal rules and total comparison leaves lend a cursor.
//! Workers discover immutable row occurrences; canonical atoms and Activity
//! updates remain with the original per-binding coordinator consumer.

#[cfg(test)]
mod tests;

use super::{RuleIr, Signature, SourceEligibility};
use crate::FormulaFailure;
use crate::formula_ground::SourceBodies;
use crate::formula_support::row_blocks::{
    coordinator,
    cursor_rows::CursorRows,
    witness_rows::{Batch, Replay, RowBinding, WitnessPlan},
};
use crate::formula_support::{Context, Executor, Join, Support};

#[derive(Clone, Copy)]
pub(super) struct Producer<'source, 'predicate> {
    pub(super) position: (usize, &'source RuleIr),
    pub(super) predicate: &'predicate Signature,
    pub(super) temporary: usize,
}

/// A declined descriptor leaves the same cursor available to ordinary traversal.
/// After staging starts, a refusal is propagated after all performed receipts
/// settle; it never retries the producer under a fresh allowance.
pub(super) fn try_rule<'source>(
    owner: &mut SourceEligibility,
    producer: Producer<'source, '_>,
    cursor: &mut Join<'_, 'source>,
    bodies: &mut SourceBodies<'_, '_, 'source, '_>,
    executor: &Executor,
) -> Result<bool, FormulaFailure> {
    let rule = producer.position.1;
    let location = rule.location;
    let support = bodies.support();
    let context = bodies.context(location);
    let Some(rows) = CursorRows::prepare_producer(
        rule,
        cursor,
        context.computation,
        context.limits,
        context.counters,
    )?
    else {
        return Ok(false);
    };
    // "Emission" is the existing full-row purpose: unlike Support, no row is
    // skipped merely because its head already occurs in completed support.
    let plan = WitnessPlan::for_emission(
        rows.descriptor(),
        support,
        context.computation,
        context.limits,
        context.counters,
    )?;
    let mut completed = coordinator::stage_witnesses(
        executor,
        &plan,
        context.budget,
        Context::new(
            &*context.computation,
            context.limits,
            context.counters,
            location,
        ),
    )?;
    completed.attach_reservation(Context::new(
        &*context.computation,
        context.limits,
        context.counters,
        location,
    ))?;
    replay_rule(owner, producer, &plan, &completed, support, bodies)?;
    Ok(true)
}

/// Replay one complete producer in original row order before its Round publishes.
fn replay_rule<'plan, 'source: 'plan>(
    owner: &mut SourceEligibility,
    producer: Producer<'source, '_>,
    plan: &'plan WitnessPlan<'plan, 'source>,
    completed: &coordinator::Completed<Batch<'plan, 'source>>,
    support: &'plan Support<'source>,
    bodies: &mut SourceBodies<'_, '_, 'source, '_>,
) -> Result<(), FormulaFailure> {
    let context = bodies.context(producer.position.1.location);
    let location = producer.position.1.location;
    let mut replay = Replay::new(
        plan,
        support,
        Context::new(
            &*context.computation,
            context.limits,
            context.counters,
            location,
        ),
    )?;
    let mut projected = None;
    for batch in completed.results() {
        let context = bodies.context(location);
        let mut selected = replay.batch(batch, context.limits, context.counters)?;
        while let Some(row) = {
            let context = bodies.context(location);
            selected.next(Context::new(
                &*context.computation,
                context.limits,
                context.counters,
                location,
            ))?
        } {
            let context = bodies.context(location);
            if projected.is_none() {
                projected = Some(RowBinding::new(
                    plan,
                    &row,
                    Context::new(
                        &*context.computation,
                        context.limits,
                        context.counters,
                        location,
                    ),
                )?);
            }
            let binding = projected
                .as_mut()
                .expect("row projection initialized")
                .read(
                    &row,
                    Context::new(
                        &*context.computation,
                        context.limits,
                        context.counters,
                        location,
                    ),
                )?;
            owner.rule_binding(producer, binding, bodies)?;
        }
    }
    Ok(())
}
