//! Integer comparison decisions: a comparison literal with an integer
//! comparison form, compiled once with the prepared program, is decided on a
//! row with machine integers.
//!
//! Every evaluation site of a comparison literal tries this decision first:
//! the join's decided prefix comparisons, its complete-row filters and the
//! rejected-cursor checks. On a row the decision checks the form's constant,
//! `nodes + variable steps` of both operands, against the work ceiling,
//! polling cancellation first, without counting it. It then attempts the
//! form: it reads each variable its operands name and evaluates both operands
//! in node order with the checked scalar operations. Afterwards it counts
//! what the attempt performed, all within what the check admitted: the units
//! of the steps it reached, the failing one included, one evaluation per
//! operand it entered and one node per step it reached. When every read gives
//! a number and every operation succeeds, the comparison's value is the
//! relation applied to the order of the two integers, which is the ASP order
//! restricted to numbers: no term is constructed and no projection is read or
//! written.
//!
//! Any other row declines: a slot that is unbound, unavailable or unreadable, a
//! value that is not a number, or a failing operation. The caller's general
//! path then decides the row with its own charges, diagnostics, failure
//! retention and zero-divisor classification. A literal without a form goes
//! to the general path directly. Whether the decision decides a row depends
//! only on the literal and the row's values, so every join takes the same
//! route on the same literal and values, in capture and in a frozen check
//! alike.
//!
//! The decision's own failure is a work or cancellation refusal at its one
//! check, located at the comparison's site; the count after the attempt
//! cannot fail. It allocates nothing.

use zetesis_core::ValueNodeRef;

use super::{Computation, Context, GroundingWork, relation_holds};
use crate::FormulaFailure;
use crate::formula_binding::Binding;
use crate::formula_ir::Comparison;
use crate::grounding_observer::Event;

/// Decide `comparison` on `binding` with its integer comparison form: `Some`
/// with the comparison's value, or `None` when the literal has no form or the
/// row declines and the general path must decide it.
///
/// # Errors
/// A work or cancellation refusal of the one check, made before evaluating.
pub(super) fn decide(
    comparison: &Comparison,
    binding: &Binding<'_>,
    context: Context<'_, &Computation<'_, '_>>,
) -> Result<Option<bool>, FormulaFailure> {
    let Ok(form) = comparison.form() else {
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
    counters.check_work(u128::from(form.charge()), limits, location)?;
    let read = computation.read();
    let attempt = form.attempt(|slot| {
        let term = binding.read_with(slot, read, location, || Ok(())).ok()?;
        match term.descriptor() {
            ValueNodeRef::Number(number) => Some(number),
            _ => None,
        }
    });
    debug_assert!(
        attempt.units <= form.charge(),
        "an attempt performs at most the constant its check admitted"
    );
    for _ in 0..attempt.entered {
        counters.record(Event::ExpressionEvaluation);
    }
    for _ in 0..attempt.steps {
        counters.record(Event::ExpressionNode);
    }
    counters.count_work(u64::from(attempt.units));
    let Some(order) = attempt.order else {
        counters.record(Event::IntegerComparisonDeclined);
        return Ok(None);
    };
    counters.record(Event::IntegerComparison);
    Ok(Some(relation_holds(comparison.relation(), order)))
}

#[cfg(test)]
mod tests;
