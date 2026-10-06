//! Values keyed by the owner-scoped identity of canonical atoms.

use std::{
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
    mem::size_of,
};

use super::{AtomRef, Error, storage};

/// Values keyed by the owner-scoped identity of canonical atoms, for any
/// number of atom owners. A lookup hashes the owner's identity and then one
/// identity word, instead of the atom's structure, so its cost does not grow
/// with the number of owners. Owned ingress and carrier atoms have no identity
/// here, and equal atoms of different owners are different keys. Each owner is
/// retained by identity only, never by its payload; `retain_held` drops the
/// owners nothing else holds.
#[derive(Debug)]
pub struct AtomIdentityMap<T> {
    /// By the owner's address, which the entry's own scope handle keeps
    /// unique and stable while the entry exists.
    owners: HashMap<usize, Scoped<T>, BuildHasherDefault<IdentityHasher>>,
}

/// One owner's entries.
#[derive(Debug)]
struct Scoped<T> {
    scope: storage::AtomScope,
    values: HashMap<storage::AtomId, T, BuildHasherDefault<IdentityHasher>>,
}

/// Identities are assigned by their owner, not by input, so one
/// [`mix_word`](crate::mix_word) of the identity word places them.
#[derive(Default)]
struct IdentityHasher(u64);

impl Hasher for IdentityHasher {
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
    fn write_u64(&mut self, word: u64) {
        self.0 = crate::mix_word(self.0, word);
    }
}

impl<T> Default for AtomIdentityMap<T> {
    fn default() -> Self {
        Self {
            owners: HashMap::default(),
        }
    }
}

impl<T: Copy> AtomIdentityMap<T> {
    /// The value recorded for a canonical atom of a held owner.
    #[must_use]
    pub fn get(&self, atom: AtomRef<'_>) -> Option<T> {
        let (read, id) = atom.canonical()?;
        self.owners
            .get(&read.atom_owner_key()?)?
            .values
            .get(&id)
            .copied()
    }

    /// Record `value` for a canonical atom, replacing an earlier value. An
    /// atom without an owner-scoped identity is not recorded (`Ok(false)`).
    ///
    /// # Errors
    /// Returns [`Error::Allocation`] when the entry cannot be reserved; no
    /// entry is recorded.
    pub fn insert(&mut self, atom: AtomRef<'_>, value: T) -> Result<bool, Error> {
        let Some((read, id)) = atom.canonical() else {
            return Ok(false);
        };
        let Some(scope) = read.atom_scope() else {
            return Ok(false);
        };
        let key = scope.key();
        if !self.owners.contains_key(&key) {
            self.owners.try_reserve(1).map_err(|_| Error::Allocation)?;
        }
        let values = &mut self
            .owners
            .entry(key)
            .or_insert_with(|| Scoped {
                scope,
                values: HashMap::default(),
            })
            .values;
        values.try_reserve(1).map_err(|_| Error::Allocation)?;
        values.insert(id, value);
        Ok(true)
    }

    /// Drop every owner that only this map still holds: no catalog, writer or
    /// store refers to it, so none of its atoms can be presented again. One
    /// scan of the owners; entries of held owners are untouched.
    pub fn retain_held(&mut self) {
        self.owners.retain(|_, owner| owner.scope.held_elsewhere());
        // A prune that leaves the owner table far below its capacity returns
        // the excess, so retained space follows the owners still held.
        if self.owners.capacity() > 4 * self.owners.len().max(4) {
            self.owners.shrink_to(2 * self.owners.len());
        }
    }

    /// Atom owners with entries or a held scope.
    #[must_use]
    pub fn owners(&self) -> usize {
        self.owners.len()
    }

    /// Keep only the entries whose value satisfies `keep`.
    pub fn retain(&mut self, mut keep: impl FnMut(T) -> bool) {
        for owner in self.owners.values_mut() {
            owner.values.retain(|_, value| keep(*value));
        }
    }

    /// Recorded entries across all owners.
    #[must_use]
    pub fn len(&self) -> usize {
        self.owners.values().map(|owner| owner.values.len()).sum()
    }

    /// Whether no entry is recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.owners.values().all(|owner| owner.values.is_empty())
    }

    /// Owner headers and entry capacity; hash-table control bytes and the
    /// owners' own storage are excluded.
    #[must_use]
    pub fn retained_bytes(&self) -> usize {
        size_of::<Self>()
            + self.owners.capacity() * size_of::<(usize, Scoped<T>)>()
            + self
                .owners
                .values()
                .map(|owner| owner.values.capacity() * size_of::<(storage::AtomId, T)>())
                .sum::<usize>()
    }
}

#[cfg(test)]
mod tests;
