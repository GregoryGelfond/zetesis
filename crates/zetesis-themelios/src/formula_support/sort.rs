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

use rayon::ThreadPool;
use rayon::prelude::*;

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

/// Most groups in one pass: the grounding executor's task geometry.
const GROUPS: usize = super::parallel_executor::TASKS;
/// Fewest values per group, so a small pass stays on one thread. Grouping
/// changes neither the result nor any charge.
const GROUP_VALUES: usize = 4096;

/// Bottom-up merge sort over block widths 1, 2, 4, ...
///
/// The merges of one width read disjoint blocks and write disjoint ranges,
/// so `pool` may run groups of whole merges on its threads while the caller
/// waits. Each group counts one permit per merge and per comparison, plus
/// the comparator's own navigation, and the pass charges their sum once it
/// ends: receipts are identical for every worker count, and a refusal is
/// found at the end of the pass that crosses the limit. It performs at most
/// n * ceil(log2 n) comparisons, about half of heapsort's, in passes over
/// contiguous memory. `scratch` must be empty with capacity for `values`;
/// each pass writes it and swaps only after its charge is admitted, so a
/// refusal leaves `values` an intact permutation of the population.
pub(crate) fn merge_by<T: Copy + Send + Sync>(
    values: &mut Vec<T>,
    scratch: &mut Vec<T>,
    pool: Option<&ThreadPool>,
    work: GroundingWork<'_>,
    compare: impl Fn(&T, &T, &mut u64) -> Ordering + Sync,
) -> Result<(), FormulaFailure> {
    let GroundingWork {
        limits,
        counters,
        location,
    } = work;
    let count = values.len();
    debug_assert!(scratch.is_empty() && scratch.capacity() >= count);
    let Some(&first) = values.first() else {
        return Ok(());
    };
    // Within the admitted capacity: no allocation.
    scratch.resize(count, first);
    let mut width = 1;
    while width < count {
        let span = 2 * width;
        let merges = count.div_ceil(span);
        let groups = merges.min(GROUPS).min(count.div_ceil(GROUP_VALUES));
        // Whole merges per group, so a group never splits a merge.
        let group_values = merges.div_ceil(groups) * span;
        let source = &values[..];
        let run = |(group, output): (usize, &mut [T])| {
            let first = group * group_values / span;
            let merges = first..first + output.len().div_ceil(span);
            merge_group(source, output, merges, width, &compare)
        };
        let total: u64 = match pool {
            Some(pool) if groups > 1 => pool.install(|| {
                scratch
                    .par_chunks_mut(group_values)
                    .enumerate()
                    .map(run)
                    .sum()
            }),
            _ => scratch.chunks_mut(group_values).enumerate().map(run).sum(),
        };
        counters.charge_work(u128::from(total), limits, location)?;
        std::mem::swap(values, scratch);
        width = span;
    }
    scratch.clear();
    Ok(())
}

/// Merge the block pairs `merges` of width `width` from `source` into
/// `output`, which starts at the first pair. Returns the permits counted.
fn merge_group<T: Copy>(
    source: &[T],
    output: &mut [T],
    merges: std::ops::Range<usize>,
    width: usize,
    compare: &impl Fn(&T, &T, &mut u64) -> Ordering,
) -> u64 {
    let count = source.len();
    let mut charged = 0;
    let mut at = 0;
    for merge in merges {
        charged += 1;
        let start = merge * 2 * width;
        let middle = (start + width).min(count);
        let end = (start + 2 * width).min(count);
        let (mut left, mut right) = (start, middle);
        while left < middle && right < end {
            charged += 1;
            if compare(&source[right], &source[left], &mut charged).is_lt() {
                output[at] = source[right];
                right += 1;
            } else {
                output[at] = source[left];
                left += 1;
            }
            at += 1;
        }
        for range in [left..middle, right..end] {
            let length = range.len();
            output[at..at + length].copy_from_slice(&source[range]);
            at += length;
        }
    }
    charged
}

#[cfg(test)]
mod tests;
