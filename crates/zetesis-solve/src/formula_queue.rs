//! Hardware-independent proposal/check/commit results awaiting scoring or output.

use std::collections::VecDeque;
use std::num::NonZeroUsize;

use zetesis_cpu::Cancellation;
use zetesis_ferraris::Interpretation;
use zetesis_sat::{BatchError, BatchLimits, CompletionExecutor, StableModels};

use crate::batch_executor::{BatchResult, CandidateBatch};
use crate::formula_execution::Failure;
use crate::{SolveConfig, SolveError};

#[cfg(feature = "gpu")]
mod batching;

#[derive(Default)]
/// Owns only reduct-verified models awaiting objective scoring or output.
/// `StableModels` separately owns unverified proposed candidates and their exact
/// blocks. This boundary keeps device residency outside membership accounting.
pub(crate) struct BatchQueue {
    ready: VecDeque<Interpretation>,
    completion: CompletionExecutor,
    accounting: crate::formula_execution::CompletionAccounting,
    original_check: OriginalCheck,
    #[cfg(feature = "gpu")]
    batching: batching::Batching,
}

#[derive(Default)]
enum OriginalCheck {
    #[default]
    Independent,
    #[cfg(feature = "gpu")]
    Joint,
}

impl BatchQueue {
    pub(crate) fn new(options: &SolveConfig) -> Result<Self, SolveError> {
        Self::with_completion_workers(options, options.completion_workers)
    }

    /// A complete oracle leaves no unresolved query for residual workers.
    /// Scalar completion still admits result storage and commits the checked
    /// batch under the same limits. The oracle establishes original satisfaction.
    #[cfg(feature = "gpu")]
    pub(crate) fn for_complete_oracle(options: &SolveConfig) -> Result<Self, SolveError> {
        Self::joint(options, NonZeroUsize::MIN)
    }

    #[cfg(feature = "gpu")]
    pub(crate) fn for_joint_oracle(options: &SolveConfig) -> Result<Self, SolveError> {
        Self::joint(options, options.completion_workers)
    }

    #[cfg(feature = "gpu")]
    fn joint(options: &SolveConfig, workers: NonZeroUsize) -> Result<Self, SolveError> {
        let mut queue = Self::with_completion_workers(options, workers)?;
        queue.original_check = OriginalCheck::Joint;
        queue.batching = batching::Batching::new(options);
        Ok(queue)
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
            #[cfg(feature = "gpu")]
            let max_candidates = self.batching.target(models, options);
            #[cfg(not(feature = "gpu"))]
            let max_candidates = options.batch_size;
            let limits = BatchLimits {
                max_candidates,
                max_pending_bytes: options.max_batch_bytes,
            };
            // Membership certification can stop after the injected checker.
            // The stream's explicit completion-attempt count prevents counting
            // a previous executor snapshot again when proposals remain pending.
            let prior_completion = models.batch_statistics().completion_calls;
            #[cfg(feature = "gpu")]
            let mut accepted_prefix = (0, None);
            let check = |theory: &zetesis_ferraris::Theory, candidates: &[Interpretation]| {
                let result = checker(CandidateBatch::new(theory, candidates))?;
                #[cfg(feature = "gpu")]
                let prefix_limit = result.prefix_limit();
                let verdicts = result
                    .into_verdicts(theory, candidates)
                    .map_err(Failure::from)?;
                #[cfg(feature = "gpu")]
                {
                    accepted_prefix = (candidates.len(), prefix_limit);
                }
                Ok(verdicts)
            };
            let result = match self.original_check {
                OriginalCheck::Independent => {
                    models.next_batch_with_completion(limits, &mut self.completion, check)
                }
                #[cfg(feature = "gpu")]
                OriginalCheck::Joint => {
                    models.next_batch_with_joint_completion(limits, &mut self.completion, check)
                }
            };
            if models.batch_statistics().completion_calls != prior_completion
                && let Some(progress) = self.completion.last_statistics()
            {
                self.accounting.record(progress);
            }
            match result {
                Ok(batch) => {
                    #[cfg(feature = "gpu")]
                    self.batching.committed(
                        limits.max_candidates,
                        accepted_prefix.0,
                        accepted_prefix.1,
                    );
                    self.ready = batch.into();
                }
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

    #[cfg(feature = "gpu")]
    #[test]
    fn failed_later_prefix_keeps_the_whole_batch_pending() {
        use crate::batch_executor::CandidateBatch;
        use crate::formula_execution::{Failure, FormulaExecutionStatistics, record_submission};
        use zetesis_sat::{BatchVerdict, Incomplete, Limits, StableModels};

        let admitted = zetesis_reference_support::formula("{a;b;c}.");
        let cancellation = zetesis_cpu::Cancellation::default();
        let mut models = StableModels::new(
            admitted.theory(),
            Limits {
                max_verification_work: 0,
                ..Limits::default()
            },
            cancellation.clone(),
        )
        .unwrap();
        let options = SolveConfig {
            batch_size: NonZeroUsize::new(3).unwrap(),
            ..Default::default()
        };
        let mut queue = BatchQueue::for_joint_oracle(&options).unwrap();
        let mut statistics = FormulaExecutionStatistics::default();
        let result = queue.next(&mut models, &options, &cancellation, |batch| {
            assert_eq!(batch.candidates().len(), 3);
            let prefix = &batch.candidates()[..1];
            let checked = zetesis_ferraris::check(
                batch.theory(),
                &prefix[0],
                zetesis_ferraris::Limits::default(),
                &cancellation,
            )
            .unwrap();
            assert!(checked.accepted());
            let receipt = CandidateBatch::new(batch.theory(), prefix)
                .finish(vec![BatchVerdict::NoProperSubset])?;
            let verdicts = receipt.into_verdicts(batch.theory(), prefix)?;
            assert_eq!(verdicts, [BatchVerdict::NoProperSubset]);
            // Inject completed-prefix observations at the host boundary. This
            // tests retained bookkeeping and publication, not device execution.
            record_submission(&mut statistics, Some(prefix.len()))?;
            statistics.gpu_batches += 1;
            statistics.gpu_candidates += u64::try_from(prefix.len()).unwrap();
            Err(Failure::Search(Incomplete::Allocation))
        });
        assert!(matches!(
            result,
            Some(Err(Failure::Search(Incomplete::Allocation)))
        ));
        assert_eq!(statistics.gpu_submitted_batches, 1);
        assert_eq!(statistics.gpu_submitted_candidates, 1);
        assert_eq!(statistics.gpu_batches, 1);
        assert_eq!(statistics.gpu_candidates, 1);
        assert_eq!(models.batch_statistics().pending, 3);
        assert_eq!(models.batch_statistics().committed, 0);
        assert_eq!(models.batch_statistics().original_validation_work, 0);
        assert_eq!(models.statistics().candidates, 3);
        assert_eq!(models.statistics().stable_models, 0);
        assert_eq!(queue.len(), 0);
        assert!(!models.exhausted());
    }
}
