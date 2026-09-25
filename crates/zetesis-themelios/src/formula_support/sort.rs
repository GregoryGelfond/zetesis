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

#[cfg(test)]
mod tests;
