//! The boundary between a bounded batch of candidates and the membership route
//! that checks it: the CPU batched completion and the GPU formula routes.
//!
//! A route receives the candidates with their original theory and returns one
//! verdict per candidate, and the host accepts the verdicts only for that exact
//! request. Each route decides membership by its own reduct check; a candidate
//! it leaves undecided returns to the host's exact completion.

use std::{error::Error, fmt};

use zetesis_ferraris::{Interpretation, Theory};
use zetesis_sat::BatchVerdict;

/// Original candidates borrowed for one membership check, in proposal order.
///
/// Each interpretation belongs to `theory()`, and fixes its reduct. A joint
/// route must establish original satisfaction as well as its reduct verdict;
/// the independent route has already checked original satisfaction on the host.
pub(crate) struct CandidateBatch<'a> {
    theory: &'a Theory,
    candidates: &'a [Interpretation],
}

impl<'a> CandidateBatch<'a> {
    pub(crate) const fn new(theory: &'a Theory, candidates: &'a [Interpretation]) -> Self {
        Self { theory, candidates }
    }

    /// Exact original subject of every candidate, which the GPU routes and the
    /// queue's native test route load.
    #[cfg(any(test, feature = "gpu"))]
    pub(crate) const fn theory(&self) -> &'a Theory {
        self.theory
    }

    /// Candidates in proposal order.
    pub(crate) const fn candidates(&self) -> &'a [Interpretation] {
        self.candidates
    }

    /// Associate one ordered verdict with each candidate in this exact request.
    ///
    /// This checks the count and retains the request's identity; the verdicts
    /// themselves are the route's reduct decisions.
    ///
    /// # Errors
    /// Refuses any result count different from the request's candidate count.
    pub(crate) fn finish(
        self,
        verdicts: Vec<BatchVerdict>,
    ) -> Result<BatchResult<'a>, ExecutorError> {
        if verdicts.len() != self.candidates.len() {
            return Err(ExecutorError::Shape {
                expected: self.candidates.len(),
                actual: verdicts.len(),
            });
        }
        Ok(BatchResult {
            batch: self,
            verdicts,
            #[cfg(feature = "gpu")]
            prefix_limit: None,
        })
    }
}

/// Ordered verdicts associated with the request that produced them.
pub(crate) struct BatchResult<'a> {
    batch: CandidateBatch<'a>,
    verdicts: Vec<BatchVerdict>,
    #[cfg(feature = "gpu")]
    prefix_limit: Option<std::num::NonZeroUsize>,
}

impl BatchResult<'_> {
    #[cfg(feature = "gpu")]
    pub(crate) const fn prefix_limit(&self) -> Option<std::num::NonZeroUsize> {
        self.prefix_limit
    }

    pub(crate) fn into_verdicts(
        self,
        theory: &Theory,
        candidates: &[Interpretation],
    ) -> Result<Vec<BatchVerdict>, ExecutorError> {
        if !self.batch.theory.same_instance(theory)
            || !std::ptr::eq(self.batch.candidates, candidates)
        {
            return Err(ExecutorError::ForeignBatch);
        }
        Ok(self.verdicts)
    }
}

/// Device verdicts plus a capacity observation, not additional semantic evidence.
#[cfg(feature = "gpu")]
pub(crate) struct DeviceVerdicts {
    pub(crate) values: Vec<BatchVerdict>,
    prefix_limit: Option<std::num::NonZeroUsize>,
}

#[cfg(feature = "gpu")]
impl DeviceVerdicts {
    pub(crate) fn new(count: usize) -> Result<Self, zetesis_sat::Incomplete> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| zetesis_sat::Incomplete::Allocation)?;
        Ok(Self {
            values,
            prefix_limit: None,
        })
    }

    /// Only a shortened nonempty prefix establishes a physical capacity limit;
    /// the final remainder alone carries no such information.
    pub(crate) fn record_prefix(&mut self, offered: usize, checked: usize) {
        if checked < offered
            && let Some(count) = std::num::NonZeroUsize::new(checked)
        {
            self.prefix_limit = Some(self.prefix_limit.map_or(count, |prior| prior.min(count)));
        }
    }

    pub(crate) fn finish(
        self,
        batch: CandidateBatch<'_>,
    ) -> Result<BatchResult<'_>, ExecutorError> {
        let mut result = batch.finish(self.values)?;
        result.prefix_limit = self.prefix_limit;
        Ok(result)
    }
}

/// A protocol failure at the batch boundary between the host and a membership
/// route. Either is an internal invariant failure, never a membership verdict.
#[derive(Debug)]
pub enum ExecutorError {
    /// A receipt belongs to a different exact subject or pending batch.
    ForeignBatch,
    /// The route returned a different number of verdicts than candidates.
    Shape {
        /// Original candidate count.
        expected: usize,
        /// Returned verdict count.
        actual: usize,
    },
}

impl fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignBatch => {
                f.write_str("a membership receipt belongs to another candidate batch")
            }
            Self::Shape { expected, actual } => write!(
                f,
                "a membership route returned {actual} verdicts for {expected} candidates"
            ),
        }
    }
}

impl Error for ExecutorError {}

#[cfg(test)]
mod tests {
    use super::{CandidateBatch, ExecutorError};
    use zetesis_ferraris::{AdmissionLimits, Interpretation, Theory};
    use zetesis_sat::BatchVerdict;

    fn theory() -> Theory {
        Theory::new(
            1,
            zetesis_ferraris::FormulaParts::default(),
            Vec::new(),
            AdmissionLimits::default(),
        )
        .unwrap()
    }

    #[test]
    fn equal_foreign_theory_cannot_supply_a_receipt() {
        let original = theory();
        let foreign = theory();
        let candidates = [Interpretation::new(&original, []).unwrap()];
        let receipt = CandidateBatch::new(&foreign, &candidates)
            .finish(vec![BatchVerdict::Residual])
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(matches!(
            receipt.into_verdicts(&original, &candidates),
            Err(ExecutorError::ForeignBatch)
        ));
    }

    #[test]
    fn equal_candidate_copies_are_not_the_pending_batch() {
        let original = theory();
        let candidates = [Interpretation::new(&original, []).unwrap()];
        let copied = candidates.clone();
        let receipt = CandidateBatch::new(&original, &copied)
            .finish(vec![BatchVerdict::Residual])
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(matches!(
            receipt.into_verdicts(&original, &candidates),
            Err(ExecutorError::ForeignBatch)
        ));
    }

    #[test]
    fn exact_batch_receipt_preserves_ordered_verdicts() {
        let original = theory();
        let candidates = [
            Interpretation::new(&original, []).unwrap(),
            Interpretation::new(&original, [0]).unwrap(),
        ];
        let verdicts = vec![BatchVerdict::NoProperSubset, BatchVerdict::Residual];
        let receipt = CandidateBatch::new(&original, &candidates)
            .finish(verdicts.clone())
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            receipt.into_verdicts(&original, &candidates).unwrap(),
            verdicts
        );
    }
}
