//! Integer comparison decisions: a comparison literal whose operands both have
//! integer operand forms, compiled once with the prepared program, is decided
//! on a row with machine integers.
//!
//! Every evaluation site of a comparison literal tries this decision first:
//! the join's decided prefix comparisons, its complete-row filters and the
//! rejected-cursor checks. On a row the decision admits one work charge of the
//! two forms' constant, `nodes + variable steps`, before evaluating, and
//! records the attempt's two evaluations as entered and its nodes as admitted.
//! It then reads each variable its operands name and evaluates both operands
//! in node order with the checked scalar operations. When every read gives a
//! number and every operation succeeds, the comparison's value is the relation
//! applied to the order of the two integers, which is the ASP order restricted
//! to numbers: no term is constructed and no projection is read or written.
//!
//! Any other row declines: a slot that is unbound, unavailable or unreadable, a
//! value that is not a number, or a failing operation. The caller's general
//! path then decides the row with its own charges, diagnostics, failure
//! retention and zero-divisor classification. A literal without a form goes
//! to the general path directly. Whether the decision decides a row depends
//! only on the literal's operands and the row's values, so every join takes
//! the same route on the same literal and values, in capture and in a frozen
//! check alike.
//!
//! The decision's own failure is a work or cancellation refusal at its one
//! charge, located at the comparison's site. It allocates nothing; an
//! attempt that declines early performs fewer operations than it charged, at
//! most `2·MAX_NODES` operations and as many reads.

use themelios_program::program::Relation;
use zetesis_core::ValueNodeRef;

use super::{Computation, Context, GroundingWork, relation_holds};
use crate::FormulaFailure;
use crate::formula_binding::Binding;
use crate::formula_ir::Expression;
use crate::grounding_observer::Event;

/// Decide `left relation right` on `binding` with the operands' integer forms:
/// `Some` with the comparison's value, or `None` when the literal has no form
/// or the row declines and the general path must decide it.
///
/// # Errors
/// A work or cancellation refusal of the one charge, made before evaluating.
pub(super) fn decide(
    ([left, right], relation): ([&Expression; 2], Relation),
    binding: &Binding<'_>,
    context: Context<'_, &Computation<'_, '_>>,
) -> Result<Option<bool>, FormulaFailure> {
    let (Ok(left), Ok(right)) = (left.integer(), right.integer()) else {
        return Ok(None);
    };
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    // Entered like the general evaluator's, before the charge that may refuse.
    counters.record(Event::ExpressionEvaluation);
    counters.record(Event::ExpressionEvaluation);
    counters.charge_work(
        u128::from(left.charge()) + u128::from(right.charge()),
        limits,
        location,
    )?;
    for _ in 0..left.nodes() + right.nodes() {
        counters.record(Event::ExpressionNode);
    }
    let read = computation.read();
    let number = |slot| {
        let term = binding.read_with(slot, read, location, || Ok(())).ok()?;
        match term.descriptor() {
            ValueNodeRef::Number(number) => Some(number),
            _ => None,
        }
    };
    let Some((left, right)) = left
        .evaluate(number)
        .and_then(|left| Some((left, right.evaluate(number)?)))
    else {
        counters.record(Event::IntegerComparisonDeclined);
        return Ok(None);
    };
    counters.record(Event::IntegerComparison);
    Ok(Some(relation_holds(relation, left.cmp(&right))))
}

#[cfg(test)]
mod tests;
