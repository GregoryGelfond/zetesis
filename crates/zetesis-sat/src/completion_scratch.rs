//! Preflighted logical storage dimensions for joined exact completion.

use std::mem::size_of;

use zetesis_ferraris::{Interpretation, Theory};

use super::Outcome;
use crate::{Incomplete, Limits};

/// Conservative authored logical storage, independent of scheduling.
/// Values count requested typed slots, including reserved unused capacity.
/// Hash-table bucket/control overhead and allocator rounding are not counted.
/// Original shared theory and retained proposal/candidate cursor storage belong
/// to their existing admission/pending limits and are not completion scratch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompletionScratch {
    /// One exact query: encoding, alias-map entries, search and verification.
    pub query_bytes: u64,
    /// Full ordered results, accepted flags, and returned interpretation slots.
    pub result_bytes: u64,
}

impl CompletionScratch {
    /// Admit at most `workers` query workspaces plus all ordered result slots.
    /// Returns effective concurrency and peak admitted logical bytes.
    /// Zero requested workspaces is valid for a certificate-only batch.
    ///
    /// # Errors
    /// Refuses a non-fitting result table/workspace or unrepresentable arithmetic.
    pub fn admit(self, workers: usize, limit: u64) -> Result<(usize, u64), Incomplete> {
        let remaining = limit
            .checked_sub(self.result_bytes)
            .ok_or(Incomplete::CompletionScratch)?;
        if workers == 0 {
            return Ok((0, self.result_bytes));
        }
        let count = usize::try_from(remaining / self.query_bytes.max(1))
            .unwrap_or(usize::MAX)
            .min(workers);
        if count == 0 {
            return Err(Incomplete::CompletionScratch);
        }
        let bytes = self
            .query_bytes
            .checked_mul(u64::try_from(count).map_err(|_| Incomplete::CounterOverflow)?)
            .and_then(|n| n.checked_add(self.result_bytes))
            .ok_or(Incomplete::CounterOverflow)?;
        Ok((count, bytes))
    }
}

pub(super) fn requirements(
    theory: &Theory,
    limits: Limits,
    candidates: usize,
) -> Result<CompletionScratch, Incomplete> {
    let dimension = |value| u64::try_from(value).map_err(|_| Incomplete::CounterOverflow);
    counts(
        dimension(theory.atom_count())?,
        dimension(theory.nodes().len())?,
        dimension(theory.roots().len())?,
        limits,
        dimension(candidates)?,
    )
}

fn counts(
    atoms: u64,
    nodes: u64,
    roots: u64,
    limits: Limits,
    candidates: u64,
) -> Result<CompletionScratch, Incomplete> {
    // Widened fixed-coefficient sums of u64 dimensions cannot overflow u128.
    // Both host dimensions and the resulting byte counts are checked on narrowing.
    let (atoms, nodes, roots) = (u128::from(atoms), u128::from(nodes), u128::from(roots));
    let variables = (atoms + nodes).min(limits.admission.max_variables as u128);
    let clauses = (3 * nodes + roots + atoms + 1).min(limits.admission.max_clauses as u128);
    let bytes = crate::encoding::scratch_bytes(atoms, nodes, roots, clauses)
        + crate::search::scratch_bytes(variables, clauses)
        // Independent original/frozen evaluation, semantic subset construction.
        + 2 * nodes * size_of::<bool>() as u128
        + atoms * size_of::<usize>() as u128
        + atoms.div_ceil(64) * size_of::<u64>() as u128
        + size_of::<Interpretation>() as u128;
    let result = u128::from(candidates)
        * (size_of::<Option<Outcome>>() + size_of::<bool>() + size_of::<Interpretation>()) as u128;
    Ok(CompletionScratch {
        query_bytes: u64::try_from(bytes).map_err(|_| Incomplete::CounterOverflow)?,
        result_bytes: u64::try_from(result).map_err(|_| Incomplete::CounterOverflow)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_byte_overflow_is_refused_before_allocation() {
        assert_eq!(
            counts(u64::MAX, u64::MAX, u64::MAX, Limits::default(), 1),
            Err(Incomplete::CounterOverflow)
        );
        assert_eq!(
            counts(0, 0, 0, Limits::default(), u64::MAX),
            Err(Incomplete::CounterOverflow)
        );
        let limits = CompletionScratch {
            query_bytes: u64::MAX,
            result_bytes: 1,
        };
        assert_eq!(
            limits.admit(4, u64::MAX),
            Err(Incomplete::CompletionScratch)
        );
        assert_eq!(limits.admit(0, 0), Err(Incomplete::CompletionScratch));
    }
}
