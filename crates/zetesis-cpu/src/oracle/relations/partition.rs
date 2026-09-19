//! Ordered old/new row views over the sole scalar tuple owner.

use zetesis_core::relation::Catalog;

use super::{RowSet, Rows, Work};
use crate::Stop;

/// The round cutoff over one catalog: rows with an insertion ID below it are
/// Old, the rest New. The catalog's last appending preparation keeps the Old
/// rows as its levels and the New rows as its tail.
#[derive(Default)]
pub(in crate::oracle) struct Partition {
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

    /// Confirm that the catalog's runs fall at this cutoff. The catalog
    /// prepares once per round, after the cutoff advanced and the round's
    /// heads were appended, so its levels hold exactly the Old extent.
    pub(super) fn confirm(&self, catalog: &Catalog, work: &mut Work<'_>) -> Result<(), Stop> {
        charge(work, 1)?;
        let runs = catalog.ordered().ok_or(Stop::InvalidProgram)?;
        let length = runs.len();
        if self.old_end > length {
            return Err(Stop::InvalidProgram);
        }
        if self.old_end != 0 && self.old_end != length && self.old_end != length - runs.tail().len()
        {
            return Err(Stop::InvalidProgram);
        }
        Ok(())
    }

    pub(super) fn rows<'a>(&'a self, catalog: &'a Catalog, set: RowSet) -> Result<Rows<'a>, Stop> {
        let runs = catalog.ordered().ok_or(Stop::InvalidProgram)?;
        let length = runs.len();
        let (first, last) = (self.old_end == 0, self.old_end == length);
        let atoms = catalog.atoms();
        // An all-old or all-new extent is the whole view or nothing.
        let (levels, tail) = match (set, first, last) {
            (RowSet::Current, _, _) | (RowSet::Old, _, true) | (RowSet::New, true, _) => {
                (runs.levels(), runs.tail())
            }
            (RowSet::Old, true, _) | (RowSet::New, _, true) => (&[][..], &[][..]),
            (RowSet::Old, false, false) => (runs.levels(), &[][..]),
            (RowSet::New, false, false) => (&[][..], runs.tail()),
        };
        if !(first || last) && length - runs.tail().len() != self.old_end {
            return Err(Stop::InvalidProgram);
        }
        Ok(Rows::Runs {
            atoms,
            levels,
            tail,
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
