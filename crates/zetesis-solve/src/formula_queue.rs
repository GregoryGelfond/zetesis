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
#[cfg(feature = "gpu")]
pub(crate) mod device;
#[cfg(all(test, feature = "gpu"))]
mod lookahead_tests;

/// The answers a session still requests, which decide whether a device
/// queue proposes the next slate while the current one is checked.
#[cfg(feature = "gpu")]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Demand {
    /// The session enumerates answer sets rather than selecting optimal ones.
    pub(crate) enumerates: bool,
    /// The answers still owed, or `None` when the request is unbounded.
    pub(crate) owed: Option<usize>,
}

/// What a device queue's slates proposed ahead did.
#[cfg(feature = "gpu")]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct LookaheadReceipts {
    /// Slates proposed while their predecessor's last submission was checked.
    pub(crate) slates: u64,
    /// The last decision, once a device slate was handed out.
    pub(crate) decision: Option<crate::GpuLookahead>,
    /// Measured device compute of the overlapped submissions.
    pub(crate) device_ns: u64,
    /// Finish waits on the overlapped submissions, after their production.
    pub(crate) finish_wait_ns: u64,
}

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
    #[cfg(feature = "gpu")]
    demand: Demand,
    #[cfg(feature = "gpu")]
    lookahead: LookaheadReceipts,
    /// Whether the last decoded device submission was checked while the host
    /// proposed the next slate. The device reports each submission's compute
    /// interval with the next submission, or at the settlement's flush.
    #[cfg(feature = "gpu")]
    previous_overlapped: bool,
    /// When this queue was built, and when it released its first answer.
    #[cfg(feature = "gpu")]
    created: Option<Instant>,
    #[cfg(feature = "gpu")]
    first_answer: Option<std::time::Duration>,
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
        queue.created = Some(Instant::now());
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

    /// Candidate slates this GPU formula queue committed with at least one
    /// candidate.
    #[cfg(feature = "gpu")]
    pub(crate) const fn slates(&self) -> u64 {
        self.batching.slates()
    }

    /// The largest slate this GPU formula queue committed; absent before its
    /// first.
    #[cfg(feature = "gpu")]
    pub(crate) const fn largest_slate(&self) -> Option<NonZeroUsize> {
        self.batching.largest()
    }

    /// The selections that returned this GPU formula queue to its batch size
    /// because the previous slate's answers tightened the objective bound.
    #[cfg(feature = "gpu")]
    pub(crate) const fn slate_resets(&self) -> u64 {
        self.batching.resets()
    }

    /// Record the answers the session still requests, before each pull.
    #[cfg(feature = "gpu")]
    pub(crate) const fn set_demand(&mut self, demand: Demand) {
        self.demand = demand;
    }

    /// Whether the session enumerates, so that its device slates take the
    /// two-phase protocol.
    #[cfg(feature = "gpu")]
    pub(crate) const fn enumerates(&self) -> bool {
        self.demand.enumerates
    }

    /// Record that an optimizing search keeps the serial schedule.
    #[cfg(feature = "gpu")]
    pub(crate) const fn decline_optimizing(&mut self) {
        self.lookahead.decision = Some(crate::GpuLookahead::Declined(
            crate::GpuLookaheadDeclined::Optimizing,
        ));
    }

    /// What this device queue's slates proposed ahead did.
    #[cfg(feature = "gpu")]
    pub(crate) const fn lookahead(&self) -> LookaheadReceipts {
        self.lookahead
    }

    /// Elapsed time from this queue's construction to its first answer.
    #[cfg(feature = "gpu")]
    pub(crate) fn first_answer_ns(&self) -> Option<u64> {
        self.first_answer.map(nanos)
    }

    /// Record the release of an answer: the first fixes the time to first answer.
    #[cfg(feature = "gpu")]
    fn released(&mut self) {
        if self.first_answer.is_none()
            && let Some(created) = self.created
        {
            self.first_answer = Some(created.elapsed());
        }
    }

    /// Attribute the settlement's flushed interval, which belongs to the last
    /// submission, if that submission was overlapped.
    #[cfg(feature = "gpu")]
    pub(crate) fn flushed(&mut self, report: Option<zetesis_backend::BatchComputeTime>) {
        let overlapped = std::mem::take(&mut self.previous_overlapped);
        self.attribute(report, overlapped);
        self.previous_overlapped = false;
    }

    /// The next reduct-verified model of a device enumeration. Each slate is
    /// checked in prefix-order submissions, at most one in flight. While its
    /// last submission is checked the stream proposes the next slate, when the
    /// session enumerates and the answers still owed exceed the slate. The
    /// slate commits after its checks decode, and a control stop observed then
    /// leaves it pending, its checks dropped. Nothing is in flight when this
    /// returns.
    #[cfg(feature = "gpu")]
    pub(crate) fn next_device<D: device::SlateDevice>(
        &mut self,
        models: &mut StableModels,
        options: &SolveConfig,
        cancellation: &Cancellation,
        phases: &crate::phase_timing::Recorder,
        device: &mut D,
    ) -> Option<Result<Interpretation, Failure>> {
        loop {
            if let Err(error) = cancellation.poll() {
                return Some(Err(Failure::Search(error.into())));
            }
            if let Some(model) = self.ready.pop_front() {
                self.released();
                return Some(Ok(model));
            }
            match self.check_slate(models, options, cancellation, phases, device) {
                Ok(true) => return None,
                Ok(false) => {}
                Err(failure) => return Some(Err(failure)),
            }
        }
    }

    /// Check and commit one slate; `true` when the stream is exhausted.
    #[cfg(feature = "gpu")]
    fn check_slate<D: device::SlateDevice>(
        &mut self,
        models: &mut StableModels,
        options: &SolveConfig,
        cancellation: &Cancellation,
        phases: &crate::phase_timing::Recorder,
        device: &mut D,
    ) -> Result<bool, Failure> {
        use crate::phase_timing::SolvePhase;
        use device::PreparedSubmission;
        use zetesis_sat::SlateProposal;

        let selected = self.batching.select(models, options);
        let limits = BatchLimits {
            max_candidates: selected,
            max_pending_bytes: options.max_batch_bytes,
        };
        let ticket = match models.propose_slate(limits).map_err(Failure::from)? {
            SlateProposal::New(ticket) => {
                let proposed = models
                    .slate(&ticket)
                    .map_err(Failure::from)?
                    .candidates()
                    .len();
                self.batching.proposed(selected, proposed);
                ticket
            }
            SlateProposal::Ahead(ticket) => {
                self.batching.promoted();
                ticket
            }
            SlateProposal::Retry(ticket) => ticket,
            SlateProposal::Exhausted => return Ok(true),
            _ => return Err(Failure::from(crate::ExecutorError::ForeignBatch)),
        };
        let slate = ticket.slate();
        let (theory, count) = {
            let view = models.slate(&ticket).map_err(Failure::from)?;
            (view.theory().clone(), view.candidates().len())
        };
        let mut verdicts = Vec::new();
        verdicts
            .try_reserve_exact(count)
            .map_err(|_| Failure::Search(zetesis_sat::Incomplete::Allocation))?;
        let mut ahead = None;
        while verdicts.len() < count {
            let offset = verdicts.len();
            let view = models.slate(&ticket).map_err(Failure::from)?;
            let prepared = phases.measure(SolvePhase::GpuHostOracle, || {
                device.prepare(view, offset, cancellation)
            })?;
            let offered = count - offset;
            self.batching.admitted(offered, prepared.count());
            let overlapped = prepared.count() >= offered && self.engages(count);
            let checked = if overlapped {
                let lookahead_limits = BatchLimits {
                    max_candidates: self.batching.select_lookahead(models, options),
                    max_pending_bytes: options.max_batch_bytes,
                };
                let submission = phases.start(SolvePhase::GpuHostOracle);
                let mut finish = None;
                let (checked, proposal) = prepared.check_overlapping(cancellation, || {
                    drop(submission);
                    let proposal = models.propose_lookahead(lookahead_limits);
                    finish = Some((phases.start(SolvePhase::GpuHostOracle), Instant::now()));
                    proposal
                });
                if let Some((span, started)) = finish {
                    drop(span);
                    self.lookahead.finish_wait_ns = self
                        .lookahead
                        .finish_wait_ns
                        .saturating_add(nanos(started.elapsed()));
                }
                ahead = proposal.map(|proposal| (proposal, lookahead_limits.max_candidates));
                checked
            } else {
                phases.measure(SolvePhase::GpuHostOracle, || prepared.check(cancellation))
            };
            let decoded = device.decode(checked, &theory, slate, offset, offered)?;
            self.attribute(decoded.compute, overlapped);
            verdicts.extend(decoded.verdicts);
        }
        // A stop observed now leaves the slate pending, its checks dropped:
        // the commit's completion and model construction poll the same control.
        if let Err(error) = cancellation.poll() {
            return Err(Failure::Search(error.into()));
        }
        if let Some((proposal, selected)) = ahead {
            self.proposed_ahead(models, proposal.map_err(Failure::from)?, selected, count);
        }
        let prior_completion = models.batch_statistics().completion_calls;
        let result = models.commit_slate(ticket, verdicts, &mut self.completion);
        if models.batch_statistics().completion_calls != prior_completion
            && let Some(progress) = self.completion.last_statistics()
        {
            self.accounting.record(progress);
        }
        let batch = result.map_err(Failure::from)?;
        // The bound generation is read after the commit and before any of the
        // slate's models is scored.
        self.batching.committed_in_flight(models.bound_generation());
        self.ready = batch.into();
        Ok(false)
    }

    /// Record what the lookahead beside a slate of `count` candidates did.
    #[cfg(feature = "gpu")]
    fn proposed_ahead(
        &mut self,
        models: &StableModels,
        proposal: zetesis_sat::Lookahead,
        selected: NonZeroUsize,
        count: usize,
    ) {
        self.lookahead.decision = Some(match proposal {
            zetesis_sat::Lookahead::Declined(reason) => {
                crate::GpuLookahead::Declined(crate::GpuLookaheadDeclined::Stream(reason))
            }
            _ => crate::GpuLookahead::Engaged,
        });
        if let zetesis_sat::Lookahead::Proposed(_) = proposal {
            self.lookahead.slates = self.lookahead.slates.saturating_add(1);
            let proposed = models.batch_statistics().pending.saturating_sub(count);
            self.batching.proposed_ahead(selected, proposed);
        }
    }

    /// Whether the last submission of a slate of `count` candidates is checked
    /// while the stream proposes the next slate; a refusal is recorded.
    #[cfg(feature = "gpu")]
    fn engages(&mut self, count: usize) -> bool {
        let declined = if !self.demand.enumerates {
            Some(crate::GpuLookaheadDeclined::Optimizing)
        } else if self.demand.owed.is_some_and(|owed| owed <= count) {
            Some(crate::GpuLookaheadDeclined::RequestedAnswers)
        } else {
            None
        };
        if let Some(reason) = declined {
            self.lookahead.decision = Some(crate::GpuLookahead::Declined(reason));
            return false;
        }
        true
    }

    /// Count a reported interval as overlapped device compute when the
    /// submission it belongs to, the one before, was overlapped.
    #[cfg(feature = "gpu")]
    fn attribute(&mut self, report: Option<zetesis_backend::BatchComputeTime>, overlapped: bool) {
        if self.previous_overlapped
            && let Some(zetesis_backend::BatchComputeTime::Measured(duration)) = report
        {
            self.lookahead.device_ns = self.lookahead.device_ns.saturating_add(nanos(duration));
        }
        self.previous_overlapped = overlapped;
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
                #[cfg(feature = "gpu")]
                self.released();
                return Some(Ok(model));
            }
            if models.exhausted() {
                return None;
            }
            #[cfg(feature = "gpu")]
            let max_candidates = self.batching.select(models, options);
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
                    // The bound generation is read after the commit and
                    // before any of the slate's models is scored.
                    #[cfg(feature = "gpu")]
                    self.batching.committed(
                        limits.max_candidates,
                        accepted_prefix.0,
                        accepted_prefix.1,
                        models.bound_generation(),
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
                // The closure form never names a slate by ticket, so its
                // receipts are bound to their batch by slice, as before.
                Err(BatchError::SlateOrder) => {
                    return Some(Err(Failure::from(crate::ExecutorError::ForeignBatch)));
                }
            }
        }
    }
}

/// A two-phase protocol failure, as the closure form's failures map.
#[cfg(feature = "gpu")]
impl From<BatchError<std::convert::Infallible>> for Failure {
    fn from(error: BatchError<std::convert::Infallible>) -> Self {
        match error {
            BatchError::Search(error) | BatchError::Limits(error) => Self::Search(error),
            BatchError::Shape { expected, actual } => {
                Self::Run(SolveError::FormulaBatchShape { expected, actual })
            }
            BatchError::SlateOrder => Self::from(crate::ExecutorError::ForeignBatch),
            BatchError::Checker(never) => match never {},
        }
    }
}

#[cfg(feature = "gpu")]
fn nanos(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

#[cfg(feature = "gpu")]
use std::time::Instant;

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
