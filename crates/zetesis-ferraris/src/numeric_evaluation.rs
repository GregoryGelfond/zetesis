//! Per-pass sums and extrema indexed by one admitted numeric row authority.
//!
//! One 16-byte slot holds either an `i128` sum or two `u64` extrema. The immutable
//! `NumericRow` domain determines its interpretation; the slot is never a second
//! row authority. Byte conversions are safe Rust, with no union or casts.
//! One invariant governs guard reads: a row whose validity bit is set holds its
//! exact measure under the node truth its owner currently holds. A complete original
//! or frozen evaluation pass clears every bit first and sets each as it reduces
//! the row. A retained tight evaluation adjusts a valid sum once per changed
//! occurrence and withdraws a changed extremum. All conditions precede every
//! guard, so a guard sees the final updated sum or reduces an invalid row.
//! Applying one changed node's incidence list is indivisible with respect to
//! guard evaluation, including when that list repeats the same numeric row.
//! Interrupted updates withdraw their owning evaluation before any later read.

use crate::{ExtremaMeasure, ExtremumCut, NumericDomain, Theory, Threshold};
use zetesis_cpu::Stop;

/// Capacity survives passes; valid bits survive only into an update of the
/// evaluation that published them, under the invariant the module states.
/// Measure cells remain safely initialized after interruption, but a cell is
/// read only while its bit is set. The cache borrows no subject or source data.
#[derive(Debug, Default)]
pub(crate) struct NumericRowScratch {
    measures: Vec<Measure>,
    valid: Vec<u64>,
}

impl NumericRowScratch {
    fn rows(theory: &Theory) -> usize {
        theory.evaluation_rows()
    }

    /// Maximum charged reset work, attained by a fresh scratch owner. The dense
    /// measure column initializes every numeric row when any numeric node exists,
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
    /// subjects. No cache allocation is requested for a theory without live numeric nodes.
    pub(crate) fn required_bytes(&self, theory: &Theory) -> u128 {
        let rows = Self::rows(theory);
        Self::bytes(
            self.measures.capacity().max(rows),
            self.valid.capacity().max(rows.div_ceil(64)),
        )
    }

    fn bytes(rows: usize, words: usize) -> u128 {
        rows as u128 * size_of::<Measure>() as u128 + words as u128 * size_of::<u64>() as u128
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
            self.measures.push(Measure([0; 2]));
        }
        Ok(())
    }

    /// Compare a guard with its row's one complete sum for this pass. Each
    /// occurrence, including zero weights and repeats, is read on the first
    /// guard only. The caller applies this node's own frozen truth gate.
    #[inline]
    pub(crate) fn threshold<E: From<Stop>>(
        &mut self,
        theory: &Theory,
        node: usize,
        threshold: Threshold<'_>,
        read: impl FnMut(usize) -> Result<bool, E>,
    ) -> Result<bool, E> {
        let descriptor = theory
            .nodes()
            .get(node)
            .and_then(|node| node.threshold_descriptor())
            .ok_or(Stop::InvalidProgram)?;
        let row = theory
            .parts()
            .thresholds()
            .get(descriptor)
            .ok_or(Stop::InvalidProgram)?
            .row;
        if theory.parts().rows()[row].domain != NumericDomain::Weights {
            return Err(Stop::InvalidProgram.into());
        }
        let measure = self.cached(row, || threshold.selected_sum(read).map(Measure::sum))?;
        Ok(measure.selected_sum() >= threshold.bound)
    }

    /// Reduce an ordinal row once per pass. The node and borrowed view come
    /// from this exact admitted theory; repeated guards use the same row ID.
    #[inline]
    pub(crate) fn extremum<E: From<Stop>>(
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
        if !matches!(
            theory.parts().rows()[row].domain,
            NumericDomain::Ordinal { .. }
        ) {
            return Err(Stop::InvalidProgram.into());
        }
        self.cached(row, || cut.row.reduce(read).map(Measure::ordinal))
            .map(Measure::extrema)
    }

    /// Apply one changed condition occurrence. The caller owns the same theory
    /// and visits each occurrence once after its truth changes; `selected` is
    /// its new truth. Row identity and weight come only from the authoritative
    /// columns. Repeated conditions remain distinct contributions, including
    /// zero weights. The caller charges the reader visit before this operation.
    ///
    /// A valid sum is adjusted exactly; an ordinal measure is withdrawn. An
    /// invalid measure stays invalid, so the next guard performs a full read.
    /// No guard may run until every earlier changed condition has been handled.
    pub(crate) fn change_condition(
        &mut self,
        theory: &Theory,
        row: usize,
        occurrence: usize,
        selected: bool,
    ) -> Result<(), Stop> {
        let descriptor = theory.parts().rows().get(row).ok_or(Stop::InvalidProgram)?;
        if occurrence >= descriptor.length {
            return Err(Stop::InvalidProgram);
        }
        let word = self.valid.get_mut(row / 64).ok_or(Stop::InvalidProgram)?;
        let bit = 1_u64 << (row % 64);
        if *word & bit == 0 {
            return Ok(());
        }
        if descriptor.domain != NumericDomain::Weights {
            *word &= !bit;
            return Ok(());
        }
        let position = descriptor
            .start
            .checked_add(occurrence)
            .ok_or(Stop::InvalidProgram)?;
        let weight = i128::from(
            *theory
                .parts()
                .numeric_values()
                .get(position)
                .ok_or(Stop::InvalidProgram)?,
        );
        let measure = self.measures.get_mut(row).ok_or(Stop::InvalidProgram)?;
        let sum = measure.selected_sum();
        let updated = if selected {
            sum.checked_add(weight)
        } else {
            sum.checked_sub(weight).filter(|&value| value >= 0)
        }
        .ok_or(Stop::InvalidProgram)?;
        *measure = Measure::sum(updated);
        Ok(())
    }

    /// Domain interpretation is fixed by the admitted descriptor before this
    /// call. A set bit holds the row's exact measure under the current truth
    /// (the module's invariant); otherwise the row is reduced now. All children
    /// precede the first referencing guard, so its complete measure remains
    /// valid for later guards even when unrelated rows are evaluated between
    /// them.
    #[inline]
    fn cached<E: From<Stop>>(
        &mut self,
        row: usize,
        reduce: impl FnOnce() -> Result<Measure, E>,
    ) -> Result<Measure, E> {
        let word = self.valid.get_mut(row / 64).ok_or(Stop::InvalidProgram)?;
        let bit = 1_u64 << (row % 64);
        let measure = self.measures.get_mut(row).ok_or(Stop::InvalidProgram)?;
        if *word & bit == 0 {
            let completed = reduce()?;
            *measure = completed;
            *word |= bit;
        }
        Ok(*measure)
    }
}

/// A compact safely initialized slot; `NumericRow` supplies its only domain tag.
#[derive(Clone, Copy, Debug)]
struct Measure([u64; 2]);

impl Measure {
    fn sum(total: i128) -> Self {
        let bytes = total.to_le_bytes();
        Self([
            u64::from_le_bytes(bytes[..8].try_into().expect("eight low bytes")),
            u64::from_le_bytes(bytes[8..].try_into().expect("eight high bytes")),
        ])
    }

    fn selected_sum(self) -> i128 {
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&self.0[0].to_le_bytes());
        bytes[8..].copy_from_slice(&self.0[1].to_le_bytes());
        i128::from_le_bytes(bytes)
    }

    fn ordinal(measure: ExtremaMeasure) -> Self {
        Self([measure.minimum, measure.maximum])
    }

    fn extrema(self) -> ExtremaMeasure {
        ExtremaMeasure {
            minimum: self.0[0],
            maximum: self.0[1],
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod weighted_tests;
