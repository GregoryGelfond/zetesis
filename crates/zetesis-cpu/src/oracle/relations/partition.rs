//! Ordered old/new ID views over the sole scalar tuple owner.

use std::mem::size_of;
use zetesis_core::relation::Catalog;

use super::{RowSet, Rows, Work, storage};
use crate::Stop;

#[derive(Default)]
pub(super) struct Partition {
    old_end: usize,
    prepared: Option<(usize, usize)>,
    ids: Vec<usize>,
}

impl Partition {
    pub(super) fn advance(&mut self, length: usize, work: &mut Work<'_>) -> Result<(), Stop> {
        charge(work, 1)?;
        self.old_end = length;
        // The cache key includes both this cutoff and the actual catalog extent.
        // Unchanged predicates become all-old without preserving an earlier delta.
        Ok(())
    }

    pub(super) fn reset(&mut self, work: &mut Work<'_>) -> Result<(), Stop> {
        charge(work, 3)?;
        self.old_end = 0;
        self.prepared = None;
        self.ids.clear();
        Ok(())
    }

    pub(super) fn has_new(&self, length: usize) -> bool { self.old_end < length }

    pub(super) fn prepare(&mut self, catalog: &Catalog, live: &mut u128,
        work: &mut Work<'_>) -> Result<(), Stop>
    {
        storage::admit(work, *live)?;
        storage::record(work, *live)?;
        charge(work, 1)?;
        let length = catalog.atoms().len();
        if self.old_end > length { return Err(Stop::InvalidProgram); }
        let key = (self.old_end, length);
        if self.prepared == Some(key) { return Ok(()); }
        charge(work, 1)?;
        self.prepared = None;
        if self.old_end == 0 || self.old_end == length {
            charge(work, 2)?;
            self.ids.clear();
            self.prepared = Some(key);
            return Ok(());
        }
        self.reserve(length, live, work)?;
        charge(work, length.saturating_sub(self.ids.len()))?;
        self.ids.resize(length, 0);
        // Old IDs and New IDs each retain their relative canonical order. The
        // old prefix contains exactly old_end distinct insertion IDs, so both
        // write cursors fill their disjoint, preallocated regions completely.
        let rows = catalog.ordered().ok_or(Stop::InvalidProgram)?;
        let mut old = 0;
        let mut new = self.old_end;
        for rank in 0..length {
            charge(work, 1)?;
            let id = rows.row_id(rank).ok_or(Stop::InvalidProgram)?;
            charge(work, 1)?;
            let position = if id < self.old_end { &mut old } else { &mut new };
            *self.ids.get_mut(*position).ok_or(Stop::InvalidProgram)? = id;
            *position += 1;
        }
        charge(work, 1)?;
        if old != self.old_end || new != length { return Err(Stop::InvalidProgram); }
        self.prepared = Some(key);
        Ok(())
    }

    fn reserve(&mut self, length: usize, live: &mut u128, work: &mut Work<'_>) -> Result<(), Stop> {
        if self.ids.capacity() >= length { return Ok(()); }
        let planned = length as u128 * size_of::<usize>() as u128;
        storage::admit(work, live.checked_add(planned).ok_or(Stop::StorageLimit)?)?;
        charge(work, self.ids.len())?;
        let old = self.ids.capacity() as u128 * size_of::<usize>() as u128;
        self.ids.try_reserve_exact(length - self.ids.len()).map_err(|_| Stop::Allocation)?;
        let actual = self.ids.capacity() as u128 * size_of::<usize>() as u128;
        let overlap = live.checked_add(actual).ok_or(Stop::StorageLimit);
        // Retained capacity changes even when allocator slack refuses the
        // operation. Update its owner before returning any such failure.
        *live = live.checked_sub(old).and_then(|bytes| bytes.checked_add(actual))
            .ok_or(Stop::StorageLimit)?;
        storage::after_reservation(work, overlap?)
    }

    pub(super) fn rows<'a>(&'a self, catalog: &'a Catalog, set: RowSet) -> Result<Rows<'a>, Stop> {
        let rows = catalog.ordered().ok_or(Stop::InvalidProgram)?;
        if set == RowSet::Current { return Ok(Rows::Catalog(rows)); }
        let length = rows.len();
        if self.prepared != Some((self.old_end, length)) { return Err(Stop::InvalidProgram); }
        match (set, self.old_end) {
            (RowSet::Old, 0) => Ok(Rows::Borrowed(&[])),
            (RowSet::New, end) if end == length => Ok(Rows::Borrowed(&[])),
            (RowSet::Old, end) if end == length => Ok(Rows::Catalog(rows)),
            (RowSet::New, 0) => Ok(Rows::Catalog(rows)),
            (RowSet::Old, end) => Ok(Rows::Selected { atoms: catalog.atoms(), ids: &self.ids[..end] }),
            (RowSet::New, end) => Ok(Rows::Selected { atoms: catalog.atoms(), ids: &self.ids[end..] }),
            (RowSet::Current, _) => Ok(Rows::Catalog(rows)),
        }
    }
}

fn charge(work: &mut Work<'_>, amount: usize) -> Result<(), Stop> {
    let before = work.statistics.work;
    let result = work.charge(amount);
    let charged = work.statistics.work - before;
    work.statistics.catalog_work = work.statistics.catalog_work.checked_add(charged)
        .ok_or(Stop::InvalidProgram)?;
    result
}

#[cfg(test)]
mod tests;
