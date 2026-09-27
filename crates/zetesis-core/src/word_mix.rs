//! The word mix the solver's fixed hashers share.

/// Fold `word` into a running hash `state`: rotate, exclusive-or, then
/// multiply by an odd constant, the scheme of the Rust compiler's own interner
/// hash. The low bits of each word reach every bit of the result. Hashers that
/// place keys reproducibly, rather than by the standard library's randomized
/// hash, fold each word of a key through this one step.
#[must_use]
pub const fn mix_word(state: u64, word: u64) -> u64 {
    (state.rotate_left(5) ^ word).wrapping_mul(0x517c_c1b7_2722_0a95)
}

#[cfg(test)]
mod tests {
    use super::mix_word;

    #[test]
    fn a_word_is_rotated_in_then_multiplied_by_the_odd_constant() {
        // Changing the mix moves every fixed-hash placement at once.
        assert_eq!(mix_word(0, 1), 0x517c_c1b7_2722_0a95);
        assert_eq!(mix_word(1, 0), 32_u64.wrapping_mul(0x517c_c1b7_2722_0a95));
    }
}
