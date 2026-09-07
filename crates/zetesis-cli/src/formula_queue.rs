//! Hardware-independent proposal/check/commit results awaiting scoring or output.

use std::collections::VecDeque;

use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, Theory};
use zetesis_sat::{BatchError, BatchLimits, BatchVerdict, CompletionExecutor, StableModels};

use crate::formula_execution::Failure;
use crate::{Options, RunError};

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
    pub(crate) fn new(options: &Options) -> Result<Self, RunError> {
        Ok(Self {
            completion: CompletionExecutor::with_scratch_limit(
                options.completion_workers,
                options.max_completion_scratch_bytes,
            )
            .map_err(RunError::CompletionPool)?,
            accounting: crate::CompletionAccounting {
                requested_workers: options.completion_workers.get(),
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
        options: &Options,
        control: &Control,
        mut checker: impl FnMut(&Theory, &[Interpretation]) -> Result<Vec<BatchVerdict>, Failure>,
    ) -> Option<Result<Interpretation, Failure>> {
        loop {
            if let Err(error) = control.poll() {
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
            // A checker failure never enters completion. Do not count a previous
            // attempt's last statistics again when proposals remain pending.
            let mut entered_completion = false;
            let result = models.next_batch_with_completion(
                limits,
                &mut self.completion,
                |theory, candidates| {
                    let verdicts = checker(theory, candidates)?;
                    entered_completion = verdicts.len() == candidates.len();
                    Ok(verdicts)
                },
            );
            if entered_completion && let Some(progress) = self.completion.last_statistics() {
                self.accounting.record(progress);
            }
            match result {
                Ok(batch) => self.ready = batch.into(),
                Err(BatchError::Search(error) | BatchError::Limits(error)) => {
                    return Some(Err(Failure::Search(error)));
                }
                Err(BatchError::Checker(error)) => return Some(Err(error)),
                Err(BatchError::Shape { expected, actual }) => {
                    return Some(Err(Failure::Run(RunError::FormulaBatchShape {
                        expected,
                        actual,
                    })));
                }
            }
        }
    }
}
