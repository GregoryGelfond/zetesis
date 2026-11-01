//! Storage-transparent packed words and operations on nonnegative positions.
//!
//! [`Words`] wraps existing storage without assigning a logical universe or
//! changing its tail bits. Semantic owners retain identity, allocation and
//! admission; slice kernels preserve their stated callback boundaries.

use std::ops::RangeInclusive;

mod storage;
pub use storage::{SetBits, Words};

const WORD_BITS: usize = u64::BITS as usize;

/// Union two word-aligned sets in ascending word order.
///
/// Charges one OR before each mutation, including its loads and store. There
/// is no allocation. Refusal preserves the unvisited suffix. Padding is part
/// of the set and must already be clear when it is outside the caller's domain.
///
/// # Errors
/// Returns the first callback refusal, before that word is changed.
///
/// # Panics
/// Panics before any callback if the lengths differ.
pub fn union_assign<E>(
    left: &mut [u64],
    right: &[u64],
    before: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<(), E> {
    assert_eq!(left.len(), right.len(), "packed rows have equal lengths");
    Words::<u64, _>::new(left).union_assign(Words::new(right), || before(1))
}

/// Decide disjointness, stopping at the first shared position.
///
/// Each visited word costs one AND and one zero test. An intersection is an
/// absorbing witness: no later word or callback is visited. Empty sets are
/// disjoint without reading a word. No allocation or mutation occurs.
///
/// # Errors
/// Returns the first callback refusal; an unfinished scan proves no result.
///
/// # Panics
/// Panics before any callback if the lengths differ.
pub fn disjoint<E>(
    left: &[u64],
    right: &[u64],
    before: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<bool, E> {
    assert_eq!(left.len(), right.len(), "packed rows have equal lengths");
    for (left, right) in left.iter().zip(right) {
        before(1)?;
        let common = *left & *right;
        before(1)?;
        if common != 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Count all set positions with a popcount reduction.
///
/// Initialization costs one operation. Each word costs one popcount and one
/// addition. All stored bits participate; the caller clears universe padding.
/// Work is `1 + 2*words.len()` and auxiliary space is constant.
///
/// # Errors
/// Returns the first callback refusal without a partial result.
///
/// # Panics
/// Panics before any callback if the complete bit width exceeds `usize::MAX`.
pub fn count<E>(words: &[u64], before: &mut impl FnMut(u64) -> Result<(), E>) -> Result<usize, E> {
    assert!(
        words.len() <= usize::MAX / WORD_BITS,
        "packed bit width fits"
    );
    before(1)?;
    let mut total = 0;
    for word in words {
        before(1)?;
        let count = Words::<u64>::popcount(*word) as usize;
        before(1)?;
        total += count;
    }
    Ok(total)
}

/// Find the least set position without inspecting any later words.
///
/// Each visited word costs its zero test. A found bit adds trailing-zero count,
/// position multiplication/addition and publication of the presence tag: four
/// operations. Absence adds one final tag. Padding participates like any bit.
/// No allocation or retained state occurs.
///
/// # Errors
/// Returns the first callback refusal without a partial result.
///
/// # Panics
/// Panics before any callback if the complete bit width exceeds `usize::MAX`.
pub fn first<E>(
    words: &[u64],
    before: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<Option<usize>, E> {
    assert!(
        words.len() <= usize::MAX / WORD_BITS,
        "packed bit width fits"
    );
    for (index, word) in words.iter().enumerate() {
        before(1)?;
        if *word != 0 {
            before(4)?;
            return Ok(Some(index * WORD_BITS + Words::<u64>::first_bit(*word)));
        }
    }
    before(1)?;
    Ok(None)
}

/// Visit every set position once, ascending, borrowing a caller-owned context.
///
/// Each word copy costs one operation. Each position costs trailing-zero count,
/// position multiplication/addition, subtraction and clearing its bit: five.
/// Visitor work is separate. Sharing a context lets both callbacks use one
/// budget without interior mutability. Auxiliary space is constant.
///
/// # Errors
/// Returns the first charging or visitor failure, with no subsequent visit.
/// Already completed visitor effects form a prefix, never a complete result.
///
/// # Panics
/// Panics before any callback if the complete bit width exceeds `usize::MAX`.
pub fn visit_set_bits<C, E>(
    words: &[u64],
    context: &mut C,
    before: &mut impl FnMut(&mut C, u64) -> Result<(), E>,
    visit: &mut impl FnMut(&mut C, usize) -> Result<(), E>,
) -> Result<(), E> {
    assert!(
        words.len() <= usize::MAX / WORD_BITS,
        "packed bit width fits"
    );
    for (index, word) in words.iter().enumerate() {
        before(context, 1)?;
        let mut remaining = Words::<u64>::bits(*word);
        while let Some(bit) = remaining.next_with(|| before(context, 5))? {
            visit(context, index * WORD_BITS + bit)?;
        }
    }
    Ok(())
}

/// Replace `left` by its symmetric difference with `right`.
///
/// Both slices represent the same word-aligned universe; every stored bit,
/// including padding, participates. Words are visited in ascending order.
/// `before` runs immediately before each word XOR, exactly once per word on
/// success. Each XOR includes its operand loads and result store. Separate
/// initialization belongs to the caller's ledger; loop control is not charged.
///
/// No allocation or retained state occurs. Work is linear in the slice length
/// and auxiliary space is constant. After each completed word, that prefix is
/// the symmetric difference and the remaining suffix is unchanged.
///
/// # Errors
/// Returns the callback's first refusal without changing the refused word or
/// visiting later words. The already-modified prefix is incomplete output.
///
/// # Panics
/// Panics before any callback or mutation if the slice lengths differ.
#[inline]
pub fn xor_assign<E>(
    left: &mut [u64],
    right: &[u64],
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), E> {
    assert_eq!(left.len(), right.len(), "packed rows have equal lengths");
    Words::<u64, _>::new(left).xor_assign(Words::new(right), before)
}

/// Count the positions present in both word-aligned sets.
///
/// Every stored bit participates, so callers must clear any padding outside
/// their logical universe. Words are visited in ascending order. For each word,
/// `before` runs once immediately before the AND and again immediately before
/// the popcount: exactly two primitive-operation charges per word on success.
/// Each primitive includes its operand loads and result store. Initializing and
/// adding to the accumulator belong to the caller's ledger; loop control is not
/// charged.
///
/// No allocation or retained state occurs. Work is linear in the slice length
/// and auxiliary space is constant. The accumulator counts the intersection
/// of the completed prefix; the length bound ensures that every sum fits.
///
/// # Errors
/// Returns the callback's first refusal before that primitive operation, with
/// no later operations or callbacks. No partial count is returned. Both input
/// slices remain unchanged, including when popcount is refused after an AND.
///
/// # Panics
/// Panics before any callback if the slice lengths differ or their full bit
/// width exceeds `usize::MAX`.
#[inline]
pub fn intersection_count<E>(
    left: &[u64],
    right: &[u64],
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<usize, E> {
    assert_eq!(left.len(), right.len(), "packed rows have equal lengths");
    assert!(
        left.len() <= usize::MAX / WORD_BITS,
        "packed bit width fits"
    );
    let mut total = 0;
    for (left, right) in left.iter().zip(right) {
        before()?;
        let common = *left & *right;
        before()?;
        let count = Words::<u64>::popcount(common);
        total += usize::try_from(count).expect("one word's popcount fits usize");
    }
    Ok(total)
}

/// Add a translated copy of the initialized source prefix to `words`.
///
/// A bit at `s` adds a bit at `s + shift`, when that position fits the output
/// slice. Existing output bits remain set. Descending source visits ensure that
/// this operation uses each original bit once, including an in-place carry
/// within one word; bits produced by this call are not shifted again.
///
/// `before` runs immediately before each source-word read. A refusal leaves a
/// partial union, which the caller must discard or otherwise treat as incomplete.
/// No allocation, callback-independent input traversal or retained state occurs.
/// Words whose entire shifted output is outside the slice are not read.
///
/// # Errors
/// Returns the callback's first refusal, before the refused source read.
///
/// # Panics
/// Panics if `source_words` exceeds the initialized slice length.
pub fn union_shifted<E>(
    words: &mut [u64],
    source_words: usize,
    shift: usize,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), E> {
    union_shifted_until(words, source_words, shift, before, &mut |_, _| false).map(|_| ())
}

/// Add translated source bits until one meets the inclusive interval.
///
/// This queries only the translated copy of the initialized source prefix,
/// not bits already present in the output. `Ok(true)` means such a translated
/// bit lies in the interval and output slice. The union may then be partial;
/// the caller must discard it or otherwise treat it as incomplete. `Ok(false)`
/// means no translated bit meets the interval and the full union is complete,
/// exactly as with [`union_shifted`]. An empty interval therefore still performs
/// the full union.
///
/// Each admitted source word contributes both of its destination fragments
/// before a hit is returned. No lower source word is read after a hit. The
/// descending source order, single-use bits, lack of allocation and metered
/// source reads have the same contract as [`union_shifted`].
///
/// # Errors
/// Returns the callback's first refusal before the refused source read. A
/// refusal is not a hit or a completed union; partial output is incomplete.
///
/// # Panics
/// Panics if `source_words` exceeds the initialized slice length.
pub fn union_shifted_until_hit<E>(
    words: &mut [u64],
    source_words: usize,
    shift: usize,
    interval: RangeInclusive<usize>,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<bool, E> {
    let (start, end) = interval.into_inner();
    let first = start / WORD_BITS;
    let last = end / WORD_BITS;
    let lower = u64::MAX << (start % WORD_BITS);
    let upper = u64::MAX >> (WORD_BITS - 1 - end % WORD_BITS);
    union_shifted_until(words, source_words, shift, before, &mut |position, bits| {
        if start > end || position < first || position > last {
            return false;
        }
        let mut mask = u64::MAX;
        if position == first {
            mask &= lower;
        }
        if position == last {
            mask &= upper;
        }
        bits & mask != 0
    })
}

/// One descending traversal serves both complete union and an optional hit.
/// The probe receives already-read translated bits, never rereads the output.
fn union_shifted_until<E>(
    words: &mut [u64],
    source_words: usize,
    shift: usize,
    before: &mut impl FnMut() -> Result<(), E>,
    hit: &mut impl FnMut(usize, u64) -> bool,
) -> Result<bool, E> {
    assert!(source_words <= words.len(), "source prefix is initialized");
    let displacement = shift / WORD_BITS;
    let offset = shift % WORD_BITS;
    let readable = source_words.min(words.len().saturating_sub(displacement));
    for source in (0..readable).rev() {
        before()?;
        let bits = words[source];
        // `readable` bounds this addition and its destination.
        let target = source + displacement;
        let low = bits << offset;
        words[target] |= low;
        let mut found = hit(target, low);
        if offset != 0 && target < words.len() - 1 {
            let high = bits >> (WORD_BITS - offset);
            words[target + 1] |= high;
            found |= hit(target + 1, high);
        }
        if found {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests;
