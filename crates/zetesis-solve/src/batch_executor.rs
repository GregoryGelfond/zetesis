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

/// Original models borrowed for one membership check, in proposal order.
///
/// The host already checked original satisfaction. Each interpretation belongs
/// to `theory()`, and remains the candidate fixing its reduct.
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
        })
    }
}

/// Ordered verdicts associated with the request that produced them.
pub(crate) struct BatchResult<'a> {
    batch: CandidateBatch<'a>,
    verdicts: Vec<BatchVerdict>,
}

impl BatchResult<'_> {
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
        Theory::new(1, Vec::new(), Vec::new(), AdmissionLimits::default()).unwrap()
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
