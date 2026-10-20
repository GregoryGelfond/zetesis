//! Residual divisibility and condition forcing over grouped coefficients.
//!
//! After mandatory propagation drains, every compatible complete sum is the
//! processed base plus a multiple of the gcd of undecided coefficients. Missing
//! that lattice refutes a region; hitting it does not establish a possible model.
//! Removing one coefficient gives the corresponding necessary test for each
//! truth value of its condition. If one branch misses, the other is required.
//!
//! A cheap whole-row scan runs first. One fitting open coefficient and a small
//! positive gcd of other open coefficients exclude useful complement tests.
//! This ascending reduction can stop before reading the whole row; it stores no
//! condition-dependent state. Otherwise one reusable suffix reduction
//! and a forward prefix reduction test all complements without quadratic scans.
//! Each batch uses one drained snapshot: conditions are unique within a grouped
//! row, and publishing one merely queues its node. Pending events cannot change
//! another condition or the processed sums during this reduction. The prefix
//! retains each originally open coefficient even after its decision is queued.
//! Mandatory propagation drains those decisions before another residual batch.
//! Every new wave fixes an open condition; no numeric bound or reason is cached.

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
    /// Called only for original subjects after all mandatory propagation drains.
    /// Thus every sure/never term has updated the processed sums.
    pub(in crate::regions) fn threshold_residual(
        &mut self,
        index: &Narrower,
        row: usize,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let checked = self.threshold_residual_check(index, row, work)?;
        if checked == Step::Contradiction {
            return Ok(checked);
        }
        let data = &index.thresholds.rows[row];
        let state = self.known.thresholds[row];
        if !data.has_nonunit_coefficient() || !state.has_residual_interval() {
            return Ok(Step::Unchanged);
        }
        let upper = state.upper.expect("two-sided residual interval");
        let width = upper.checked_sub(state.lower).ok_or(Stop::InvalidProgram)?;
        // Every complement except the first fitting coefficient's retains
        // that coefficient. Its own complement has a lattice witness once a
        // positive partial gcd fits the width: later gcds only divide it.
        // Clipped branch endpoints are all-off/all-on witnesses after the
        // mandatory bound rules drain. The empty gcd (zero) is no witness.
        let mut fitting = false;
        let mut complement = 0;
        for &(condition, weight) in data.terms.iter().rev() {
            work.tick()?;
            if !fitting && weight > width {
                break;
            }
            if bit(self.known.sure, condition) || bit(self.known.never, condition) {
                continue;
            }
            if !fitting {
                fitting = true;
                continue;
            }
            if weight <= width {
                return Ok(Step::Unchanged);
            }
            complement = gcd(complement, weight, work)?;
            if complement > 0 && complement <= width {
                return Ok(Step::Unchanged);
            }
        }
        prepare_suffix(
            &data.terms,
            self.known.sure,
            self.known.never,
            &mut self.lists.residual_suffix,
            work,
        )?;
        let upper = upper.checked_sub(1).ok_or(Stop::InvalidProgram)?;
        let mut prefix = 0;
        let mut step = Step::Unchanged;
        for (position, &(condition, weight)) in data.terms.iter().enumerate() {
            work.tick()?;
            // Earlier publications touch only earlier, distinct conditions.
            // This condition still has its initial snapshot truth.
            if bit(self.known.sure, condition) || bit(self.known.never, condition) {
                continue;
            }
            let suffix = self.lists.residual_suffix[data.terms.len() - position - 1];
            let divisor = gcd(prefix, suffix, work)?;
            prefix = gcd(prefix, weight, work)?;
            // A unit lattice cannot improve the already drained bound rules.
            if divisor == 1 {
                continue;
            }
            let false_maximum = state
                .possible
                .checked_sub(weight)
                .ok_or(Stop::InvalidProgram)?;
            let true_base = state
                .known
                .checked_add(weight)
                .ok_or(Stop::InvalidProgram)?;
            work.tick()?;
            let false_excluded =
                branch_misses(state.known, false_maximum, state.lower, upper, divisor)?;
            work.tick()?;
            let true_excluded =
                branch_misses(true_base, state.possible, state.lower, upper, divisor)?;
            if false_excluded && true_excluded {
                return Ok(Step::Contradiction);
            }
            let decision = if false_excluded {
                self.sure(condition)
            } else if true_excluded {
                self.never(condition)
            } else {
                Step::Unchanged
            };
            if decision == Step::Contradiction {
                return Ok(decision);
            }
            step = step.join(decision);
        }
        Ok(step)
    }

    /// The allocation-free whole-row check precedes complement preparation.
    /// A partial gcd admitting an interval point cannot lead to refutation:
    /// every later gcd divides it and retains that point. Reading the small
    /// coefficients first exposes this exit without another immutable order.
    fn threshold_residual_check(
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
            self.lists.nodes.is_empty()
                && self.lists.threshold_rows.is_empty()
                && self.lists.heads.is_empty(),
            "residual sums require drained mandatory events"
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
        let width = upper.checked_sub(lower).ok_or(Stop::InvalidProgram)?;
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
            // This first undecided term was already read and charged. A
            // coefficient fitting the exclusive width supplies a point for
            // every final divisor, without a separate probe or modulo test.
            // Later terms are no smaller, so test this witness only once.
            if divisor == 0 && weight <= width {
                return Ok(Step::Unchanged);
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
        // Every positive divisor was tested when it last changed. Reaching
        // the end means that final progression already missed the interval.
        if divisor != 0 {
            return Ok(Step::Contradiction);
        }
        work.tick()?;
        Ok(if misses(state.known, lower, upper, divisor)? {
            Step::Contradiction
        } else {
            Step::Unchanged
        })
    }
}

/// Rebuild one reversed reduction from the drained condition masks. Capacity is
/// retained by the walker, but no truth-dependent entry survives a new batch.
/// The initial empty suffix and every copied/reduced position are charged.
fn prepare_suffix(
    terms: &[(usize, i128)],
    sure: &[u64],
    never: &[u64],
    suffixes: &mut Vec<i128>,
    work: &mut Work<'_>,
) -> Result<(), Stop> {
    suffixes.clear();
    let length = terms.len().checked_add(1).ok_or(Stop::Allocation)?;
    work.tick()?;
    suffixes
        .try_reserve_exact(length)
        .map_err(|_| Stop::Allocation)?;
    suffixes.push(0);
    let mut suffix = 0;
    for &(condition, weight) in terms.iter().rev() {
        work.tick()?;
        if !bit(sure, condition) && !bit(never, condition) {
            suffix = gcd(suffix, weight, work)?;
        }
        suffixes.push(suffix);
    }
    Ok(())
}

/// Intersect a hypothetical branch's attainable bounds with the required
/// interval before testing its necessary lattice. Empty intersections refute
/// directly; divisor zero represents the singleton base, not a modulo operation.
fn branch_misses(
    base: i128,
    maximum: i128,
    lower: i128,
    upper: i128,
    divisor: i128,
) -> Result<bool, Stop> {
    let lower = base.max(lower);
    let upper = maximum.min(upper);
    if lower > upper {
        return Ok(true);
    }
    misses(base, lower, upper, divisor)
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
