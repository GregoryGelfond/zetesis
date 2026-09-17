//! The fixed word hash the crate's indexes share.
//!
//! An index keyed by a structure the crate builds itself, such as a formula
//! node or a spelled atom, is placed by this hash rather than the standard
//! library's randomized one: the keys are not adversarial, the placement is
//! reproducible, and the mixing is a few instructions per word.

use std::hash::Hasher;

/// Mixes the words of a key by multiplication with an odd constant
/// after rotating the running value, the scheme of the Rust compiler's own
/// interner hash; the low bits of each word reach every bit of the result.
#[derive(Default)]
pub(crate) struct WordHasher(u64);

impl Hasher for WordHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(u64::from(byte));
        }
    }
    fn write_u8(&mut self, word: u8) {
        self.write_u64(u64::from(word));
    }
    fn write_usize(&mut self, word: usize) {
        self.write_u64(word as u64);
    }
    fn write_u64(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}
