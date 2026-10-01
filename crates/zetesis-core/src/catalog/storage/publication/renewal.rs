//! Extend sole-owned prefix directories after admitting every reservation.
//!
//! Segment payload remains immutable. Old readable prefixes retained by another
//! consumer take the fresh-publication path instead. Repeated exclusive growth
//! moves directory entries only at geometric capacity boundaries.

use super::super::vocabulary::Vocabulary;
use super::super::{Budget, Fault, RowSegment, Snapshot, Store, VocabularySegment, budget};
use std::{mem::size_of, sync::Arc};

impl Store {
    /// Owner identity and extents authenticate the current sealed prefix. The
    /// exclusive mutable borrow prevents new sharing after this decision.
    pub(super) fn can_renew(&self, prior: &mut Snapshot) -> bool {
        let Some(data) = Arc::get_mut(&mut prior.data) else {
            return false;
        };
        if !Arc::ptr_eq(&self.atom_owner, &prior.atom_owner)
            || data.atoms != self.tail.start
            || data.segments.len() != self.sealed.len()
        {
            return false;
        }
        match &self.vocabulary {
            Vocabulary::Frozen(base) => Arc::ptr_eq(&base.data, &data.vocabulary),
            Vocabulary::Growing(growing) => {
                let Some(vocabulary) = Arc::get_mut(&mut data.vocabulary) else {
                    return false;
                };
                Arc::ptr_eq(&growing.owner, &vocabulary.owner)
                    && growing.tail.start == vocabulary.counts
                    && growing.sealed.len() == vocabulary.segments.len()
            }
        }
    }

    pub(super) fn renewal_steps(&self, prior: &Snapshot) -> Result<usize, Fault> {
        let rows_added = !self.tail.is_empty();
        let rows = growth_steps(&self.sealed, rows_added)
            .checked_add(growth_steps(&prior.data.segments, rows_added))
            .ok_or(Fault::Overflow)?;
        let vocabulary = match &self.vocabulary {
            Vocabulary::Frozen(_) => 0,
            Vocabulary::Growing(growing) => {
                let added = !growing.tail.is_empty();
                growth_steps(&growing.sealed, added)
                    .checked_add(growth_steps(&prior.data.vocabulary.segments, added))
                    .ok_or(Fault::Overflow)?
            }
        };
        // The same fixed metadata operations as fresh publication. Directory
        // relocation is additional; existing references are otherwise untouched.
        rows.checked_add(vocabulary)
            .and_then(|steps| steps.checked_add(16))
            .ok_or(Fault::Overflow)
    }

    pub(super) fn renew(&mut self, prior: &mut Snapshot, extra: u128) -> Result<(), Fault> {
        let data = Arc::get_mut(&mut prior.data).expect("exclusive current snapshot");
        let rows_added = !self.tail.is_empty();
        let vocabulary_added = self.vocabulary.has_unpublished();
        let headers = u128::from(rows_added) * size_of::<RowSegment>() as u128
            + u128::from(vocabulary_added) * size_of::<VocabularySegment>() as u128;
        let mut reservation = Reservation::new(&mut self.budget, extra, headers)?;
        if rows_added {
            reservation.writer(&mut self.sealed)?;
        }
        if let Vocabulary::Growing(growing) = &mut self.vocabulary
            && vocabulary_added
        {
            reservation.writer(&mut growing.sealed)?;
        }
        if rows_added {
            reservation.snapshot(&mut data.segments)?;
        }
        if vocabulary_added {
            let vocabulary = Arc::get_mut(&mut data.vocabulary).expect("exclusive growing prefix");
            reservation.snapshot(&mut vocabulary.segments)?;
        }
        // Every fallible reservation is complete. Counts and visible lengths
        // stay unchanged until this point, even if an earlier reserve failed.
        reservation.finish();
        if let Vocabulary::Growing(growing) = &mut self.vocabulary
            && vocabulary_added
        {
            let vocabulary = Arc::get_mut(&mut data.vocabulary).expect("exclusive growing prefix");
            let counts = growing.tail.counts();
            let tail = std::mem::replace(&mut growing.tail, VocabularySegment::new(counts));
            let segment = Arc::new(tail);
            vocabulary.segments.push(Arc::clone(&segment));
            growing.sealed.push(segment);
            vocabulary.counts = counts;
        }
        let atoms = self.tail.count();
        if rows_added {
            let tail = std::mem::replace(&mut self.tail, RowSegment::new(atoms));
            let segment = Arc::new(tail);
            data.segments.push(Arc::clone(&segment));
            self.sealed.push(segment);
        }
        data.atoms = atoms;
        Ok(())
    }
}

fn growth_steps<T>(directory: &Vec<T>, append: bool) -> usize {
    if append && directory.len() == directory.capacity() {
        directory.len()
    } else {
        0
    }
}

/// One live ledger includes the writer and caller-owned snapshot capacities.
/// Only writer reservations update the writer's retained-byte ledger. A failed
/// reservation still records actual capacity growth and replacement overlap.
struct Reservation<'a> {
    owner: &'a mut Budget,
    live: Budget,
    headers: u128,
}

impl<'a> Reservation<'a> {
    fn new(owner: &'a mut Budget, extra: u128, headers: u128) -> Result<Self, Fault> {
        owner.check_extra(extra)?;
        owner.peak = owner.peak.max(owner.used + extra);
        let mut live = owner.clone();
        live.used += extra;
        live.add(headers)?;
        live.peak = live.used;
        Ok(Self {
            owner,
            live,
            headers,
        })
    }

    fn writer<T>(&mut self, directory: &mut Vec<T>) -> Result<(), Fault> {
        let before = budget::capacity(directory);
        let result = self.snapshot(directory);
        self.owner.used = self.owner.used - before + budget::capacity(directory);
        result
    }

    fn snapshot<T>(&mut self, directory: &mut Vec<T>) -> Result<(), Fault> {
        let result = budget::reserve(directory, 1, &mut self.live);
        // The fixed headers are admitted but not allocated until finish.
        self.owner.peak = self.owner.peak.max(self.live.peak - self.headers);
        result
    }

    fn finish(self) {
        self.owner.used += self.headers;
        self.owner.peak = self.owner.peak.max(self.live.peak);
    }
}
