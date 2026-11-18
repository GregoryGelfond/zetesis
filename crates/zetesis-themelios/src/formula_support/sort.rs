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
/// order costs n - 1 comparisons; r runs cost at most n * ceil(log2 r) more,
/// so never more than a bottom-up merge sort's n * ceil(log2 n) plus the run
/// pass. `scratch` must be empty with capacity for `values`, and `runs` empty
/// with capacity for one more; each pass writes `scratch` and then swaps, so a
/// refusal leaves `values` an intact permutation of the population.
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
            let (mut left, mut right) = (start, middle);
            while left < middle && right < end {
                work.counters.work(work.limits, work.location)?;
                if compare(&values[right], &values[left], &mut work)?.is_lt() {
                    scratch.push(values[right]);
                    right += 1;
                } else {
                    scratch.push(values[left]);
                    left += 1;
                }
            }
            scratch.extend_from_slice(&values[left..middle]);
            scratch.extend_from_slice(&values[right..end]);
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

#[cfg(test)]
mod tests;
