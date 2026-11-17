//! Checked in-place sorting of private metadata slices.
//!
//! Each comparison admits a permit before reading its cells; the comparator
//! admits any further payload navigation. Each swap admits its own permit.
//! Refusal preserves the population but may leave a partial permutation.
//! Heapsort allocates nothing and uses constant scratch and O(n log n)
//! comparisons. Callers retain the slice's existing capacity receipt.
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

/// Bottom-up merge sort over block widths 1, 2, 4, ...
///
/// The comparisons performed depend only on the input and the fixed block
/// boundaries, so the merges of one width are independent and may run on
/// separate workers with identical charged work. One permit per comparison
/// (the comparator admits its own navigation) and one per merged block. It
/// performs at most n * ceil(log2 n) comparisons, about half of heapsort's,
/// in sequential passes over contiguous memory. `scratch` must be empty with
/// capacity for `values`; each pass writes it and then swaps, so a refusal
/// leaves `values` an intact permutation of the population.
pub(crate) fn merge_by<T: Copy>(
    values: &mut Vec<T>,
    scratch: &mut Vec<T>,
    mut work: GroundingWork<'_>,
    mut compare: impl FnMut(&T, &T, &mut GroundingWork<'_>) -> Result<Ordering, FormulaFailure>,
) -> Result<(), FormulaFailure> {
    let count = values.len();
    debug_assert!(scratch.is_empty() && scratch.capacity() >= count);
    let mut width = 1;
    while width < count {
        scratch.clear();
        let mut start = 0;
        while start < count {
            work.counters.work(work.limits, work.location)?;
            let middle = (start + width).min(count);
            let end = (start + 2 * width).min(count);
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
            start = end;
        }
        std::mem::swap(values, scratch);
        width *= 2;
    }
    scratch.clear();
    Ok(())
}

#[cfg(test)]
mod tests;
