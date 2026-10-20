//! Bounded, exact-original reuse of conditional unsupported sets.
//!
//! Every row says that under its signed atom conditions all its output atoms
//! are absent from every answer set. Rows are independent of the region that
//! supplied them. Deleting a row loses pruning only. No rejected candidate,
//! partial closure, objective bound or frozen reduct supplies a certificate.

use super::explanations::{self, Explanation, Input, Limits};
use crate::regions::{SupportExplanationStatistics, Theory, Work};
use zetesis_cpu::Stop;

mod row;

pub(in crate::regions) use row::OutputCursor;
use row::Row;

#[cfg(test)]
mod tests;

const MAX_ROWS: usize = u64::BITS as usize;
const MAX_OUTPUTS: usize = 64;
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
                atoms: MAX_OUTPUTS,
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
        let retained = self.retained_bytes();
        let result = self.retain(explanation, work, &mut temporary_peak);
        self.peak_bytes = self
            .peak_bytes
            .max(retained + temporary_peak)
            .max(self.retained_bytes());
        if result? {
            counts.certified += 1;
        } else {
            counts.declined += 1;
        }
        Ok(())
    }

    /// Consolidate a completed certificate without changing any retained row
    /// until every comparison, allocation and publication visit has succeeded.
    /// Row positions are private to a matching pass; capture runs between passes.
    fn retain(
        &mut self,
        mut explanation: Explanation,
        work: &mut Work<'_>,
        peak: &mut u128,
    ) -> Result<bool, Stop> {
        // Existing admission keeps the row count below this one-word mask.
        debug_assert!(self.rows.len() < MAX_ROWS);
        let mut replaced = 0_u64;
        for position in 0..self.rows.len() {
            if !self.capture_read(work)? {
                return Ok(false);
            }
            let Some(subsumed) =
                self.rows[position].subsumes(&explanation, &mut self.remaining, work)?
            else {
                return Ok(false);
            };
            if subsumed {
                return Ok(false);
            }
            let Some(combined) = self.rows[position].union_outputs(
                &mut explanation,
                MAX_OUTPUTS,
                &mut self.remaining,
                work,
                peak,
            )?
            else {
                return Ok(false);
            };
            if combined {
                replaced |= 1_u64 << position;
            }
        }
        // A union can dominate a row examined before its outputs were added.
        // Compare the completed proposed certificate against every other row.
        for position in 0..self.rows.len() {
            if replaced & (1_u64 << position) != 0 {
                continue;
            }
            if !self.capture_read(work)? {
                return Ok(false);
            }
            let Some(dominated) =
                self.rows[position].subsumed_by(&explanation, &mut self.remaining, work)?
            else {
                return Ok(false);
            };
            if dominated {
                replaced |= 1_u64 << position;
            }
        }
        let Some(row) = Row::prepare(explanation, &mut self.remaining, work, peak)? else {
            return Ok(false);
        };
        if replaced == 0 {
            self.rows
                .try_reserve_exact(1)
                .map_err(|_| Stop::Allocation)?;
            self.peak_bytes = self
                .peak_bytes
                .max(self.retained_bytes() + size_of::<Row>() as u128 + row.retained_bytes());
            self.rows.push(row);
        } else {
            // Pay for the bounded compaction before its first mutation. A stop
            // during this admission preserves all previous explanations.
            for _ in 0..self.rows.len() {
                if !self.capture_read(work)? {
                    return Ok(false);
                }
            }
            let first = replaced.trailing_zeros() as usize;
            self.rows[first] = row;
            let mut position = 0;
            self.rows.retain(|_| {
                let keep = position == first || replaced & (1_u64 << position) == 0;
                position += 1;
                keep
            });
        }
        Ok(true)
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
