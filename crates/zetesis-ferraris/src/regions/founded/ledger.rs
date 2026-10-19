//! Bounded, exact-original reuse of conditional unsupported sets.
//!
//! Every row says that under its signed atom conditions all its output atoms
//! are absent from every answer set. Rows are independent of the region that
//! supplied them. Deleting a row loses pruning only. No rejected candidate,
//! partial closure, objective bound or frozen reduct supplies a certificate.

use super::explanations::{self, Input, Limits};
use crate::regions::{SupportExplanationStatistics, Theory, Work};
use zetesis_cpu::Stop;

mod row;

pub(in crate::regions) use row::OutputCursor;
use row::Row;

#[cfg(test)]
mod tests;

const MAX_ROWS: usize = 64;
const MAX_ATTEMPTS: usize = 256;
const CAPTURE_WORK: u64 = 131_072;

/// A consequence of one certified row under the current signed atom masks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::regions) enum Deduction {
    None,
    ExcludeOutputs,
    Require { atom: usize, value: bool },
}

#[derive(Debug)]
pub(in crate::regions) struct Ledger {
    owner: Option<Theory>,
    rows: Vec<Row>,
    attempts: usize,
    remaining: u64,
    peak_bytes: u128,
}

impl Default for Ledger {
    fn default() -> Self {
        Self {
            owner: None,
            rows: Vec::new(),
            attempts: 0,
            remaining: CAPTURE_WORK,
            peak_bytes: 0,
        }
    }
}

impl Ledger {
    /// Only an eligible original subject calls this. Restriction and frozen
    /// visits suspend use rather than discarding the original's deductions.
    pub(in crate::regions) fn prepare(&mut self, theory: &Theory) {
        if self
            .owner
            .as_ref()
            .is_none_or(|owner| !owner.same_instance(theory))
        {
            self.rows.clear();
            self.owner = Some(theory.clone());
            self.attempts = 0;
            self.remaining = CAPTURE_WORK;
            self.peak_bytes = self.retained_bytes();
        }
    }

    pub(in crate::regions) fn retained_bytes(&self) -> u128 {
        self.rows.capacity() as u128 * size_of::<Row>() as u128
            + self.rows.iter().map(Row::retained_bytes).sum::<u128>()
    }

    pub(in crate::regions) fn peak_bytes(&self) -> u128 {
        self.peak_bytes.max(self.retained_bytes())
    }

    pub(in crate::regions) fn len(&self) -> usize {
        self.rows.len()
    }

    #[cfg(test)]
    pub(in crate::regions) fn outputs(&self, row: usize) -> impl Iterator<Item = usize> + '_ {
        self.rows[row].outputs()
    }

    pub(in crate::regions) fn output_count(&self, row: usize) -> usize {
        self.rows[row].output_count()
    }

    pub(in crate::regions) fn next_output(
        &self,
        row: usize,
        cursor: &mut OutputCursor,
    ) -> Option<usize> {
        self.rows[row].next_output(cursor)
    }

    pub(in crate::regions) fn can_capture(&self) -> bool {
        self.rows.len() < MAX_ROWS && self.attempts < MAX_ATTEMPTS && self.remaining != 0
    }

    /// Discovery, subsumption and conversion share the optional capture allowance.
    /// Its exhaustion declines retention; a real quota stop still propagates.
    pub(in crate::regions) fn capture_read(&mut self, work: &mut Work<'_>) -> Result<bool, Stop> {
        capture_read(&mut self.remaining, work)
    }

    /// A complete condition excludes every output. If an output is held and
    /// exactly one condition is open, that condition must instead be false.
    /// An opposite premise or two open premises supplies no deduction. This
    /// scans scalar entries or compact word groups and allocates no matching state.
    pub(in crate::regions) fn deduction(
        &self,
        row: usize,
        held: &[u64],
        cut: &[u64],
        work: &mut Work<'_>,
    ) -> Result<Deduction, Stop> {
        self.rows[row].deduction(held, cut, work)
    }

    pub(in crate::regions) fn capture(
        &mut self,
        input: Input<'_>,
        atom: usize,
        work: &mut Work<'_>,
        counts: &mut SupportExplanationStatistics,
    ) -> Result<(), Stop> {
        if !self.can_capture() {
            return Ok(());
        }
        self.attempts += 1;
        counts.attempts += 1;
        let mut temporary_peak = 0;
        let before = work.spent;
        let result = explanations::collect(
            input,
            atom,
            Limits {
                atoms: 64,
                literals: 16,
                visits: self.remaining.min(4096),
                bytes: 65_536,
            },
            work,
            &mut temporary_peak,
        );
        self.remaining = self.remaining.saturating_sub(work.spent - before);
        self.peak_bytes = self.peak_bytes.max(self.retained_bytes() + temporary_peak);
        let Some(explanation) = result? else {
            counts.declined += 1;
            return Ok(());
        };
        for position in 0..self.rows.len() {
            if !self.capture_read(work)? {
                counts.declined += 1;
                return Ok(());
            }
            let Some(subsumed) =
                self.rows[position].subsumes(&explanation, &mut self.remaining, work)?
            else {
                counts.declined += 1;
                return Ok(());
            };
            if subsumed {
                counts.declined += 1;
                return Ok(());
            }
        }
        let prepared = Row::prepare(explanation, &mut self.remaining, work, &mut temporary_peak);
        self.peak_bytes = self.peak_bytes.max(self.retained_bytes() + temporary_peak);
        let Some(row) = prepared? else {
            counts.declined += 1;
            return Ok(());
        };
        self.rows
            .try_reserve_exact(1)
            .map_err(|_| Stop::Allocation)?;
        self.peak_bytes = self
            .peak_bytes
            .max(self.retained_bytes() + size_of::<Row>() as u128 + row.retained_bytes());
        self.rows.push(row);
        counts.certified += 1;
        self.peak_bytes = self.peak_bytes.max(self.retained_bytes());
        Ok(())
    }
}

/// One finite capture allowance covers discovery, comparison and conversion.
fn capture_read(remaining: &mut u64, work: &mut Work<'_>) -> Result<bool, Stop> {
    if *remaining == 0 {
        return Ok(false);
    }
    work.tick()?;
    *remaining -= 1;
    Ok(true)
}
