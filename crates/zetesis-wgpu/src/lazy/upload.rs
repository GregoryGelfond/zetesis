//! Input validity within one batch-owned transport. No address or global cache.

use super::{Plan, plan::Retention};
use crate::{GpuError, GpuErrorKind};
use zetesis_cpu::lazy::Chunk;

/// A successful submission installed both input prefixes for this batch-local
/// round and layout. The owning transport never crosses a public batch boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Receipt {
    round: u64,
    words: usize,
    worlds: usize,
}

#[cfg(test)]
mod tests;

impl From<&Chunk<'_>> for Receipt {
    fn from(chunk: &Chunk<'_>) -> Self {
        Self {
            round: chunk.round_index(),
            words: chunk.words(),
            worlds: chunk.worlds(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Uploads {
    pub(super) snapshots: bool,
    pub(super) seeds: bool,
}

impl Uploads {
    pub(super) const ALL: Self = Self {
        snapshots: true,
        seeds: true,
    };

    pub(super) fn needed(previous: Option<Receipt>, current: Receipt, retained: Retention) -> Self {
        let Some(previous) = previous else {
            return Self::ALL;
        };
        let same_layout = previous.words == current.words && previous.worlds == current.worlds;
        Self {
            snapshots: !retained.inputs[2] || !same_layout || previous.round != current.round,
            seeds: !retained.inputs[3] || !same_layout,
        }
    }

    /// Actual writes. The plan retains the unchanged conservative full-input
    /// host allowance for admission even when valid device prefixes are reused.
    pub(super) fn bytes(self, plan: &Plan) -> Result<u64, GpuError> {
        plan.uploaded_bytes
            .checked_sub(u64::from(!self.snapshots) * plan.capacity.inputs[2])
            .and_then(|bytes| bytes.checked_sub(u64::from(!self.seeds) * plan.capacity.inputs[3]))
            .ok_or_else(|| {
                GpuError::new(GpuErrorKind::Capacity, "lazy upload accounting underflow")
            })
    }
}
