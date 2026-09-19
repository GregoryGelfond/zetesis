//! The fixed word hash the crate's indexes share.
//!
//! An index keyed by identities the crate assigns itself, the formula node
//! index, is placed by this hash rather than the standard library's
//! randomized one: such a key carries nothing a program's author chooses,
//! so no input drives it into collisions, the placement is reproducible,
//! and the mixing is a few instructions per word. An index keyed by what
//! an author spells, such as the JSON document's table of spelled atoms,
//! keeps the randomized hasher.

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
