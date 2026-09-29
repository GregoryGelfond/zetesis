//! Batch limits, and an oracle that leaves every candidate residual, for the
//! propositions about candidate batches.

use std::num::NonZeroUsize;

use zetesis_ferraris::{Interpretation, Theory};
use zetesis_sat::{BatchLimits, BatchVerdict};

/// A batch oracle that decides nothing: every candidate stays residual.
pub fn residual(
    _: &Theory,
    candidates: &[Interpretation],
) -> Result<Vec<BatchVerdict>, std::collections::TryReserveError> {
    let mut verdicts = Vec::new();
    verdicts.try_reserve_exact(candidates.len())?;
    verdicts.resize(candidates.len(), BatchVerdict::Residual);
    Ok(verdicts)
}

/// Batches of at most `count` candidates and 1 MiB of pending bytes.
pub fn batch(count: usize) -> BatchLimits {
    BatchLimits {
        max_candidates: NonZeroUsize::new(count).unwrap(),
        max_pending_bytes: 1024 * 1024,
    }
}
