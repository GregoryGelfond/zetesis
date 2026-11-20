//! A sparse canonical-to-local equality translation beside semantic ordering.
//!
//! Published entries are exactly the append dictionary's representatives. The
//! bound source supplies their vocabulary; no map entry retains a scope or
//! payload. Hash operations admit one fixed-key container operation, not each
//! opaque internal probe. A value the read cannot name is found by its
//! semantic content hash; no value order is kept.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, DefaultHasher, Hash, Hasher};

use crate::catalog::{CatalogRead, ReadError, TermRef, storage::TermId};
use crate::word_mix::{Prehashed, WordHasher};

use super::{Failure, Resource, Work, ceiling};

#[derive(Default)]
pub(super) struct Append {
    pub identities: Inverse,
    pub content: Content,
    /// Equality IDs are the owner's term identities, so no representative,
    /// content chain or renumbering is kept: the read resolves each ID.
    pub terms: bool,
}

/// Content identity for values this owner's read cannot name.
///
/// Every dictionary value is chained under the semantic hash of its term
/// (`Hash for TermRef`, consistent with term equality). A lookup of an
/// uninterned or foreign value hashes it once and compares only the values in
/// its chain, so no value order is kept: insertion records a value in
/// constant work beyond its hash, and a lookup's comparisons do not grow with
/// the dictionary. Hashing reads every payload word once and is charged per
/// expanded node and per eight canonical bytes. Retained bytes are the head
/// table plus one `u32` link per value.
#[derive(Default)]
pub(super) struct Content {
    heads: HashMap<u64, u32, BuildHasherDefault<Prehashed>>,
    next: Vec<u32>,
}

/// The chain end.
const NO_LINK: u32 = u32::MAX;

impl Content {
    /// The semantic hash of a value: fixed keys, so the same in every run.
    pub(super) fn hash(value: TermRef<'_>) -> u64 {
        let mut state = DefaultHasher::new();
        value.hash(&mut state);
        state.finish()
    }

    /// Charged work of hashing a value of `nodes` expanded nodes and
    /// `bytes` canonical bytes: one unit per node and per payload word.
    pub(super) fn hash_work(nodes: usize, bytes: usize) -> u128 {
        nodes as u128 + bytes.div_ceil(8) as u128
    }

    /// Addressable key/value capacity and links, not opaque control bytes.
    pub(super) fn bytes(&self) -> usize {
        self.heads.capacity() * size_of::<(u64, u32)>() + self.next.capacity() * size_of::<u32>()
    }

    /// Reset visits retained hash storage even when its logical extent is small.
    pub(super) fn clear_work(&self) -> u128 {
        self.heads.capacity() as u128
    }

    pub(super) fn clear(&mut self) {
        self.heads.clear();
        self.next.clear();
    }

    pub(super) fn reserve(&mut self, additional: usize, work: &mut Work) -> Result<(), Failure> {
        let needed = self
            .next
            .len()
            .checked_add(additional)
            .ok_or(Failure::Overflow)?;
        if needed <= self.next.capacity() && needed <= self.heads.capacity() {
            return Ok(());
        }
        let previous = self.bytes();
        let proposed = needed.max(self.next.capacity().saturating_mul(2));
        ceiling(
            Resource::Bytes,
            work.live as u128
                + proposed as u128 * (size_of::<(u64, u32)>() + size_of::<u32>()) as u128,
            work.limits.max_bytes as u128,
        )?;
        // Relocation and the container reservation precede any publication.
        work.tick(self.next.len() as u128 + 1)?;
        self.heads
            .try_reserve(proposed.saturating_sub(self.heads.len()))
            .map_err(|_| Failure::Allocation)?;
        self.next
            .try_reserve(proposed.saturating_sub(self.next.len()))
            .map_err(|_| Failure::Allocation)?;
        work.replacement(previous, self.bytes())
    }

    /// Chain value `equality`, the next identity, under `hash`. Reserved by
    /// the surrounding transaction.
    pub(super) fn publish(&mut self, hash: u64, equality: u32) {
        debug_assert_eq!(self.next.len(), equality as usize);
        let link = self.heads.insert(hash, equality).unwrap_or(NO_LINK);
        self.next.push(link);
    }

    /// The value equal to the query among those chained under `hash`;
    /// `equal(id)` decides each candidate, newest first.
    pub(super) fn find<E>(
        &self,
        hash: u64,
        mut equal: impl FnMut(u32) -> Result<bool, E>,
    ) -> Result<Option<u32>, E> {
        let mut link = self.heads.get(&hash).copied().unwrap_or(NO_LINK);
        while link != NO_LINK {
            if equal(link)? {
                return Ok(Some(link));
            }
            link = self.next[link as usize];
        }
        Ok(None)
    }
}

#[derive(Default)]
pub(super) struct Inverse {
    entries: HashMap<TermId, u32, BuildHasherDefault<WordHasher>>,
}

pub(super) enum Probe {
    /// Semantic lookup remains available when this read cannot name the term.
    Unavailable(ReadError),
    Local {
        term: TermId,
        equality: Option<u32>,
    },
}

impl Inverse {
    pub(super) fn probe_with<E>(
        &self,
        read: CatalogRead<'_>,
        value: TermRef<'_>,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Probe, E> {
        before()?;
        let term = match read.selected_term(value) {
            Ok(term) => term,
            Err(error) => return Ok(Probe::Unavailable(error)),
        };
        before()?;
        Ok(Probe::Local {
            term,
            equality: self.entries.get(&term).copied(),
        })
    }

    /// Addressable key/value capacity, not opaque bucket/control allocation.
    pub(super) fn bytes(&self) -> usize {
        self.entries.capacity() * size_of::<(TermId, u32)>()
    }

    pub(super) fn reserve(&mut self, additional: usize, work: &mut Work) -> Result<(), Failure> {
        let needed = self
            .entries
            .len()
            .checked_add(additional)
            .ok_or(Failure::Overflow)?;
        if needed <= self.entries.capacity() {
            return Ok(());
        }
        let previous = self.bytes();
        let proposed = needed.max(self.entries.capacity().saturating_mul(2));
        ceiling(
            Resource::Bytes,
            work.live as u128 + proposed as u128 * size_of::<(TermId, u32)>() as u128,
            work.limits.max_bytes as u128,
        )?;
        // Relocation and the container reservation precede any publication.
        work.tick(self.entries.len() as u128 + 1)?;
        self.entries
            .try_reserve(proposed - self.entries.len())
            .map_err(|_| Failure::Allocation)?;
        work.replacement(previous, self.bytes())
    }

    /// All entries and writes were reserved/admitted by the surrounding tuple
    /// transaction. Standard fixed-ID hash/equality has no caller code or error.
    pub(super) fn publish(&mut self, term: TermId, equality: u32) {
        let previous = self.entries.insert(term, equality);
        debug_assert!(
            previous.is_none(),
            "each dictionary identity publishes once"
        );
    }

    /// Reset visits retained hash storage even when its logical extent is small.
    pub(super) fn clear_work(&self) -> u128 {
        self.entries.capacity() as u128
    }

    /// The complete reset is admitted before any owner extent changes.
    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod content_tests {
    use super::Content;

    #[test]
    fn a_shared_hash_chains_its_values_and_finds_each() {
        let mut content = Content::default();
        for (equality, hash) in [(0, 7), (1, 9), (2, 7), (3, 7)] {
            content.next.reserve(1);
            content.publish(hash, equality);
        }
        let values = [10, 20, 30, 40];
        let find = |hash: u64, query: i32| {
            let mut compared = 0;
            let found = content
                .find(hash, |id| {
                    compared += 1;
                    Ok::<_, ()>(values[id as usize] == query)
                })
                .unwrap();
            (found, compared)
        };
        assert_eq!(find(7, 10), (Some(0), 3));
        assert_eq!(find(7, 40), (Some(3), 1));
        assert_eq!(find(9, 20), (Some(1), 1));
        // A miss compares only its own chain, never the whole dictionary.
        assert_eq!(find(7, 99), (None, 3));
        assert_eq!(find(5, 10), (None, 0));
    }

    #[test]
    fn hash_work_counts_nodes_and_payload_words() {
        assert_eq!(Content::hash_work(1, 0), 1);
        assert_eq!(Content::hash_work(1, 8), 2);
        assert_eq!(Content::hash_work(3, 17), 6);
    }
}
