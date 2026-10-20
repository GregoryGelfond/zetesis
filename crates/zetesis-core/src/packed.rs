//! Allocation-free operations on caller-owned packed sets of nonnegative positions.

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
    assert!(source_words <= words.len(), "source prefix is initialized");
    let displacement = shift / WORD_BITS;
    let offset = shift % WORD_BITS;
    let readable = source_words.min(words.len().saturating_sub(displacement));
    for source in (0..readable).rev() {
        before()?;
        let bits = words[source];
        // `readable` bounds this addition and its destination.
        let target = source + displacement;
        words[target] |= bits << offset;
        if offset != 0 && target < words.len() - 1 {
            words[target + 1] |= bits >> (WORD_BITS - offset);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
