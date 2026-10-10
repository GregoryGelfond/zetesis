//! Bounded original-candidate proposal, injected propagation and exact completion.

use std::fmt;
use std::num::NonZeroUsize;
use std::sync::Arc;

use super::{CompletionExecutor, IndexedTheory, StableModels, verification};
use crate::Incomplete;
use crate::search::{Budget, increment};
use crate::timing::{self, Phase};
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
    /// The candidate is not an answer set and needs no query. This requires
    /// sound nonminimality evidence: for example a checked proper-subset reduct
    /// model, or a present atom lacking a true producer under a complete tight
    /// plan, whose removal models the reduct by the support law.
    Refuted,
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
            let _ = self.proposer.finish_production();
        }
        result
    }

    fn batch_step<E>(
        &mut self,
        limits: BatchLimits,
        completion: &mut CompletionExecutor,
        checker: impl FnOnce(&Theory, &[Interpretation]) -> Result<Vec<BatchVerdict>, E>,
    ) -> Result<Vec<Interpretation>, BatchError<E>> {
        self.cancellation
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
        let Some(certificate) = self
            .certificate
            .as_ref()
            .and_then(super::certified::Certificate::cpu)
        else {
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
                    &mut self.certificate_workspace,
                    candidate,
                    self.limits,
                    &self.cancellation,
                    &mut self.statistics,
                    &mut search,
                )? {
                    super::certified::Verdict::Stable => BatchVerdict::NoProperSubset,
                    super::certified::Verdict::NotModel => BatchVerdict::NotModel,
                    super::certified::Verdict::Unsupported
                    | super::certified::Verdict::NonMinimal => BatchVerdict::Refuted,
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
        let index = self.walk_index()?;
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: self.limits.search,
            cancellation: &self.cancellation,
            statistics: self.statistics.search,
        };
        if self.determined_candidates.is_none()
            && let Some(index) = &index
            && let super::Proposer::Proposals(proposals) = &mut self.proposer
        {
            let subject = match index.subject(&self.theory) {
                Ok(subject) => subject,
                Err(error) => {
                    self.statistics.search = budget.statistics;
                    return Err(error);
                }
            };
            let started = timing::start(self.statistics.phase_timings.as_ref());
            let produced = proposals.fill(
                subject,
                limits.max_candidates.get(),
                self.limits
                    .max_candidates
                    .saturating_sub(self.statistics.candidates),
                &mut budget,
                &mut self.batch.pending,
                self.statistics.phase_timings.is_some(),
            );
            if let Some(timings) = self.statistics.phase_timings.as_mut() {
                timings
                    .original_validation
                    .merge(produced.original_validation);
            }
            timing::finish(
                &mut self.statistics.phase_timings,
                Phase::Candidates,
                started,
            );
            self.statistics.search = budget.statistics;
            self.admit_produced(&produced);
            return Ok(());
        }
        while self.batch.pending.len() < limits.max_candidates.get() {
            let walk = match self.determined_candidates.as_mut() {
                Some(candidates) => super::Walk::Determined(candidates),
                None => super::Walk::Index(index.as_ref()),
            };
            match proposal(
                &self.theory,
                &mut self.proposer,
                walk,
                self.limits,
                &mut budget,
                &mut self.statistics,
            ) {
                Ok(Some(candidate)) => {
                    // The interpretation is retained even if its exact block cannot fit.
                    let started = timing::start(self.statistics.phase_timings.as_ref());
                    let block = self.proposer.exclude(&candidate, &mut budget);
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

    /// The original index the batch's walk reads, built at the walk's one
    /// build site, as in scalar iteration; the positive cursor walks no
    /// region and builds no index. A refused build charge stops the batch.
    fn walk_index(&mut self) -> Result<Option<Arc<IndexedTheory>>, Incomplete> {
        if self.determined_candidates.is_some() {
            return Ok(None);
        }
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: self.limits.search,
            cancellation: &self.cancellation,
            statistics: self.statistics.search,
        };
        let index = super::walk_index(
            &mut self.index,
            &mut self.proposer,
            &mut budget,
            &mut self.statistics.phase_timings,
        )
        .map(|index| index.map(Arc::clone));
        self.statistics.search = budget.statistics;
        index
    }

    /// Admit a joined production round's pending candidates. No producer
    /// decides membership. Every completed classical leaf crosses the same
    /// independent original-satisfaction boundary as scalar proposals before
    /// the external batch checker receives it.
    fn admit_produced(&mut self, produced: &super::region_proposals::Produced) {
        self.batch.exhausted = produced.exhausted;
        self.pending_error = produced.stopped;
        for (validated, candidate) in self.batch.pending.iter().enumerate() {
            let result = validate_proposal(
                &self.theory,
                candidate,
                self.limits,
                &self.cancellation,
                &mut self.statistics,
            )
            .and_then(|()| increment(&mut self.statistics.candidates));
            if let Err(error) = result {
                // Only the validated prefix crossed the proposal boundary.
                // The stopped suffix establishes no coverage, even when
                // production had reached the end of its region frontier.
                self.batch.pending.truncate(validated);
                self.batch.exhausted = false;
                self.pending_error = Some(error);
                break;
            }
        }
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
        // Completion owns every resident query workspace in this attempt.
        // Scalar scratch carries no semantic state and must not remain as an
        // additional unaccounted workspace when switching execution modes.
        self.reduct.workspace = crate::ReductWorkspace::default();
        let mut accepted = Vec::new();
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: self.limits.search,
            cancellation: &self.cancellation,
            statistics: self.statistics.search,
        };
        increment(&mut self.batch.statistics.completion_calls)?;
        let result = completion.complete(
            super::completion::Input {
                theory: &self.theory,
                candidates: &self.batch.pending,
                verdicts,
                limits: self.limits,
                prepared: None,
                // A regions residual exists only after a walk, which built
                // the index; completion borrows it and never builds one.
                query: self.index.get().map(|index| super::ReductQuery::new(index)),
            },
            &mut budget,
            &mut self.statistics,
            &mut accepted,
            &mut self.reduct,
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
    proposer: &mut super::Proposer,
    walk: super::Walk<'_>,
    limits: super::Limits,
    budget: &mut Budget<'_>,
    statistics: &mut super::Statistics,
) -> Result<Option<Interpretation>, Incomplete> {
    let started = timing::start(statistics.phase_timings.as_ref());
    let proposal = proposer.propose(theory, walk, limits, None, budget, statistics);
    timing::finish(&mut statistics.phase_timings, Phase::Candidates, started);
    // The batched protocol checks its proposals itself; a worker-decided
    // model is refused here rather than checked twice.
    let candidate = match proposal? {
        None => return Ok(None),
        Some(super::Proposal::Candidate(candidate)) => candidate,
        Some(super::Proposal::Stable(_)) => return Err(Incomplete::InvalidWitness),
    };
    // Proposals cross to an external checker, so they are validated here
    // under their own limit and phase before that boundary, whatever the
    // proposer's construction promises.
    validate_proposal(theory, &candidate, limits, budget.cancellation, statistics)?;
    increment(&mut statistics.candidates)?;
    Ok(Some(candidate))
}

fn validate_proposal(
    theory: &Theory,
    candidate: &Interpretation,
    limits: super::Limits,
    cancellation: &crate::Cancellation,
    statistics: &mut super::Statistics,
) -> Result<(), Incomplete> {
    let started = timing::start(statistics.phase_timings.as_ref());
    let original = models(theory, candidate, verification(limits), cancellation);
    timing::finish(
        &mut statistics.phase_timings,
        Phase::OriginalValidation,
        started,
    );
    if !original? {
        return Err(Incomplete::InvalidWitness);
    }
    Ok(())
}
