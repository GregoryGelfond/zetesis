//! Values keyed by the owner-scoped identity of canonical atoms.

use std::{
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
    mem::size_of,
};

use super::{AtomRef, Error, storage};

/// Values keyed by the owner-scoped identity of canonical atoms, for any
/// number of atom owners. A lookup hashes one identity word instead of the
/// atom's structure. Owned ingress and carrier atoms have no identity here,
/// and equal atoms of different owners are different keys. Each owner is
/// retained by identity only, never by its payload. Owners are found by a
/// linear scan, which suits the few owners of one run, such as one per
/// search worker.
#[derive(Debug)]
pub struct AtomIdentityMap<T> {
    owners: Vec<Scoped<T>>,
}

/// One owner's entries.
#[derive(Debug)]
struct Scoped<T> {
    scope: storage::AtomScope,
    values: HashMap<storage::AtomId, T, BuildHasherDefault<IdentityHasher>>,
}

/// Identities are assigned by their owner, not by input, so one
/// multiplicative mix of the identity word places them. The mix is the one
/// zetesis-themelios's word hash uses; keep the two in step.
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
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

impl<T> Default for AtomIdentityMap<T> {
    fn default() -> Self {
        Self { owners: Vec::new() }
    }
}

impl<T: Copy> AtomIdentityMap<T> {
    /// The value recorded for a canonical atom of a held owner.
    #[must_use]
    pub fn get(&self, atom: AtomRef<'_>) -> Option<T> {
        let (read, id) = atom.canonical()?;
        self.owners
            .iter()
            .find(|owner| read.accepts_atom_scope(&owner.scope))?
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
        let position = if let Some(position) = self
            .owners
            .iter()
            .position(|owner| read.accepts_atom_scope(&owner.scope))
        {
            position
        } else {
            self.owners.try_reserve(1).map_err(|_| Error::Allocation)?;
            self.owners.push(Scoped {
                scope,
                values: HashMap::default(),
            });
            self.owners.len() - 1
        };
        let values = &mut self.owners[position].values;
        values.try_reserve(1).map_err(|_| Error::Allocation)?;
        values.insert(id, value);
        Ok(true)
    }

    /// Keep only the entries whose value satisfies `keep`.
    pub fn retain(&mut self, mut keep: impl FnMut(T) -> bool) {
        for owner in &mut self.owners {
            owner.values.retain(|_, value| keep(*value));
        }
    }

    /// Recorded entries across all owners.
    #[must_use]
    pub fn len(&self) -> usize {
        self.owners.iter().map(|owner| owner.values.len()).sum()
    }

    /// Whether no entry is recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.owners.iter().all(|owner| owner.values.is_empty())
    }

    /// Owner headers and entry capacity; hash-table control bytes and the
    /// owners' own storage are excluded.
    #[must_use]
    pub fn retained_bytes(&self) -> usize {
        size_of::<Self>()
            + self.owners.capacity() * size_of::<Scoped<T>>()
            + self
                .owners
                .iter()
                .map(|owner| owner.values.capacity() * size_of::<(storage::AtomId, T)>())
                .sum::<usize>()
    }
}

#[cfg(test)]
mod tests;
