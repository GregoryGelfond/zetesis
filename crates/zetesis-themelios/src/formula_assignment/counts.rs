//! Ordered count proposals from already coalesced unit contributions.

#[cfg(test)]
mod tests;

use crate::ProgramSite;
use crate::formula::ceiling;
use crate::formula_support::{Buffer, Computation, Counters};
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource};
use themelios_program::term::EvalError;

/// After k unit contributions the buffer is exactly 0 through k. Appending the
/// next checked value preserves all subset cardinalities without merging runs.
/// Every contribution remains optional here, including unconditional source
/// witnesses: proposal generation does not suppress later source diagnostics.
pub(super) fn counts(
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
    let mut values = Buffer::new(computation, limits, counters, location)?;
    let mut maximum = 0_i32;
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
