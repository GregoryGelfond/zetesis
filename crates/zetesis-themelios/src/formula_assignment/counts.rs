//! Ordered count proposals from already coalesced unit contributions.

#[cfg(test)]
mod tests;

use crate::ProgramSite;
use crate::formula::ceiling;
use crate::formula_support::{Buffer, Computation, Counters};
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource};
use themelios_program::term::EvalError;

/// The counts of every selection between `required` tuples and those plus the
/// optional unit contributions: after k optional contributions the buffer is
/// exactly `required` through `required + k`. Appending the next checked value
/// preserves all selection cardinalities without merging runs.
pub(super) fn counts(
    required: usize,
    weights: impl IntoIterator<Item = i32>,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Buffer<i32>, FormulaFailure> {
    // Retain the empty family's first refusal before work or storage admission.
    ceiling(
        FormulaResource::AssignmentValues,
        1,
        limits.max_assignment_values as u128,
        location,
    )?;
    let mut maximum = i32::try_from(required).map_err(|_| ExpansionFailure::Evaluation {
        error: EvalError::Overflow,
        location,
    })?;
    let mut values = Buffer::new(computation, limits, counters, location)?;
    values.push(maximum, computation, limits, counters, location)?;
    for weight in weights {
        counters.work(limits, location)?;
        assert_eq!(weight, 1, "count contributions are unit weights");
        maximum = maximum.checked_add(1).ok_or(ExpansionFailure::Evaluation {
            error: EvalError::Overflow,
            location,
        })?;
        ceiling(
            FormulaResource::AssignmentValues,
            values.len() as u128 + 1,
            limits.max_assignment_values as u128,
            location,
        )?;
        values.push(maximum, computation, limits, counters, location)?;
    }
    Ok(values)
}
