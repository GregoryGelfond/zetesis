//! Bounded original-candidate proposal, injected propagation and exact completion.

use std::fmt;
use std::num::NonZeroUsize;

use super::{CompletionExecutor, StableModels, verification};
use crate::encoding;
use crate::search::{Budget, increment};
use crate::timing::{self, Phase};
use crate::{Incomplete, Solve};
use zetesis_ferraris::{Interpretation, Theory, models};

/// Verdict data supplied through the trusted batch-checker protocol. The caller
/// must supply a sound decision about the exact original candidate and its frozen
/// reduct, as specified by [`StableModels::next_batch`]. Constructing a variant
/// does not check that obligation or bind the data to a subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchVerdict {
    /// The original candidate satisfies the theory and no proper subset models its reduct.
    NoProperSubset,
    /// Exact native membership must finish this candidate.
    Residual,
    /// Original satisfaction failed, contradicting this model-only producer.
    NotModel,
}

/// Bounds on explicitly retained, not-yet-committed candidate interpretations.
#[derive(Clone, Copy, Debug)]
pub struct BatchLimits {
    /// Maximum candidates proposed for one checker call.
    pub max_candidates: NonZeroUsize,
    /// Logical payload allowance for pending/result vectors and interpretation words.
    /// Shared theory storage, allocator overhead and checker-private storage are excluded.
    pub max_pending_bytes: u64,
}

/// Proposal/check/commit counters, including unresolved work after a failure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BatchStatistics {
    /// Calls made to the injected checker, including failed calls.
    pub checker_calls: u64,
    /// Exact completion attempts entered, including scratch refusals.
    pub completion_calls: u64,
    /// Candidates with completed membership and committed results.
    pub committed: u64,
    /// Committed candidates decided by the injected no-proper-subset verdict.
    pub propagated: u64,
    /// Committed candidates completed by native exact residual search.
    pub residuals: u64,
    /// Proposed candidates whose results are still retained. A failed last exact
    /// block is a delayed terminal error and cannot establish exhaustive coverage.
    pub pending: usize,
}

/// An unresolved batch; no failure asserts exhausted candidate coverage.
#[derive(Debug)]
pub enum BatchError<E> {
    /// Native proposal, validation, completion or accounting stopped.
    Search(Incomplete),
    /// The injected checker failed; the same pending candidates remain retryable.
    Checker(E),
    /// Current-call limits cannot admit the retained batch. Its proposals remain
    /// owned and retryable under sufficient limits, without calling the checker.
    Limits(Incomplete),
    /// The checker did not return exactly one ordered verdict per pending candidate.
    Shape {
        /// Pending candidate count.
        expected: usize,
        /// Returned verdict count.
        actual: usize,
    },
}
impl<E: fmt::Display> fmt::Display for BatchError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Search(error) | Self::Limits(error) => error.fmt(f),
            Self::Checker(error) => error.fmt(f),
            Self::Shape { expected, actual } => write!(
                f,
                "candidate checker returned {actual} results for {expected} candidates"
            ),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for BatchError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Search(error) | Self::Limits(error) => Some(error),
            Self::Checker(error) => Some(error),
            Self::Shape { .. } => None,
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct State {
    pub(super) pending: Vec<Interpretation>,
    exhausted: bool,
    statistics: BatchStatistics,
}

impl StableModels {
    /// Current explicit proposal/commit accounting. Pending candidates survive
    /// checker failures and candidate-only restrictions; they are never silently dropped.
    #[must_use]
    pub fn batch_statistics(&self) -> BatchStatistics {
        BatchStatistics {
            pending: self.batch.pending.len(),
            ..self.batch.statistics
        }
    }

    /// Propose a bounded batch with the retained native cursor, check its original
    /// candidates, then return its stable members in proposal order. An empty
    /// result alone is not exhaustion; consult [`Self::exhausted`].
    ///
    /// The checker must return ordered, sound verdicts for this exact immutable
    /// theory. In particular, `NoProperSubset` must establish original satisfaction
    /// and refute every proper-subset reduct model. This is an explicit semantic
    /// trust boundary, not verification of an arbitrary supplied implementation.
    /// Every residual receives the existing exact native membership check under
    /// cumulative search limits. Original satisfaction is independently checked
    /// before proposal, and objective restrictions never enter this theory.
    ///
    /// Proposals are exactly blocked before the next is generated and remain
    /// owned here until the entire batch is classified. A later proposal/block
    /// failure is reported after this batch; failed checking returns no partial
    /// batch. Returned models remain valid across subsequent restrictions, even
    /// when they no longer meet an improved objective bound.
    ///
    /// # Errors
    /// Native failures terminate without exhaustion. Checker/shape/retry-limit failures keep
    /// pending candidates retryable. Scalar iteration with a pending batch is
    /// refused. Allocation, work and cancellation never discard pending coverage.
    pub fn next_batch<E>(
        &mut self,
        limits: BatchLimits,
        checker: impl FnOnce(&Theory, &[Interpretation]) -> Result<Vec<BatchVerdict>, E>,
    ) -> Result<Vec<Interpretation>, BatchError<E>> {
        self.next_batch_with_completion(limits, &mut CompletionExecutor::default(), checker)
    }

    /// The same proposal/check/commit contract as [`Self::next_batch`], with a
    /// reusable exact completion executor. The checker runs on the calling
    /// thread; only residual membership jobs may run concurrently.
    ///
    /// All workers share this enumeration's remaining work and decision quota.
    /// They join before any model is committed. A failed job retains the entire
    /// pending batch and terminates search; already committed older batches
    /// remain valid. Completed models preserve candidate order. Parallel
    /// scheduling can change attempted work when a limit interrupts completion.
    /// Worker timing sums are available from the executor, separately from the
    /// scalar search phase recorder.
    ///
    /// # Errors
    /// Has the same typed failures and retry rules as [`Self::next_batch`].
    pub fn next_batch_with_completion<E>(
        &mut self,
        limits: BatchLimits,
        completion: &mut CompletionExecutor,
        checker: impl FnOnce(&Theory, &[Interpretation]) -> Result<Vec<BatchVerdict>, E>,
    ) -> Result<Vec<Interpretation>, BatchError<E>> {
        if self.exhausted {
            return Ok(Vec::new());
        }
        if self.terminal {
            return Err(BatchError::Search(Incomplete::ClosedEnumerator));
        }
        let result = self.batch_step(limits, completion, checker);
        if matches!(result, Err(BatchError::Search(_))) {
            self.terminal = true;
        }
        result
    }

    fn batch_step<E>(
        &mut self,
        limits: BatchLimits,
        completion: &mut CompletionExecutor,
        checker: impl FnOnce(&Theory, &[Interpretation]) -> Result<Vec<BatchVerdict>, E>,
    ) -> Result<Vec<Interpretation>, BatchError<E>> {
        self.control
            .poll()
            .map_err(|e| BatchError::Search(e.into()))?;
        if self.batch.pending.is_empty() {
            if let Some(error) = self.pending_error.take() {
                return Err(BatchError::Search(error));
            }
            self.propose(limits).map_err(BatchError::Search)?;
        }
        if self.batch.pending.is_empty() {
            if let Some(error) = self.pending_error.take() {
                return Err(BatchError::Search(error));
            }
            self.exhausted = self.batch.exhausted;
            self.terminal = self.exhausted;
            return Ok(Vec::new());
        }
        if self.batch.pending.len() > limits.max_candidates.get() {
            return Err(BatchError::Limits(Incomplete::BatchCandidateLimit));
        }
        // Retained vector capacity can exceed the number of rows in a partial
        // batch. Do not silently waive its payload on a smaller retry request.
        if self.pending_row_bytes() * self.batch.pending.capacity() as u128
            > u128::from(limits.max_pending_bytes)
        {
            return Err(BatchError::Limits(Incomplete::PendingBytes));
        }
        increment(&mut self.batch.statistics.checker_calls).map_err(BatchError::Search)?;
        let mut verdicts =
            checker(&self.theory, &self.batch.pending).map_err(BatchError::Checker)?;
        if verdicts.len() != self.batch.pending.len() {
            return Err(BatchError::Shape {
                expected: self.batch.pending.len(),
                actual: verdicts.len(),
            });
        }
        self.certify_pending(&mut verdicts)
            .map_err(BatchError::Search)?;
        self.commit(&verdicts, completion).map_err(|error| {
            if error == Incomplete::CompletionScratch {
                BatchError::Limits(error)
            } else {
                BatchError::Search(error)
            }
        })
    }

    fn certify_pending(&mut self, verdicts: &mut [BatchVerdict]) -> Result<(), Incomplete> {
        let Some(certificate) = &self.certification else {
            return Ok(());
        };
        let mut search = self.statistics.search;
        let result = (|| {
            for (candidate, verdict) in self.batch.pending.iter().zip(verdicts) {
                if *verdict != BatchVerdict::Residual {
                    continue;
                }
                *verdict = match super::certified::classify(
                    certificate,
                    candidate,
                    self.limits,
                    &self.control,
                    &mut self.statistics,
                    &mut search,
                )? {
                    zetesis_ferraris::TightVerdict::Stable => BatchVerdict::NoProperSubset,
                    zetesis_ferraris::TightVerdict::NotModel { .. } => BatchVerdict::NotModel,
                    zetesis_ferraris::TightVerdict::Residual { .. } => BatchVerdict::Residual,
                };
            }
            Ok(())
        })();
        self.statistics.search = search;
        result
    }

    fn propose(&mut self, limits: BatchLimits) -> Result<(), Incomplete> {
        if self.batch.exhausted {
            return Ok(());
        }
        if self.pending_row_bytes() * limits.max_candidates.get() as u128
            > u128::from(limits.max_pending_bytes)
        {
            return Err(Incomplete::PendingBytes);
        }
        self.batch
            .pending
            .try_reserve_exact(limits.max_candidates.get())
            .map_err(|_| Incomplete::Allocation)?;
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: self.limits.search,
            control: &self.control,
            statistics: self.statistics.search,
        };
        while self.batch.pending.len() < limits.max_candidates.get() {
            match proposal(
                &self.theory,
                &mut self.candidate_cnf,
                &mut self.candidate_cursor,
                self.limits,
                &mut budget,
                &mut self.statistics,
            ) {
                Ok(Some(candidate)) => {
                    // The interpretation is retained even if its exact block cannot fit.
                    let started = timing::start(self.statistics.phase_timings.as_ref());
                    let block = self.candidate_cursor.exclude(
                        &mut self.candidate_cnf,
                        &candidate,
                        &mut budget,
                    );
                    timing::finish(
                        &mut self.statistics.phase_timings,
                        Phase::Candidates,
                        started,
                    );
                    self.batch.pending.push(candidate);
                    if let Err(error) = block {
                        self.pending_error = Some(error);
                        break;
                    }
                }
                Ok(None) => {
                    self.batch.exhausted = true;
                    break;
                }
                Err(error) => {
                    self.pending_error = Some(error);
                    break;
                }
            }
        }
        self.statistics.search = budget.statistics;
        Ok(())
    }

    fn pending_row_bytes(&self) -> u128 {
        (std::mem::size_of::<Interpretation>() as u128) * 2
            + (self.theory.atom_count().div_ceil(64) as u128) * 8
            + std::mem::size_of::<BatchVerdict>() as u128
            + 1
    }

    fn commit(
        &mut self,
        verdicts: &[BatchVerdict],
        completion: &mut CompletionExecutor,
    ) -> Result<Vec<Interpretation>, Incomplete> {
        let mut accepted = Vec::new();
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: self.limits.search,
            control: &self.control,
            statistics: self.statistics.search,
        };
        increment(&mut self.batch.statistics.completion_calls)?;
        let result = completion.complete(
            super::completion::Input {
                theory: &self.theory,
                candidates: &self.batch.pending,
                verdicts,
                limits: self.limits,
            },
            &mut budget,
            &mut self.statistics,
            &mut accepted,
        );
        self.statistics.search = budget.statistics;
        result?;
        // The executor preflight included these output slots before any query.
        let mut output = crate::search::storage(verdicts.len())?;
        let mut stats = self.batch.statistics;
        let mut stable = self.statistics.stable_models;
        for (verdict, accepted) in verdicts.iter().zip(&accepted) {
            increment(&mut stats.committed)?;
            increment(if *verdict == BatchVerdict::Residual {
                &mut stats.residuals
            } else {
                &mut stats.propagated
            })?;
            if *accepted {
                increment(&mut stable)?;
            }
        }
        for (candidate, accepted) in std::mem::take(&mut self.batch.pending)
            .into_iter()
            .zip(accepted)
        {
            if accepted {
                output.push(candidate);
            }
        }
        self.batch.statistics = stats;
        self.statistics.stable_models = stable;
        Ok(output)
    }
}

fn proposal(
    theory: &Theory,
    cnf: &mut crate::Cnf,
    cursor: &mut crate::search::Cursor,
    limits: super::Limits,
    budget: &mut Budget<'_>,
    statistics: &mut super::Statistics,
) -> Result<Option<Interpretation>, Incomplete> {
    let started = timing::start(statistics.phase_timings.as_ref());
    let proposal = (|| {
        increment(&mut statistics.candidate_queries)?;
        let assignment = match cursor.query(cnf, budget) {
            Solve::Sat(assignment) => assignment,
            Solve::Unsat => return Ok(None),
            Solve::Inconclusive(error) => return Err(error),
        };
        if statistics.candidates >= limits.max_candidates {
            return Err(Incomplete::CandidateLimit);
        }
        encoding::interpretation(theory, &assignment, budget).map(Some)
    })();
    timing::finish(&mut statistics.phase_timings, Phase::Candidates, started);
    let Some(candidate) = proposal? else {
        return Ok(None);
    };
    let started = timing::start(statistics.phase_timings.as_ref());
    let original = models(theory, &candidate, verification(limits), budget.control);
    timing::finish(
        &mut statistics.phase_timings,
        Phase::OriginalValidation,
        started,
    );
    if !original? {
        return Err(Incomplete::InvalidWitness);
    }
    increment(&mut statistics.candidates)?;
    Ok(Some(candidate))
}
