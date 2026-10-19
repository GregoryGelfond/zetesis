//! Bounded, exact-original reuse of conditional unsupported sets.
//!
//! Every row says that under its signed atom conditions all its output atoms
//! are absent from every answer set. Rows are independent of the region that
//! supplied them. Deleting a row loses pruning only. No rejected candidate,
//! partial closure, objective bound or frozen reduct supplies a certificate.

use super::explanations::{self, Explanation, Input, Limits};
use crate::regions::{SupportExplanationStatistics, Theory, Work, bit};
use zetesis_cpu::Stop;

#[cfg(test)]
mod tests;

const MAX_ROWS: usize = 64;
const MAX_ATTEMPTS: usize = 256;
const CAPTURE_WORK: u64 = 131_072;

#[derive(Debug)]
pub(in crate::regions) struct Ledger {
    owner: Option<Theory>,
    rows: Vec<Explanation>,
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
        self.rows.capacity() as u128 * size_of::<Explanation>() as u128
            + self
                .rows
                .iter()
                .map(Explanation::retained_bytes)
                .sum::<u128>()
    }

    pub(in crate::regions) fn peak_bytes(&self) -> u128 {
        self.peak_bytes.max(self.retained_bytes())
    }

    pub(in crate::regions) fn len(&self) -> usize {
        self.rows.len()
    }

    pub(in crate::regions) fn outputs(&self, row: usize) -> &[usize] {
        &self.rows[row].unsupported
    }

    pub(in crate::regions) fn can_capture(&self) -> bool {
        self.rows.len() < MAX_ROWS && self.attempts < MAX_ATTEMPTS && self.remaining != 0
    }

    /// Discovery and deduplication share the optional body-capture allowance.
    /// Its exhaustion declines retention; a real quota stop still propagates.
    pub(in crate::regions) fn capture_read(&mut self, work: &mut Work<'_>) -> Result<bool, Stop> {
        if self.remaining == 0 {
            return Ok(false);
        }
        work.tick()?;
        self.remaining -= 1;
        Ok(true)
    }

    /// Conditions are a subset query against current knowledge, not an exact
    /// region key. All conditions must hold, including explicitly absent atoms.
    pub(in crate::regions) fn matches(
        &self,
        row: usize,
        held: &[u64],
        cut: &[u64],
        work: &mut Work<'_>,
    ) -> Result<bool, Stop> {
        work.tick()?;
        for &(atom, value) in &self.rows[row].conditions {
            work.tick()?;
            if !bit(if value { held } else { cut }, atom) {
                return Ok(false);
            }
        }
        Ok(true)
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
            let row = &self.rows[position];
            let mut equal = row.conditions.len() == explanation.conditions.len()
                && row.unsupported.len() == explanation.unsupported.len();
            if equal {
                for (index, right) in explanation.conditions.iter().enumerate() {
                    if !self.capture_read(work)? {
                        counts.declined += 1;
                        return Ok(());
                    }
                    if &self.rows[position].conditions[index] != right {
                        equal = false;
                        break;
                    }
                }
            }
            if equal {
                for (index, right) in explanation.unsupported.iter().enumerate() {
                    if !self.capture_read(work)? {
                        counts.declined += 1;
                        return Ok(());
                    }
                    if &self.rows[position].unsupported[index] != right {
                        equal = false;
                        break;
                    }
                }
            }
            if equal {
                counts.declined += 1;
                return Ok(());
            }
        }
        self.rows
            .try_reserve_exact(1)
            .map_err(|_| Stop::Allocation)?;
        self.rows.push(explanation);
        counts.certified += 1;
        self.peak_bytes = self.peak_bytes.max(self.retained_bytes());
        Ok(())
    }
}
