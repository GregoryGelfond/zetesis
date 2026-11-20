//! Discovery positions: a view over the store's atom identities, never an order.
//!
//! A hash table maps each discovered canonical `AtomId` to its discovery
//! position. It holds positions only; equality reads the committed/pending
//! identity map, so the table never copies an identity. A find is one probe
//! and allocates nothing; the table grows only when a new position is
//! discovered, through the owner's admitted reservation.

use hashbrown::HashTable;

use super::{AtomAppender, AtomId, Failure, Limits, admit};

#[derive(Default)]
pub(super) struct Discovery {
    table: HashTable<u32>,
}

fn hash(atom: AtomId) -> u64 {
    crate::mix_word(0, u64::from(atom.word()))
}

pub(super) fn identity(committed: &[AtomId], pending: &[AtomId], id: usize) -> Option<AtomId> {
    if id < committed.len() {
        committed.get(id)
    } else {
        pending.get(id - committed.len())
    }
    .copied()
}

fn rehash(committed: &[AtomId], pending: &[AtomId]) -> impl Fn(&u32) -> u64 {
    move |&position| {
        hash(identity(committed, pending, position as usize).expect("published discovery"))
    }
}

impl Discovery {
    /// One charged probe. Tag-filtered equality checks within the probed
    /// group read the identity map and allocate nothing.
    pub(super) fn find<E>(
        &self,
        committed: &[AtomId],
        pending: &[AtomId],
        atom: AtomId,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Option<usize>, E> {
        before()?;
        Ok(self
            .table
            .find(hash(atom), |&position| {
                identity(committed, pending, position as usize) == Some(atom)
            })
            .map(|&position| position as usize))
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.table.len()
    }

    /// Bytes of the table's allocation: identity cells and control bytes.
    pub(super) fn bytes(&self) -> u128 {
        self.table.allocation_size() as u128
    }

    /// Publish a reserved position for a new identity. Allocates nothing.
    pub(super) fn insert(
        &mut self,
        committed: &[AtomId],
        pending: &[AtomId],
        atom: AtomId,
        position: usize,
    ) {
        debug_assert!(self.table.len() < self.table.capacity());
        let position = u32::try_from(position).expect("admitted discovery position");
        self.table
            .insert_unique(hash(atom), position, rehash(committed, pending));
    }
}

/// The most bytes a table holding `count` positions may allocate across one
/// growth: the old and the new allocation live at once.
pub(super) fn bound(count: usize) -> u128 {
    let buckets = (count.saturating_mul(8) / 7 + 1).next_power_of_two().max(4);
    // Buckets of `u32` plus one control byte each and a group of trailing
    // control bytes, for the old and the doubled table.
    3 * (buckets as u128 * (size_of::<u32>() as u128 + 1) + 16)
}

impl AtomAppender<'_> {
    /// Reserve one discovery cell after canonical admission and before any
    /// publication. Growth is admitted with its old/new overlap first.
    pub(super) fn reserve_discovery<E>(
        &mut self,
        extra: u128,
        limits: Limits,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        before().map_err(Failure::Stopped)?;
        let table = &self.discovery.table;
        if table.len() < table.capacity() {
            return Ok(());
        }
        // Growth at least doubles the buckets; the old table stays live until
        // its positions move.
        let grown = (2 * table.allocation_size()).max(64) as u128;
        let live = self.storage_bytes() + extra + grown;
        admit(live, limits)?;
        *self.peak = (*self.peak).max(live);
        for _ in 0..table.len() {
            before().map_err(Failure::Stopped)?; // Each moved position.
        }
        let (committed, pending) = (self.committed, &*self.pending);
        self.discovery
            .table
            .try_reserve(1, rehash(committed, pending))
            .map_err(|_| Failure::Catalog(crate::catalog::Error::Allocation))?;
        *self.peak = (*self.peak).max(self.storage_bytes() + extra);
        Ok(())
    }
}
