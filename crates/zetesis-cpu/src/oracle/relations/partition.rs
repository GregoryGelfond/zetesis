//! Ordered old/new row views over the sole scalar tuple owner.

use zetesis_core::relation::Catalog;

use super::{RowSet, Rows, Work};
use crate::Stop;

/// The round cutoff over one catalog: rows with an insertion ID below it are
/// Old, the rest New. The catalog's last merging preparation keeps exactly
/// these two runs in canonical order, so no derived ID buffer is needed here.
#[derive(Default)]
pub(super) struct Partition {
    old_end: usize,
}

impl Partition {
    pub(super) fn advance(&mut self, length: usize, work: &mut Work<'_>) -> Result<(), Stop> {
        charge(work, 1)?;
        self.old_end = length;
        Ok(())
    }

    pub(super) fn reset(&mut self, work: &mut Work<'_>) -> Result<(), Stop> {
        charge(work, 1)?;
        self.old_end = 0;
        Ok(())
    }

    pub(super) fn has_new(&self, length: usize) -> bool {
        self.old_end < length
    }

    /// Check that the catalog's runs fall at this cutoff. The catalog merges
    /// once per round, after the cutoff advanced and the round's heads were
    /// appended, so the run it merged from is exactly the Old extent.
    pub(super) fn prepare(&self, catalog: &Catalog, work: &mut Work<'_>) -> Result<(), Stop> {
        charge(work, 1)?;
        let rows = catalog.ordered().ok_or(Stop::InvalidProgram)?;
        if self.old_end > rows.len() {
            return Err(Stop::InvalidProgram);
        }
        if self.old_end != 0 && self.old_end != rows.len() {
            let (before, _) = catalog.ordered_runs().ok_or(Stop::InvalidProgram)?;
            if before.len() != self.old_end {
                return Err(Stop::InvalidProgram);
            }
        }
        Ok(())
    }

    pub(super) fn rows<'a>(&'a self, catalog: &'a Catalog, set: RowSet) -> Result<Rows<'a>, Stop> {
        let rows = catalog.ordered().ok_or(Stop::InvalidProgram)?;
        let (first, last) = (self.old_end == 0, self.old_end == rows.len());
        // An all-old or all-new extent is the whole view or nothing.
        match (set, first, last) {
            (RowSet::Current, _, _) | (RowSet::Old, _, true) | (RowSet::New, true, _) => {
                return Ok(Rows::Catalog(rows));
            }
            (RowSet::Old, true, _) | (RowSet::New, _, true) => {
                return Ok(Rows::Borrowed(&[]));
            }
            (RowSet::Old | RowSet::New, false, false) => {}
        }
        let (before, appended) = catalog.ordered_runs().ok_or(Stop::InvalidProgram)?;
        if before.len() != self.old_end {
            return Err(Stop::InvalidProgram);
        }
        Ok(Rows::Selected {
            atoms: catalog.atoms(),
            ids: if set == RowSet::Old { before } else { appended },
        })
    }
}

fn charge(work: &mut Work<'_>, amount: usize) -> Result<(), Stop> {
    let before = work.statistics.work;
    let result = work.charge(amount);
    let charged = work.statistics.work - before;
    work.statistics.catalog_work = work
        .statistics
        .catalog_work
        .checked_add(charged)
        .ok_or(Stop::InvalidProgram)?;
    result
}

#[cfg(test)]
mod tests;
