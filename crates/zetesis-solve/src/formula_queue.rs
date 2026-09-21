//! Hardware-independent proposal/check/commit results awaiting scoring or output.

use std::collections::VecDeque;
use std::num::NonZeroUsize;

use zetesis_cpu::Cancellation;
use zetesis_ferraris::Interpretation;
use zetesis_sat::{BatchError, BatchLimits, CompletionExecutor, StableModels};

use crate::formula_execution::Failure;
use crate::{BatchResult, CandidateBatch, SolveConfig, SolveError};

#[derive(Default)]
/// Owns only reduct-verified models awaiting objective scoring or output.
/// `StableModels` separately owns unverified proposed candidates and their exact
/// blocks. This boundary keeps device residency outside membership accounting.
pub(crate) struct BatchQueue {
    ready: VecDeque<Interpretation>,
    completion: CompletionExecutor,
    accounting: crate::formula_execution::CompletionAccounting,
}

impl BatchQueue {
    pub(crate) fn new(options: &SolveConfig) -> Result<Self, SolveError> {
        Self::with_completion_workers(options, options.completion_workers)
    }

    /// A complete oracle leaves no unresolved query for residual workers.
    /// Scalar completion still admits result storage, validates original
    /// satisfaction and commits the checked batch under the same limits.
    #[cfg(feature = "gpu")]
    pub(crate) fn for_complete_oracle(options: &SolveConfig) -> Result<Self, SolveError> {
        Self::with_completion_workers(options, NonZeroUsize::MIN)
    }

    fn with_completion_workers(
        options: &SolveConfig,
        workers: NonZeroUsize,
    ) -> Result<Self, SolveError> {
        Ok(Self {
            completion: CompletionExecutor::with_scratch_limit(
                workers,
                options.max_completion_scratch_bytes,
            )
            .map_err(SolveError::CompletionPool)?,
            accounting: crate::CompletionAccounting {
                requested_workers: workers.get(),
                ..Default::default()
            },
            ..Self::default()
        })
    }

    pub(crate) const fn accounting(&self) -> crate::formula_execution::CompletionAccounting {
        self.accounting
    }

    pub(crate) fn len(&self) -> usize {
        self.ready.len()
    }

    /// The checker sees original candidates, never models already returned by
    /// the stable iterator. A restriction cannot erase these verified queued
    /// interpretations; they must still be scored or explicitly left unreported.
    pub(crate) fn next(
        &mut self,
        models: &mut StableModels,
        options: &SolveConfig,
        cancellation: &Cancellation,
        mut checker: impl for<'a> FnMut(CandidateBatch<'a>) -> Result<BatchResult<'a>, Failure>,
    ) -> Option<Result<Interpretation, Failure>> {
        loop {
            if let Err(error) = cancellation.poll() {
                return Some(Err(Failure::Search(error.into())));
            }
            if let Some(model) = self.ready.pop_front() {
                return Some(Ok(model));
            }
            if models.exhausted() {
                return None;
            }
            let limits = BatchLimits {
                max_candidates: options.batch_size,
                max_pending_bytes: options.max_batch_bytes,
            };
            // Membership certification can stop after the injected checker.
            // The stream's explicit completion-attempt count prevents counting
            // a previous executor snapshot again when proposals remain pending.
            let prior_completion = models.batch_statistics().completion_calls;
            let result = models.next_batch_with_completion(
                limits,
                &mut self.completion,
                |theory, candidates| {
                    checker(CandidateBatch::new(theory, candidates))?
                        .into_verdicts(theory, candidates)
                        .map_err(Failure::from)
                },
            );
            if models.batch_statistics().completion_calls != prior_completion
                && let Some(progress) = self.completion.last_statistics()
            {
                self.accounting.record(progress);
            }
            match result {
                Ok(batch) => self.ready = batch.into(),
                Err(BatchError::Search(error) | BatchError::Limits(error)) => {
                    return Some(Err(Failure::Search(error)));
                }
                Err(BatchError::Checker(error)) => return Some(Err(error)),
                Err(BatchError::Shape { expected, actual }) => {
                    return Some(Err(Failure::Run(SolveError::FormulaBatchShape {
                        expected,
                        actual,
                    })));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use super::BatchQueue;
    use crate::SolveConfig;

    fn options() -> SolveConfig {
        SolveConfig {
            completion_workers: NonZeroUsize::new(4).unwrap(),
            max_completion_scratch_bytes: 12_345,
            ..SolveConfig::DEFAULT
        }
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn complete_oracle_uses_scalar_completion() {
        let options = options();
        let queue = BatchQueue::for_complete_oracle(&options).unwrap();
        assert_eq!(queue.completion.workers(), 1);
        assert_eq!(queue.accounting().requested_workers, 1);
        assert_eq!(queue.accounting().effective_workers, 0);
        assert_eq!(
            queue.completion.scratch_limit(),
            options.max_completion_scratch_bytes
        );
    }

    #[test]
    fn residual_queue_retains_requested_workers() {
        let options = options();
        let queue = BatchQueue::new(&options).unwrap();
        assert_eq!(queue.completion.workers(), options.completion_workers.get());
        assert_eq!(
            queue.accounting().requested_workers,
            options.completion_workers.get()
        );
        assert_eq!(
            queue.completion.scratch_limit(),
            options.max_completion_scratch_bytes
        );
    }
}
