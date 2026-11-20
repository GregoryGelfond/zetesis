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

/// Places keys by folding each word written to it through [`mix_word`]. The
/// crate's tables keyed by its own identities use it instead of the standard
/// library's randomized hash: the placement is reproducible and the mixing
/// a few instructions per word.
#[derive(Default)]
pub(crate) struct WordHasher(u64);

impl std::hash::Hasher for WordHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(u64::from(byte));
        }
    }
    fn write_u32(&mut self, word: u32) {
        self.write_u64(u64::from(word));
    }
    fn write_usize(&mut self, word: usize) {
        self.write_u64(word as u64);
    }
    fn write_u64(&mut self, word: u64) {
        self.0 = mix_word(self.0, word);
    }
}

/// Places keys that are already semantic hashes by those hashes as they are.
#[derive(Default)]
pub(crate) struct Prehashed(u64);

impl std::hash::Hasher for Prehashed {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = self.0.rotate_left(8) ^ u64::from(byte);
        }
    }
    fn write_u64(&mut self, value: u64) {
        self.0 = value;
    }
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
