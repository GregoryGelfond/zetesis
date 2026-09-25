//! A sparse canonical-to-local equality translation beside semantic ordering.
//!
//! Published entries are exactly the append dictionary's representatives. The
//! bound source supplies their vocabulary; no map entry retains a scope or
//! payload. Hash operations admit one fixed-key container operation, not each
//! opaque internal probe. Semantic ordering remains the foreign-input fallback.

use std::collections::HashMap;

use crate::catalog::{CatalogRead, ReadError, TermRef, storage::TermId};

use super::{Failure, Resource, Work, ceiling};

#[derive(Default)]
pub(super) struct Append {
    pub order: crate::ordered_index::Index,
    pub identities: Inverse,
}

#[derive(Default)]
pub(super) struct Inverse {
    entries: HashMap<TermId, u32>,
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
