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
use std::sync::atomic::{AtomicU64, Ordering as Atomic};

use rayon::ThreadPool;

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

/// Most groups in one pass: the grounding executor's task geometry.
const GROUPS: usize = super::parallel_executor::TASKS;
/// Fewest values per group, so a small pass stays on one thread. Grouping
/// changes neither the result nor any charge.
const GROUP_VALUES: usize = 4096;

/// Natural merge sort: one pass splits `values` into maximal ascending runs,
/// then adjacent runs merge pairwise until one remains.
///
/// The comparisons performed depend only on the input and the run boundaries,
/// so the merges of one pass read disjoint runs and write disjoint ranges:
/// `pool` may run groups of whole merges on its threads while the caller
/// waits. Each pass counts one permit per merge and per comparison, plus the
/// comparator's own navigation, and charges their sum once it ends, so
/// receipts are identical for every worker count and a refusal is found at
/// the end of the pass that crosses the limit. Input already in order costs
/// n - 1 comparisons. A merge gallops once one side has won [`GALLOP`]
/// comparisons in a row: it probes that side at doubling offsets and then
/// bisects, so a block of k values that interleaves with nothing costs about
/// 2 log2 k comparisons instead of k. `scratch` must be empty with capacity
/// for `values`, and `runs` empty with capacity for one more; each pass
/// writes `scratch` and swaps only after its charge is admitted, so a refusal
/// leaves `values` an intact permutation of the population.
pub(crate) fn merge_by<T: Copy + Send + Sync>(
    values: &mut Vec<T>,
    scratch: &mut Vec<T>,
    runs: &mut Vec<usize>,
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
    debug_assert!(runs.is_empty() && runs.capacity() > count);
    let Some(&first) = values.first() else {
        return Ok(());
    };
    let mut charged = 0;
    runs.push(0);
    for index in 1..count {
        charged += 1;
        if compare(&values[index], &values[index - 1], &mut charged).is_lt() {
            runs.push(index);
        }
    }
    runs.push(count);
    counters.charge_work(u128::from(charged), limits, location)?;
    // Within the admitted capacity: no allocation.
    scratch.resize(count, first);
    while runs.len() > 2 {
        let total = merge_pass(values, scratch, runs, pool, &compare);
        counters.charge_work(u128::from(total), limits, location)?;
        std::mem::swap(values, scratch);
        // Each merged pair starts where its left run started.
        let mut kept = 0;
        let mut run = 0;
        while run + 1 < runs.len() {
            runs[kept] = runs[run];
            kept += 1;
            run += 2;
        }
        runs[kept] = count;
        runs.truncate(kept + 1);
    }
    scratch.clear();
    runs.clear();
    Ok(())
}

/// Merge each adjacent pair of `runs` from `values` into `scratch`, in
/// groups of whole merges of at least [`GROUP_VALUES`] values on `pool`.
/// Returns the permits counted.
fn merge_pass<T: Copy + Send + Sync>(
    values: &[T],
    scratch: &mut [T],
    runs: &[usize],
    pool: Option<&ThreadPool>,
    compare: &(impl Fn(&T, &T, &mut u64) -> Ordering + Sync),
) -> u64 {
    let count = values.len();
    let merges = (runs.len() - 1).div_ceil(2);
    let groups = merges.min(GROUPS).min(count.div_ceil(GROUP_VALUES));
    let pool = pool.filter(|_| groups > 1);
    let Some(pool) = pool else {
        return merge_group(values, scratch, runs, 0..merges, compare);
    };
    // Whole merges per group, cut where a group first reaches its share of
    // values; the cuts depend only on the run boundaries.
    let share = count.div_ceil(groups);
    let total = AtomicU64::new(0);
    pool.scope(|scope| {
        let mut rest = scratch;
        let mut merge = 0;
        while merge < merges {
            let start = runs[2 * merge];
            let mut end_merge = merge + 1;
            while end_merge < merges && runs[(2 * end_merge).min(runs.len() - 1)] - start < share {
                end_merge += 1;
            }
            let end = runs[(2 * end_merge).min(runs.len() - 1)];
            let (output, tail) = std::mem::take(&mut rest).split_at_mut(end - start);
            rest = tail;
            let total = &total;
            let range = merge..end_merge;
            scope.spawn(move |_| {
                let charged = merge_group(values, output, runs, range, compare);
                total.fetch_add(charged, Atomic::Relaxed);
            });
            merge = end_merge;
        }
    });
    total.into_inner()
}

/// Merge the run pairs `merges` (pair m is runs 2m and 2m + 1) of `values`
/// into `output`, which starts where the first pair starts. Returns the
/// permits counted.
fn merge_group<T: Copy>(
    values: &[T],
    output: &mut [T],
    runs: &[usize],
    merges: std::ops::Range<usize>,
    compare: &impl Fn(&T, &T, &mut u64) -> Ordering,
) -> u64 {
    let last = runs.len() - 1;
    let base = runs[2 * merges.start];
    let mut charged = 0;
    for merge in merges {
        charged += 1;
        let start = runs[2 * merge];
        let middle = runs[2 * merge + 1];
        let end = runs[(2 * merge + 2).min(last)].max(middle);
        let range = start - base..end - base;
        charged += merge_pair(values, &mut output[range], (start, middle, end), compare);
    }
    charged
}

/// Consecutive wins by one side of a merge before it gallops.
const GALLOP: usize = 7;

/// Merge the ascending runs `values[start..middle]` and
/// `values[middle..end]` into `output`. Ties take the left value first.
/// Returns the permits counted.
fn merge_pair<T: Copy>(
    values: &[T],
    output: &mut [T],
    (start, middle, end): (usize, usize, usize),
    compare: &impl Fn(&T, &T, &mut u64) -> Ordering,
) -> u64 {
    let mut charged = 0;
    let (mut left, mut right, mut at) = (start, middle, 0);
    let put = |output: &mut [T], from: std::ops::Range<usize>, at: &mut usize| {
        output[*at..*at + from.len()].copy_from_slice(&values[from.clone()]);
        *at += from.len();
    };
    // The side that won the last comparisons (true for right) and how often.
    let (mut side, mut wins) = (false, 0);
    while left < middle && right < end {
        if wins >= GALLOP {
            if side {
                // Right values strictly below the left head.
                let pivot = values[left];
                let to = gallop(right, end, &mut charged, |index, charged| {
                    compare(&values[index], &pivot, charged).is_lt()
                });
                put(output, right..to, &mut at);
                right = to;
            } else {
                // Left values not above the right head.
                let pivot = values[right];
                let to = gallop(left, middle, &mut charged, |index, charged| {
                    !compare(&pivot, &values[index], charged).is_lt()
                });
                put(output, left..to, &mut at);
                left = to;
            }
            // A gallop that stopped inside its run did so on a probe that
            // already compared the two heads: the other side's head is next.
            if left < middle && right < end {
                if side {
                    put(output, left..left + 1, &mut at);
                    left += 1;
                } else {
                    put(output, right..right + 1, &mut at);
                    right += 1;
                }
            }
            (side, wins) = (!side, 1);
            continue;
        }
        charged += 1;
        let won = compare(&values[right], &values[left], &mut charged).is_lt();
        if won {
            put(output, right..right + 1, &mut at);
            right += 1;
        } else {
            put(output, left..left + 1, &mut at);
            left += 1;
        }
        if won == side {
            wins += 1;
        } else {
            (side, wins) = (won, 1);
        }
    }
    put(output, left..middle, &mut at);
    put(output, right..end, &mut at);
    charged
}

/// The first index in `from..to` where `holds` fails, for a predicate that
/// holds on a prefix: probe offsets 1, 2, 4, ... and then bisect the last
/// gap. One permit per probe.
fn gallop(
    from: usize,
    to: usize,
    charged: &mut u64,
    mut holds: impl FnMut(usize, &mut u64) -> bool,
) -> usize {
    // Invariant: holds on from..low; fails at high (or high == to).
    let (mut low, mut step) = (from, 1_usize);
    let mut high = loop {
        let probe = from.saturating_add(step - 1);
        if probe >= to {
            break to;
        }
        *charged += 1;
        if !holds(probe, charged) {
            break probe;
        }
        low = probe + 1;
        step = step.saturating_mul(2);
    };
    while low < high {
        let probe = low + (high - low) / 2;
        *charged += 1;
        if holds(probe, charged) {
            low = probe + 1;
        } else {
            high = probe;
        }
    }
    high
}

#[cfg(test)]
mod tests;
