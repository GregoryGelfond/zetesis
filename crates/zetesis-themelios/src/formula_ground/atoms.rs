//! One owned formula-atom sequence with a lookup table containing only IDs.
//!
//! Full typed equality decides identity even when every hash collides. The table
//! is never enumerated to emit atoms: first insertion fixes each dense ID, and
//! consuming the catalog transfers that sequence without cloning its contents.
//! Lookup is expected constant table work plus atom hashing/equality; a collision
//! chain can inspect every atom. Growth can rehash existing atoms. Randomized
//! hashing changes neither IDs nor emission order.
//!
//! Atom count bounds the index population. Vector/table reservations are fallible
//! before membership changes. Table buckets are additional authored index storage;
//! the caller's conservative scalar-byte allowance is not total live memory.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;

use hashbrown::HashTable;
use zetesis_core::Atom;

use crate::AtomAllocation;

#[derive(Default)]
pub(super) struct Catalog<S = RandomState> {
    atoms: Vec<Atom>,
    index: HashTable<usize>,
    hasher: S,
}

pub(super) enum Entry<'a, S> {
    Occupied(usize),
    Vacant(Vacant<'a, S>),
}

pub(super) struct Vacant<'a, S> {
    catalog: &'a mut Catalog<S>,
    atom: Atom,
    hash: u64,
}

impl<S: BuildHasher> Catalog<S> {
    pub(super) fn len(&self) -> usize {
        self.atoms.len()
    }

    pub(super) fn atoms(&self) -> &[Atom] {
        &self.atoms
    }

    pub(super) fn find(&self, atom: &Atom) -> Option<usize> {
        self.index
            .find(self.hasher.hash_one(atom), |&id| self.atoms[id] == *atom)
            .copied()
    }

    pub(super) fn entry(&mut self, atom: Atom) -> Entry<'_, S> {
        let hash = self.hasher.hash_one(&atom);
        match self.index.find(hash, |&id| self.atoms[id] == atom) {
            Some(&id) => Entry::Occupied(id),
            None => Entry::Vacant(Vacant {
                catalog: self,
                atom,
                hash,
            }),
        }
    }

    fn reserve(&mut self, additional: usize) -> Result<(), AtomAllocation> {
        self.atoms
            .try_reserve(additional)
            .map_err(AtomAllocation::Atoms)?;
        self.index
            .try_reserve(additional, |&id| self.hasher.hash_one(&self.atoms[id]))
            .map_err(AtomAllocation::Index)
    }

    pub(super) fn into_atoms(self) -> Vec<Atom> {
        self.atoms
    }
}

impl<S: BuildHasher> Vacant<'_, S> {
    /// Publish the new ID only after both reservations and updates complete.
    /// A refused reservation may retain capacity, but changes no atom membership.
    pub(super) fn insert(self) -> Result<usize, AtomAllocation> {
        self.catalog.reserve(1)?;
        let id = self.catalog.atoms.len();
        self.catalog.atoms.push(self.atom);
        self.catalog
            .index
            .insert_unique(self.hash, id, |&existing| {
                self.catalog.hasher.hash_one(&self.catalog.atoms[existing])
            });
        Ok(id)
    }
}

#[cfg(test)]
mod tests;
