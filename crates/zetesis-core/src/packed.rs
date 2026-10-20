//! Allocation-free operations on caller-owned packed sets of nonnegative positions.

use std::ops::RangeInclusive;

const WORD_BITS: usize = u64::BITS as usize;

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
