//! Per-pass ordinal measures indexed by the admitted numeric row authority.

use super::{ExtremaMeasure, ExtremumCut};
use crate::Theory;
use zetesis_cpu::Stop;

/// Capacity survives passes; valid bits never do. Measure cells remain safely
/// initialized after interruption, but are readable only after the current
/// pass has published their bit. The cache borrows no subject or source data.
#[derive(Debug, Default)]
pub(crate) struct ExtremaScratch {
    measures: Vec<ExtremaMeasure>,
    valid: Vec<u64>,
}

impl ExtremaScratch {
    fn rows(theory: &Theory) -> usize {
        if theory.parts().extremum_cuts().is_empty() {
            0
        } else {
            theory.parts().rows().len()
        }
    }

    /// Maximum charged reset work, attained by a fresh scratch owner. The dense
    /// measure column initializes every numeric row when any cut descriptor exists,
    /// while each pass clears one validity word per 64 rows. Existing initialized
    /// cells can only reduce this bound, including after an interrupted pass.
    pub(crate) fn reset_work_bound(theory: &Theory) -> u128 {
        let rows = Self::rows(theory);
        rows as u128 + rows.div_ceil(64) as u128
    }

    /// Requested columns for a fresh owner, before observing allocator capacity.
    pub(crate) fn requested_bytes(theory: &Theory) -> u128 {
        let rows = Self::rows(theory);
        Self::bytes(rows, rows.div_ceil(64))
    }

    /// Actual vector payload only; owning workspaces account their own headers.
    pub(crate) fn retained_bytes(&self) -> u128 {
        Self::bytes(self.measures.capacity(), self.valid.capacity())
    }

    /// Complete requested payload, including all capacity retained from prior
    /// subjects. No cache allocation is requested for a theory without cuts.
    pub(crate) fn required_bytes(&self, theory: &Theory) -> u128 {
        let rows = Self::rows(theory);
        Self::bytes(
            self.measures.capacity().max(rows),
            self.valid.capacity().max(rows.div_ceil(64)),
        )
    }

    fn bytes(rows: usize, words: usize) -> u128 {
        rows as u128 * size_of::<ExtremaMeasure>() as u128
            + words as u128 * size_of::<u64>() as u128
    }

    /// Reserve columns without initializing cells or asserting pass validity.
    /// A refused reservation can leave increased capacity for the caller's
    /// actual-byte observation. Initialized lengths remain unchanged.
    pub(crate) fn reserve(&mut self, theory: &Theory) -> Result<(), Stop> {
        self.reserve_observed(theory, |_| Ok::<_, Stop>(()))
    }

    /// Observe complete actual payload after each column reservation, including
    /// its refused attempt, before attempting another allocation. Preserve the
    /// original allocation error when observation also refuses.
    pub(crate) fn reserve_observed<E: From<Stop>>(
        &mut self,
        theory: &Theory,
        mut observe: impl FnMut(u128) -> Result<(), E>,
    ) -> Result<(), E> {
        let rows = Self::rows(theory);
        let reserved = self
            .measures
            .try_reserve_exact(rows.saturating_sub(self.measures.len()))
            .map_err(|_| Stop::Allocation);
        let observed = observe(self.retained_bytes());
        reserved?;
        observed?;
        let reserved = self
            .valid
            .try_reserve_exact(rows.div_ceil(64).saturating_sub(self.valid.len()))
            .map_err(|_| Stop::Allocation);
        let observed = observe(self.retained_bytes());
        reserved?;
        observed
    }

    /// Start one complete topological pass. Charge every validity-word reset
    /// and every newly initialized measure cell before its write. Tail bits
    /// remain zero. Old initialized cells need no write and remain inaccessible
    /// until this pass publishes their individual bit. A stopped reset is not
    /// usable; the next reset starts by discarding every validity word again.
    pub(crate) fn reset<E: From<Stop>>(
        &mut self,
        theory: &Theory,
        mut tick: impl FnMut() -> Result<(), E>,
    ) -> Result<(), E> {
        let rows = Self::rows(theory);
        let words = rows.div_ceil(64);
        self.valid.clear();
        if self.measures.capacity() < rows || self.valid.capacity() < words {
            return Err(Stop::InvalidProgram.into());
        }
        for _ in 0..words {
            tick()?;
            self.valid.push(0);
        }
        while self.measures.len() < rows {
            tick()?;
            self.measures.push(ExtremaMeasure {
                minimum: 0,
                maximum: 0,
            });
        }
        Ok(())
    }

    /// Borrow a completed current-pass measure or publish one after all fallible
    /// reads succeed. The caller has reset for this exact theory/pass and passes
    /// its admitted node view. All row children precede the first referencing
    /// cut, so later cut nodes see the same final predecessor truth. No failure
    /// publishes a partial measure or valid bit.
    pub(crate) fn measure<E: From<Stop>>(
        &mut self,
        theory: &Theory,
        node: usize,
        cut: ExtremumCut<'_>,
        read: impl FnMut(usize) -> Result<bool, E>,
    ) -> Result<ExtremaMeasure, E> {
        let descriptor = theory
            .nodes()
            .get(node)
            .and_then(|node| node.extremum_cut_descriptor())
            .ok_or(Stop::InvalidProgram)?;
        let row = theory
            .parts()
            .extremum_cuts()
            .get(descriptor)
            .ok_or(Stop::InvalidProgram)?
            .row;
        let word = self.valid.get_mut(row / 64).ok_or(Stop::InvalidProgram)?;
        let bit = 1_u64 << (row % 64);
        let measure = self.measures.get_mut(row).ok_or(Stop::InvalidProgram)?;
        if *word & bit == 0 {
            let completed = cut.row.reduce(read)?;
            *measure = completed;
            *word |= bit;
        }
        Ok(*measure)
    }
}

#[cfg(test)]
mod tests;
