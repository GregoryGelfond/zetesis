//! A residual divisibility check over the existing grouped coefficients.
//!
//! After node events drain, every compatible complete sum is the processed
//! base plus a multiple of the gcd of undecided coefficients. Missing that
//! lattice refutes a region; hitting it does not establish a possible model.
//! This original-only check neither publishes bounds nor changes formula truth.
//! A partial gcd admitting an interval point cannot lead to a refutation: every
//! later gcd divides it, so the final lattice still contains that point. Scan
//! small coefficients first to expose these exits without another term order.
//! It allocates nothing, charging terms, Euclidean steps and interval checks.
//! Dirty events coalesce, but successive event waves can still rescan a row.

use crate::regions::{Closure, Count, Narrower, Step, Work, bit};
use zetesis_cpu::Stop;

use super::State;

#[cfg(test)]
mod tests;

impl State {
    /// Either endpoint is already a lattice witness unless both requirements
    /// exclude it. This filter leaves one-sided requirements unchanged.
    pub(super) fn has_residual_interval(self) -> bool {
        self.lower > self.known && self.upper.is_some_and(|upper| upper <= self.possible)
    }
}

impl<C: Count> Closure<'_, C> {
    /// Node events have drained at this original row event. Transport may
    /// enqueue more nodes, so the residual check must run first. Arithmetic-only
    /// notifications do not revisit prefix or Boolean requirement neighbours.
    pub(in crate::regions) fn threshold_event(
        &mut self,
        index: &Narrower,
        row: usize,
        transfer: bool,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let residual = self.threshold_residual(index, row, work)?;
        if residual == Step::Contradiction || !transfer {
            Ok(residual)
        } else {
            self.threshold_requirements(index, row, work)
        }
    }

    /// Called only for original subjects, before row transport, with no pending
    /// node events. Thus every sure/never term has updated the processed sums.
    pub(in crate::regions) fn threshold_residual(
        &self,
        index: &Narrower,
        row: usize,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let data = &index.thresholds.rows[row];
        // Existing unit-row transport acquires no arithmetic scan or work tick.
        if !data.has_nonunit_coefficient() {
            return Ok(Step::Unchanged);
        }
        work.tick()?;
        debug_assert!(
            self.lists.nodes.is_empty(),
            "residual sums require drained events"
        );
        let state = self.known.thresholds[row];
        if !state.has_residual_interval() {
            return Ok(Step::Unchanged);
        }
        let upper = state.upper.expect("two-sided residual interval");
        let lower = state.known.max(state.lower);
        if lower >= upper || lower > state.possible {
            return Ok(Step::Contradiction);
        }
        let upper = state
            .possible
            .min(upper.checked_sub(1).ok_or(Stop::InvalidProgram)?);
        let mut divisor = 0;
        // Forcing keeps its descending order; this independent reduction reads
        // the same immutable coefficients from the small end.
        for &(condition, weight) in data.terms.iter().rev() {
            work.tick()?;
            if bit(self.known.sure, condition) || bit(self.known.never, condition) {
                continue;
            }
            let previous = divisor;
            divisor = gcd(divisor, weight, work)?;
            if divisor == 1 {
                return Ok(Step::Unchanged);
            }
            if divisor != previous {
                work.tick()?;
                if !misses(state.known, lower, upper, divisor)? {
                    return Ok(Step::Unchanged);
                }
            }
        }
        work.tick()?;
        Ok(if misses(state.known, lower, upper, divisor)? {
            Step::Contradiction
        } else {
            Step::Unchanged
        })
    }
}

/// Positive grouped coefficients fit i128, even when they exceed u64. Each
/// Euclidean step strictly decreases the nonnegative right operand.
fn gcd(mut left: i128, mut right: i128, work: &mut Work<'_>) -> Result<i128, Stop> {
    if left == 0 || left == right {
        return Ok(right);
    }
    while right != 0 {
        work.tick()?;
        let remainder = left.checked_rem(right).ok_or(Stop::InvalidProgram)?;
        left = right;
        right = remainder;
    }
    Ok(left)
}

/// Invariant: 0 <= base <= lower <= upper. Distance to the next lattice point
/// is compared with interval width, without adding to an extreme bound.
fn misses(base: i128, lower: i128, upper: i128, divisor: i128) -> Result<bool, Stop> {
    if divisor == 0 {
        return Ok(base < lower || base > upper);
    }
    let offset = lower.checked_sub(base).ok_or(Stop::InvalidProgram)?;
    let remainder = offset.checked_rem(divisor).ok_or(Stop::InvalidProgram)?;
    let distance = if remainder == 0 {
        0
    } else {
        divisor - remainder
    };
    let width = upper.checked_sub(lower).ok_or(Stop::InvalidProgram)?;
    Ok(distance > width)
}
