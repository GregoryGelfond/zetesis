//! Optional decode evidence for one authenticated slate attempt. Original slot
//! numbers preserve the association across device submissions and CPU checks.

use std::mem::size_of;

use zetesis_algebra::MeterBytes;
use zetesis_wgpu::ResidualReason;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DecodedSlot {
    pub(crate) slot: usize,
    pub(crate) reason: ResidualReason,
    pub(crate) open_atoms: u64,
}

#[derive(Debug)]
pub(crate) struct DecodeJournal {
    // None means the optional record was lost. An empty vector is an observed
    // prefix with zero residuals and owns no heap storage.
    slots: Option<Vec<DecodedSlot>>,
}

impl Default for DecodeJournal {
    fn default() -> Self {
        Self {
            slots: Some(Vec::new()),
        }
    }
}

impl DecodeJournal {
    pub(crate) fn record(
        &mut self,
        slot: usize,
        slate_len: usize,
        reason: ResidualReason,
        open_atoms: u64,
        limit: u64,
    ) {
        let Some(rows) = &mut self.slots else { return };
        if slot >= slate_len || rows.last().is_some_and(|last| last.slot >= slot) {
            self.slots = None;
            return;
        }
        if rows.is_empty() {
            let Some(reserved) = reserve(slate_len, limit) else {
                self.slots = None;
                return;
            };
            *rows = reserved;
        } else if slate_len > rows.capacity() || payload(rows.capacity()) > u128::from(limit) {
            // The authenticated slate has fixed length. Recording another row
            // must never grow the already charged allocation.
            self.slots = None;
            return;
        }
        rows.push(DecodedSlot {
            slot,
            reason,
            open_atoms,
        });
    }

    pub(crate) fn bytes(&self) -> u64 {
        self.slots.as_ref().map_or(0, |rows| {
            u64::try_from(payload(rows.capacity())).expect("capacity admitted under a u64 limit")
        })
    }

    /// Yield optional decode evidence to mandatory storage when their live
    /// capacities would otherwise overlap beyond the fixed allowance.
    pub(crate) fn release_unless_fits(&mut self, max_bytes: u64) {
        if self.bytes() > max_bytes {
            self.slots = None;
        }
    }

    pub(crate) fn rows(&self) -> Option<&[DecodedSlot]> {
        self.slots.as_deref()
    }
}

fn payload(capacity: usize) -> u128 {
    capacity as u128 * size_of::<DecodedSlot>() as u128
}

fn reserve(slate_len: usize, limit: u64) -> Option<Vec<DecodedSlot>> {
    let limit = u128::from(limit);
    MeterBytes::check(payload(slate_len), limit).ok()?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(slate_len).ok()?;
    MeterBytes::check(payload(rows.capacity()), limit).ok()?;
    Some(rows)
}

#[cfg(test)]
mod tests;
