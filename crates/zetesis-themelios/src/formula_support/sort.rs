//! Checked in-place sorting of private metadata slices.
//!
//! Each comparison admits a permit before reading its cells; the comparator
//! admits any further payload navigation. Each swap admits its own permit.
//! Refusal preserves the population but may leave a partial permutation.
//! Heapsort allocates nothing and uses constant scratch and O(n log n)
//! comparisons; it first confirms ordered input in at most n - 1 comparisons. Callers retain the slice's existing capacity receipt.
//! Successful sortedness requires a consistent total comparator; permutation
//! preservation does not. Private partially sorted metadata is not publication.

use std::cmp::Ordering;

use super::GroundingWork;
use crate::FormulaFailure;

pub(crate) fn by<T>(
    values: &mut [T],
    mut work: GroundingWork<'_>,
    mut compare: impl FnMut(&T, &T, &mut GroundingWork<'_>) -> Result<Ordering, FormulaFailure>,
) -> Result<(), FormulaFailure> {
    let count = values.len();
    // Ordered input is common (rows arrive in index order): confirm it in
    // n - 1 comparisons before building the heap.
    let mut sorted = true;
    for index in 1..count {
        work.counters.work(work.limits, work.location)?;
        if compare(&values[index], &values[index - 1], &mut work)?.is_lt() {
            sorted = false;
            break;
        }
    }
    if sorted {
        return Ok(());
    }
    for root in (0..count / 2).rev() {
        sift(values, root, count, &mut work, &mut compare)?;
    }
    for end in (1..count).rev() {
        work.counters.work(work.limits, work.location)?;
        values.swap(0, end);
        sift(values, 0, end, &mut work, &mut compare)?;
    }
    Ok(())
}

fn sift<T>(
    values: &mut [T],
    mut root: usize,
    end: usize,
    work: &mut GroundingWork<'_>,
    compare: &mut impl FnMut(&T, &T, &mut GroundingWork<'_>) -> Result<Ordering, FormulaFailure>,
) -> Result<(), FormulaFailure> {
    while root < end / 2 {
        // This guard bounds root * 2 + 1 below end, including at usize's limit.
        let mut child = root * 2 + 1;
        if child + 1 < end {
            work.counters.work(work.limits, work.location)?;
            if compare(&values[child], &values[child + 1], work)?.is_lt() {
                child += 1;
            }
        }
        work.counters.work(work.limits, work.location)?;
        if !compare(&values[root], &values[child], work)?.is_lt() {
            break;
        }
        work.counters.work(work.limits, work.location)?;
        values.swap(root, child);
        root = child;
    }
    Ok(())
}

/// Natural merge sort: one pass splits `values` into maximal ascending runs,
/// then adjacent runs merge pairwise until one remains.
///
/// The comparisons performed depend only on the input and the run boundaries,
/// so the merges of one pass are independent and may run on separate workers
/// with identical charged work. One permit per comparison (the comparator
/// admits its own navigation) and one per merged run pair. Input already in
/// order costs n - 1 comparisons. A merge gallops once one side has won
/// [`GALLOP`] comparisons in a row: it probes that side at doubling offsets
/// and then bisects, so a block of k values that interleaves with nothing
/// costs about 2 log2 k comparisons instead of k. Interleaved runs merge as a
/// plain merge does, plus at most one probe per gallop. `scratch` must be
/// empty with capacity for `values`, and `runs` empty with capacity for one
/// more; each pass writes `scratch` and then swaps, so a refusal leaves
/// `values` an intact permutation of the population.
pub(crate) fn merge_by<T: Copy>(
    values: &mut Vec<T>,
    scratch: &mut Vec<T>,
    runs: &mut Vec<usize>,
    mut work: GroundingWork<'_>,
    mut compare: impl FnMut(&T, &T, &mut GroundingWork<'_>) -> Result<Ordering, FormulaFailure>,
) -> Result<(), FormulaFailure> {
    let count = values.len();
    debug_assert!(scratch.is_empty() && scratch.capacity() >= count);
    debug_assert!(runs.is_empty() && runs.capacity() > count);
    runs.push(0);
    for index in 1..count {
        work.counters.work(work.limits, work.location)?;
        if compare(&values[index], &values[index - 1], &mut work)?.is_lt() {
            runs.push(index);
        }
    }
    runs.push(count);
    while runs.len() > 2 {
        scratch.clear();
        let mut kept = 0;
        let mut run = 0;
        while run + 1 < runs.len() {
            work.counters.work(work.limits, work.location)?;
            let (start, middle) = (runs[run], runs[run + 1]);
            let end = runs.get(run + 2).copied().unwrap_or(middle);
            merge_pair(
                values,
                scratch,
                (start, middle, end),
                &mut work,
                &mut compare,
            )?;
            runs[kept] = start;
            kept += 1;
            run += 2;
        }
        runs[kept] = count;
        runs.truncate(kept + 1);
        std::mem::swap(values, scratch);
    }
    scratch.clear();
    runs.clear();
    Ok(())
}

/// Consecutive wins by one side of a merge before it gallops.
const GALLOP: usize = 7;

/// Merge the ascending runs `values[start..middle]` and
/// `values[middle..end]` onto `scratch`. Ties take the left value first.
fn merge_pair<T: Copy>(
    values: &[T],
    scratch: &mut Vec<T>,
    (start, middle, end): (usize, usize, usize),
    work: &mut GroundingWork<'_>,
    compare: &mut impl FnMut(&T, &T, &mut GroundingWork<'_>) -> Result<Ordering, FormulaFailure>,
) -> Result<(), FormulaFailure> {
    let (mut left, mut right) = (start, middle);
    // The side that won the last comparisons (true for right) and how often.
    let (mut side, mut wins) = (false, 0);
    while left < middle && right < end {
        if wins >= GALLOP {
            if side {
                // Right values strictly below the left head.
                let pivot = values[left];
                let to = gallop(right, end, work, |index, work| {
                    Ok(compare(&values[index], &pivot, work)?.is_lt())
                })?;
                scratch.extend_from_slice(&values[right..to]);
                right = to;
            } else {
                // Left values not above the right head.
                let pivot = values[right];
                let to = gallop(left, middle, work, |index, work| {
                    Ok(!compare(&pivot, &values[index], work)?.is_lt())
                })?;
                scratch.extend_from_slice(&values[left..to]);
                left = to;
            }
            // A gallop that stopped inside its run did so on a probe that
            // already compared the two heads: the other side's head is next.
            if side && right < end && left < middle {
                scratch.push(values[left]);
                left += 1;
            } else if !side && left < middle && right < end {
                scratch.push(values[right]);
                right += 1;
            }
            (side, wins) = (!side, 1);
            continue;
        }
        work.counters.work(work.limits, work.location)?;
        let won = compare(&values[right], &values[left], work)?.is_lt();
        if won {
            scratch.push(values[right]);
            right += 1;
        } else {
            scratch.push(values[left]);
            left += 1;
        }
        if won == side {
            wins += 1;
        } else {
            (side, wins) = (won, 1);
        }
    }
    scratch.extend_from_slice(&values[left..middle]);
    scratch.extend_from_slice(&values[right..end]);
    Ok(())
}

/// The first index in `from..to` where `holds` fails, for a predicate that
/// holds on a prefix: probe offsets 1, 2, 4, ... and then bisect the last
/// gap. One permit per probe.
fn gallop(
    from: usize,
    to: usize,
    work: &mut GroundingWork<'_>,
    mut holds: impl FnMut(usize, &mut GroundingWork<'_>) -> Result<bool, FormulaFailure>,
) -> Result<usize, FormulaFailure> {
    // Invariant: holds on from..low; fails at high (or high == to).
    let (mut low, mut step) = (from, 1);
    let mut high = loop {
        let probe = from.saturating_add(step - 1);
        if probe >= to {
            break to;
        }
        work.counters.work(work.limits, work.location)?;
        if !holds(probe, work)? {
            break probe;
        }
        low = probe + 1;
        step = step.saturating_mul(2);
    };
    while low < high {
        let probe = low + (high - low) / 2;
        work.counters.work(work.limits, work.location)?;
        if holds(probe, work)? {
            low = probe + 1;
        } else {
            high = probe;
        }
    }
    Ok(high)
}

#[cfg(test)]
mod tests;
