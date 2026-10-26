//! Ordered result storage for a completion attempt, and its admission beside
//! the worker queries.

use std::mem::size_of;

use zetesis_ferraris::Interpretation;

use super::Outcome;
use crate::Incomplete;

/// The full ordered results, accepted flags and returned interpretation
/// slots of `candidates`, in bytes. Allocator metadata, thread stacks, the
/// shared original theory and separate candidate/proposal storage are
/// excluded.
///
/// # Errors
/// Refuses an unrepresentable byte count.
pub(super) fn results(candidates: usize) -> Result<u64, Incomplete> {
    u64::try_from(
        candidates as u128
            * (size_of::<Option<Outcome>>() + size_of::<bool>() + size_of::<Interpretation>())
                as u128,
    )
    .map_err(|_| Incomplete::CounterOverflow)
}

/// Admit `results` bytes of ordered results and at most `workers` queries
/// under `limit`, returning the queries admitted. No query storage is
/// estimated in advance: each worker's workspace is measured as it is
/// created, so each admitted query needs one byte of the allowance left
/// beside the results. Zero queries is valid when all supplied certificates
/// decide membership.
///
/// # Errors
/// Refuses results that do not fit, and queries with no allowance left.
pub(super) fn admit(results: u64, workers: usize, limit: u64) -> Result<usize, Incomplete> {
    let remaining = limit
        .checked_sub(results)
        .ok_or(Incomplete::CompletionScratch)?;
    if workers == 0 {
        return Ok(0);
    }
    match usize::try_from(remaining)
        .unwrap_or(usize::MAX)
        .min(workers)
    {
        0 => Err(Incomplete::CompletionScratch),
        count => Ok(count),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_byte_overflow_is_refused_before_allocation() {
        assert_eq!(results(usize::MAX), Err(Incomplete::CounterOverflow));
    }

    #[test]
    fn admission_refuses_results_or_queries_that_do_not_fit() {
        assert_eq!(admit(2, 0, 1), Err(Incomplete::CompletionScratch));
        assert_eq!(admit(2, 4, 2), Err(Incomplete::CompletionScratch));
        assert_eq!(admit(2, 0, 2), Ok(0));
        assert_eq!(admit(2, 4, 4), Ok(2));
        assert_eq!(admit(2, 4, u64::MAX), Ok(4));
    }
}
