//! The freeze sort: fixed blocks, sorted independently, then merged pairwise.
//!
//! Block boundaries and the merge tree depend only on the input length, so
//! the comparisons made, and their count, are the same whether the blocks run
//! on one thread or many. Each block and each pairwise merge of a pass is
//! independent: the same shape maps onto SIMD lanes or a GPU's merge-path.

use std::cmp::Ordering;
use std::sync::atomic::{AtomicU64, Ordering as Atomic};

use zetesis_algebra::threads::Pool;

/// Entries per independently sorted block.
pub const BLOCK: usize = 1024;

/// Sort `items` by `compare` and return the comparisons made. A strict total
/// order on distinct items gives one result regardless of `pool`.
pub fn sort_by_content<T, F>(items: &mut [T], pool: Option<&Pool>, compare: F) -> u64
where
    T: Copy + Send + Sync,
    F: Fn(&T, &T) -> Ordering + Sync,
{
    let compares = AtomicU64::new(0);
    let sort_block = |block: &mut [T]| {
        let mut count = 0;
        block_sort(block, &mut count, &compare);
        compares.fetch_add(count, Atomic::Relaxed);
    };
    match pool {
        Some(pool) if items.len() > BLOCK => pool.scope(|scope| {
            let sort_block = &sort_block;
            for block in items.chunks_mut(BLOCK) {
                scope.spawn(move |_| sort_block(block));
            }
        }),
        _ => items.chunks_mut(BLOCK).for_each(sort_block),
    }
    let mut width = BLOCK;
    let mut scratch: Vec<T> = items.to_vec();
    while width < items.len() {
        let merge_pair = |(input, output): (&[T], &mut [T])| {
            let mut count = 0;
            let (left, right) = input.split_at(width.min(input.len()));
            merge_into(left, right, output, &mut count, &compare);
            compares.fetch_add(count, Atomic::Relaxed);
        };
        let pairs = items.chunks(2 * width).zip(scratch.chunks_mut(2 * width));
        match pool {
            Some(pool) => pool.scope(|scope| {
                let merge_pair = &merge_pair;
                for pair in pairs {
                    scope.spawn(move |_| merge_pair(pair));
                }
            }),
            None => pairs.for_each(merge_pair),
        }
        items.copy_from_slice(&scratch);
        width *= 2;
    }
    compares.into_inner()
}

/// Runs sorted by binary insertion before the block's merge passes.
const RUN: usize = 16;

/// Sort one block: binary insertion into runs of [`RUN`], then pairwise merges
/// through a block-sized scratch. O(b log b) comparisons and moves.
fn block_sort<T: Copy>(block: &mut [T], count: &mut u64, compare: &impl Fn(&T, &T) -> Ordering) {
    for run in block.chunks_mut(RUN) {
        for end in 1..run.len() {
            let item = run[end];
            let (mut low, mut high) = (0, end);
            while low < high {
                let middle = low + (high - low) / 2;
                *count += 1;
                if compare(&run[middle], &item) == Ordering::Greater {
                    high = middle;
                } else {
                    low = middle + 1;
                }
            }
            run.copy_within(low..end, low + 1);
            run[low] = item;
        }
    }
    let mut scratch = block.to_vec();
    let mut width = RUN;
    while width < block.len() {
        for (input, output) in block.chunks(2 * width).zip(scratch.chunks_mut(2 * width)) {
            let (left, right) = input.split_at(width.min(input.len()));
            merge_into(left, right, output, count, compare);
        }
        block.copy_from_slice(&scratch);
        width *= 2;
    }
}

fn merge_into<T: Copy>(
    left: &[T],
    right: &[T],
    output: &mut [T],
    count: &mut u64,
    compare: &impl Fn(&T, &T) -> Ordering,
) {
    let (mut i, mut j) = (0, 0);
    for slot in output.iter_mut() {
        let take_left = if i == left.len() {
            false
        } else if j == right.len() {
            true
        } else {
            *count += 1;
            compare(&left[i], &right[j]) != Ordering::Greater
        };
        if take_left {
            *slot = left[i];
            i += 1;
        } else {
            *slot = right[j];
            j += 1;
        }
    }
}

/// Merge two sorted runs into a new one, counting comparisons.
pub(super) fn merge<T: Copy>(
    left: &[T],
    right: &[T],
    count: &mut u64,
    compare: impl Fn(&T, &T) -> Ordering,
) -> Vec<T> {
    let mut output = left.to_vec();
    output.extend_from_slice(right);
    merge_into(left, right, &mut output, count, &compare);
    output
}
