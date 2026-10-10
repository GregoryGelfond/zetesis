//! One owned allocation for the six disjoint knowledge masks.
//!
//! The node pair precedes the atom pair, seen snapshot and chain witnesses. Borrowing splits
//! the block once; narrowing then indexes plain slices, with no per-bit shape
//! decoding. Allocation retains Knowledge's infallible construction boundary.

use super::flag_words;

#[derive(Debug)]
pub(super) struct Masks {
    pub(super) words: Box<[u64]>,
    node_words: usize,
    atom_words: usize,
}

impl Masks {
    pub(super) fn empty(nodes: usize, atoms: usize, chains: usize) -> Self {
        let node_words = flag_words(nodes);
        let atom_words = flag_words(atoms);
        let words = node_words
            .checked_mul(2)
            .and_then(|nodes| atom_words.checked_mul(3)?.checked_add(nodes))
            .and_then(|words| words.checked_add(flag_words(chains)))
            .expect("the six knowledge masks fit one word block");
        Self {
            words: vec![0; words].into_boxed_slice(),
            node_words,
            atom_words,
        }
    }

    /// The six exact, nonoverlapping spans, including unused tail bits.
    /// Safe splits check every boundary before the closure can access them.
    pub(super) fn split(&mut self) -> [&mut [u64]; 6] {
        let (sure, rest) = self.words.split_at_mut(self.node_words);
        let (never, rest) = rest.split_at_mut(self.node_words);
        let (atom_sure, rest) = rest.split_at_mut(self.atom_words);
        let (atom_never, rest) = rest.split_at_mut(self.atom_words);
        let (seen, witnessed) = rest.split_at_mut(self.atom_words);
        [sure, never, atom_sure, atom_never, seen, witnessed]
    }

    #[cfg(test)]
    pub(super) fn slices(&self) -> [&[u64]; 6] {
        let (sure, rest) = self.words.split_at(self.node_words);
        let (never, rest) = rest.split_at(self.node_words);
        let (atom_sure, rest) = rest.split_at(self.atom_words);
        let (atom_never, rest) = rest.split_at(self.atom_words);
        let (seen, witnessed) = rest.split_at(self.atom_words);
        [sure, never, atom_sure, atom_never, seen, witnessed]
    }
}

impl Clone for Masks {
    fn clone(&self) -> Self {
        Self {
            words: self.words.clone(),
            node_words: self.node_words,
            atom_words: self.atom_words,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.words.clone_from(&source.words);
        self.node_words = source.node_words;
        self.atom_words = source.atom_words;
    }
}

#[cfg(test)]
mod tests;
