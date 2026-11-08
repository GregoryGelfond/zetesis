//! Work permits precede logical reads, hash input, equality probes and reserves.
//! A bulk reserve/copy receives all permits before it starts, so a caller stop
//! cannot leave a partly published row. Standard container internals are one
//! bounded library operation, not a claim about allocator or CPU instructions.

use std::hash::{Hash, Hasher};

use super::{
    Failure, Fault, TermId,
    budget::{self, Budget},
};

/// The hash of a key's words, deterministic and cheap: each written word is
/// folded into the state by [`mix_word`](crate::mix_word), the mix the
/// solver's fixed hashers share; bytes are folded eight at a time, the tail
/// tagged with its length, and the finish mixes the state's halves. The index that
/// reads it probes by equality, so a hash only has to spread keys, never to
/// resist an adversary, and the ids the catalog assigns do not depend on it.
/// Keys crafted to collide degrade the index to linear probes; a program from
/// an untrusted source is bounded by its caller's deadline and memory
/// allowance, which the grounding observes between catalog operations.
#[derive(Default)]
struct KeyHasher(u64);

impl KeyHasher {
    fn fold(&mut self, word: u64) {
        self.0 = crate::mix_word(self.0, word);
    }
}

impl Hasher for KeyHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            self.fold(u64::from_le_bytes(chunk.try_into().expect("eight bytes")));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut tail = [0_u8; 8];
            tail[..rest.len()].copy_from_slice(rest);
            self.fold(u64::from_le_bytes(tail) ^ (rest.len() as u64) << 56);
        }
    }

    fn write_u8(&mut self, byte: u8) {
        self.fold(u64::from(byte));
    }

    fn write_u32(&mut self, word: u32) {
        self.fold(u64::from(word));
    }

    fn write_u64(&mut self, word: u64) {
        self.fold(word);
    }

    fn write_usize(&mut self, word: usize) {
        self.fold(word as u64);
    }

    fn finish(&self) -> u64 {
        let mut mixed = self.0;
        mixed ^= mixed >> 32;
        mixed = mixed.wrapping_mul(0xD6E8_FEB8_6659_FD93);
        mixed ^= mixed >> 32;
        mixed
    }
}

pub(super) struct Work<'a, E> {
    before: &'a mut dyn FnMut() -> Result<(), E>,
}

impl<'a, E> Work<'a, E> {
    pub(super) fn new(before: &'a mut dyn FnMut() -> Result<(), E>) -> Self {
        Self { before }
    }

    pub(super) fn step(&mut self) -> Result<(), Failure<E>> {
        (self.before)().map_err(Failure::Stopped)
    }

    pub(super) fn steps(&mut self, count: usize) -> Result<(), Failure<E>> {
        for _ in 0..count {
            self.step()?;
        }
        Ok(())
    }

    pub(super) fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        budget: &mut Budget,
    ) -> Result<(), Failure<E>> {
        self.step()?;
        let needed = values
            .len()
            .checked_add(additional)
            .ok_or(Fault::Overflow)?;
        if needed > values.capacity() {
            self.steps(values.len())?;
        }
        budget::reserve(values, additional, budget).map_err(Failure::Storage)
    }

    pub(super) fn reserve_text(
        &mut self,
        text: &mut String,
        additional: usize,
        budget: &mut Budget,
    ) -> Result<(), Failure<E>> {
        self.step()?;
        let needed = text.len().checked_add(additional).ok_or(Fault::Overflow)?;
        if needed > text.capacity() {
            self.steps(text.len())?;
        }
        budget::reserve_text(text, additional, budget).map_err(Failure::Storage)
    }

    pub(super) fn text_hash(&mut self, text: &str) -> Result<u64, Failure<E>> {
        let mut state = KeyHasher::default();
        self.step()?;
        text.len().hash(&mut state);
        for index in 0..text.len() {
            self.step()?;
            state.write_u8(text.as_bytes()[index]);
        }
        self.step()?;
        Ok(state.finish())
    }

    pub(super) fn key_hash(
        &mut self,
        key: impl Hash,
        children: &[TermId],
    ) -> Result<u64, Failure<E>> {
        self.key_hash_by(key, children.len(), |index| children[index])
    }

    /// Hash borrowed canonical coordinates without allocating an argument copy.
    /// The permit precedes each argument read as well as its hash operation.
    pub(super) fn key_hash_by(
        &mut self,
        key: impl Hash,
        count: usize,
        mut argument: impl FnMut(usize) -> TermId,
    ) -> Result<u64, Failure<E>> {
        let mut state = KeyHasher::default();
        self.step()?;
        key.hash(&mut state);
        self.step()?;
        count.hash(&mut state);
        for index in 0..count {
            self.step()?;
            argument(index).hash(&mut state);
        }
        self.step()?;
        Ok(state.finish())
    }

    pub(super) fn text_equal(&mut self, left: &str, right: &str) -> Result<bool, Failure<E>> {
        self.step()?;
        if left.len() != right.len() {
            return Ok(false);
        }
        for index in 0..left.len() {
            self.step()?;
            if left.as_bytes()[index] != right.as_bytes()[index] {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn quoted_bytes(&mut self, text: &str) -> Result<usize, Failure<E>> {
        self.step()?;
        let mut bytes = text.len().checked_add(2).ok_or(Fault::Overflow)?;
        for index in 0..text.len() {
            self.step()?;
            if matches!(text.as_bytes()[index], b'"' | b'\\' | b'\n') {
                bytes = bytes.checked_add(1).ok_or(Fault::Overflow)?;
            }
        }
        Ok(bytes)
    }
}

pub(super) fn uncontrolled<T>(
    result: Result<T, Failure<std::convert::Infallible>>,
) -> Result<T, Fault> {
    match result {
        Ok(value) => Ok(value),
        Err(Failure::Storage(error)) => Err(error),
        Err(Failure::Stopped(never)) => match never {},
    }
}

#[cfg(test)]
mod hasher_tests {
    use std::hash::Hasher;

    use super::KeyHasher;

    fn hash(words: &[u64]) -> u64 {
        let mut state = KeyHasher::default();
        for &word in words {
            state.write_u64(word);
        }
        state.finish()
    }

    #[test]
    fn the_key_hasher_is_deterministic_and_spreads_small_keys() {
        // The same words hash the same; the small keys the catalog meets
        // every day, ids and arities, land apart, and a byte string hashes
        // as its words with its tail length.
        assert_eq!(hash(&[1, 2, 3]), hash(&[1, 2, 3]));
        let small: std::collections::BTreeSet<u64> =
            (0..4096_u64).map(|id| hash(&[id, 2])).collect();
        assert_eq!(small.len(), 4096);
        assert_ne!(hash(&[1, 2]), hash(&[2, 1]));
        let mut text = KeyHasher::default();
        text.write(b"predicate");
        let mut again = KeyHasher::default();
        again.write(b"predicate");
        assert_eq!(text.finish(), again.finish());
        let mut other = KeyHasher::default();
        other.write(b"predicat");
        assert_ne!(text.finish(), other.finish());
    }
}
