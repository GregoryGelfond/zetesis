//! Append-only packed identities with a bounded, indices-only hash table.
//!
//! Slots are empty or the positive successor of a key's insertion position.
//! The table stays at most half full. Hashing chooses a starting slot only:
//! every occupied candidate is compared against the authoritative packed key.
//! Capacity grows geometrically; collision chains have an explicit work bound,
//! not a worst-case constant-time promise. Hash values are not serialized.
//!
//! Growth copies into a fallibly reserved replacement before swapping buffers.
//! A stopped copy/rehash preserves every previous key and slot. New key words
//! remain unpublished until complete, and are truncated on any failure. Only
//! equivalent capacity changes may survive a failed insertion.

use std::hash::{DefaultHasher, Hasher};
use std::num::NonZeroUsize;

use super::{ProjectionError, ProjectionResource, Work, bytes, ceiling};

type Slot = Option<NonZeroUsize>;

pub(super) struct History {
    width: usize,
    entries: usize,
    keys: Vec<u64>,
    slots: Vec<Slot>,
}

impl History {
    pub(super) const fn new(width: usize) -> Self {
        Self {
            width,
            entries: 0,
            keys: Vec::new(),
            slots: Vec::new(),
        }
    }

    pub(super) const fn len(&self) -> usize {
        self.entries
    }

    fn key(&self, index: usize) -> &[u64] {
        let start = index * self.width;
        &self.keys[start..start + self.width]
    }

    fn retained(&self, base: u128) -> Result<u128, ProjectionError> {
        base.checked_add(bytes::<u64>(self.keys.capacity())?)
            .and_then(|sum| {
                sum.checked_add(self.slots.capacity() as u128 * size_of::<Slot>() as u128)
            })
            .ok_or(ProjectionError::Overflow)
    }

    /// Duplicate lookup precedes entry admission, including the empty key.
    pub(super) fn insert(
        &mut self,
        key: &[u64],
        base: u128,
        work: &mut Work<'_>,
    ) -> Result<bool, ProjectionError> {
        assert_eq!(key.len(), self.width, "fixed projection domain");
        work.charge(1)?;
        if self.width == 0 && self.entries != 0 {
            return Ok(false);
        }
        let hash = hash(key, work)?;
        if !self.slots.is_empty() && self.contains(key, hash, work)? {
            return Ok(false);
        }
        let count = self
            .entries
            .checked_add(1)
            .ok_or(ProjectionError::Overflow)?;
        ceiling(
            ProjectionResource::Keys,
            count as u128,
            work.limits.max_keys as u128,
        )?;
        if self.width == 0 {
            self.entries = count;
            return Ok(true);
        }
        let words = count
            .checked_mul(self.width)
            .ok_or(ProjectionError::Overflow)?;
        self.reserve_keys(words, base, work)?;
        self.reserve_slots(count, base, work)?;
        let slot = empty_slot(&self.slots, hash, work)?;
        let start = self.keys.len();
        let result = self.append(key, work);
        if let Err(error) = result {
            self.keys.truncate(start);
            return Err(error);
        }
        // Both dimensions and the positive successor were admitted above.
        // No callback, allocation or fallible operation follows publication.
        self.slots[slot] = NonZeroUsize::new(count);
        self.entries = count;
        Ok(true)
    }

    fn append(&mut self, key: &[u64], work: &mut Work<'_>) -> Result<(), ProjectionError> {
        for word in key {
            work.charge(1)?;
            self.keys.push(*word);
        }
        work.charge(1)
    }

    fn contains(
        &self,
        key: &[u64],
        hash: u64,
        work: &mut Work<'_>,
    ) -> Result<bool, ProjectionError> {
        let mut slot = first_slot(hash, self.slots.len())?;
        loop {
            work.charge(1)?;
            let Some(index) = self.slots[slot] else {
                return Ok(false);
            };
            if equal(self.key(index.get() - 1), key, work)? {
                return Ok(true);
            }
            slot = (slot + 1) & (self.slots.len() - 1);
        }
    }

    fn reserve_keys(
        &mut self,
        required: usize,
        base: u128,
        work: &mut Work<'_>,
    ) -> Result<(), ProjectionError> {
        if required <= self.keys.capacity() {
            return Ok(());
        }
        let capacity = capacity(required)?;
        let retained = self.retained(base)?;
        work.admit_bytes(
            retained
                .checked_add(bytes::<u64>(capacity)?)
                .ok_or(ProjectionError::Overflow)?,
        )?;
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(capacity)
            .map_err(|_| ProjectionError::Allocation)?;
        work.observe(retained, bytes::<u64>(replacement.capacity())?)?;
        for word in &self.keys {
            work.charge(1)?;
            replacement.push(*word);
        }
        self.keys = replacement;
        work.observe(self.retained(base)?, 0)
    }

    fn reserve_slots(
        &mut self,
        count: usize,
        base: u128,
        work: &mut Work<'_>,
    ) -> Result<(), ProjectionError> {
        let required = count.checked_mul(2).ok_or(ProjectionError::Overflow)?;
        if required <= self.slots.len() {
            return Ok(());
        }
        let capacity = capacity(required)?;
        let retained = self.retained(base)?;
        work.admit_bytes(
            retained
                .checked_add(bytes::<Slot>(capacity)?)
                .ok_or(ProjectionError::Overflow)?,
        )?;
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(capacity)
            .map_err(|_| ProjectionError::Allocation)?;
        work.observe(retained, bytes::<Slot>(replacement.capacity())?)?;
        for _ in 0..capacity {
            work.charge(1)?;
            replacement.push(None);
        }
        for index in 0..self.entries {
            let hash = hash(self.key(index), work)?;
            let slot = empty_slot(&replacement, hash, work)?;
            replacement[slot] = NonZeroUsize::new(index + 1);
        }
        self.slots = replacement;
        work.observe(self.retained(base)?, 0)
    }
}

fn capacity(required: usize) -> Result<usize, ProjectionError> {
    required
        .max(4)
        .checked_next_power_of_two()
        .ok_or(ProjectionError::Overflow)
}

fn hash(key: &[u64], work: &mut Work<'_>) -> Result<u64, ProjectionError> {
    // This standard-library hasher is deterministic within the build. Neither
    // its algorithm nor its result is a persistent identity or public format.
    let mut hasher = DefaultHasher::new();
    for word in key {
        work.charge(1)?;
        hasher.write_u64(*word);
    }
    Ok(hasher.finish())
}

fn first_slot(hash: u64, length: usize) -> Result<usize, ProjectionError> {
    let mask = u64::try_from(length - 1).map_err(|_| ProjectionError::Overflow)?;
    usize::try_from(hash & mask).map_err(|_| ProjectionError::Overflow)
}

fn empty_slot(slots: &[Slot], hash: u64, work: &mut Work<'_>) -> Result<usize, ProjectionError> {
    let mut slot = first_slot(hash, slots.len())?;
    loop {
        work.charge(1)?;
        if slots[slot].is_none() {
            return Ok(slot);
        }
        slot = (slot + 1) & (slots.len() - 1);
    }
}

fn equal(left: &[u64], right: &[u64], work: &mut Work<'_>) -> Result<bool, ProjectionError> {
    for (left, right) in left.iter().zip(right) {
        work.charge(1)?;
        if left != right {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests;
