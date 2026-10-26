//! Bounded original-candidate proposal, injected propagation and exact completion.

use std::convert::Infallible;
use std::fmt;
use std::num::NonZeroUsize;
use std::sync::Arc;

use super::{CompletionExecutor, IndexedTheory, StableModels};
use crate::Incomplete;
use crate::budget::{Budget, increment};
use crate::timing::{self, Phase};
use zetesis_ferraris::{EvaluationLimits, Interpretation, SlateId, SlateRef, Theory};

#[cfg(test)]
mod tests;
mod validation;

/// Faults the unit tests inject where no fixture fails on demand, as
/// `original_index`'s build hook does: an allocation the allocator would
/// refuse, and a walk whose index cannot be read.
#[cfg(test)]
pub(super) mod hooks {
    use std::cell::Cell;

    thread_local! {
        static REFUSE_LOOKAHEAD: Cell<bool> = const { Cell::new(false) };
        static FAIL_WALK: Cell<bool> = const { Cell::new(false) };
    }

    /// Refuse the next lookahead reservation on this thread.
    pub(in crate::ferraris) fn refuse_next_lookahead_reservation() {
        REFUSE_LOOKAHEAD.with(|refuse| refuse.set(true));
    }

    pub(super) fn refuse_lookahead_reservation() -> bool {
        REFUSE_LOOKAHEAD.with(Cell::take)
    }

    /// Fail the next slate production's walk on this thread, as a failure to
    /// read the walk's index would.
    pub(in crate::ferraris) fn fail_next_walk() {
        FAIL_WALK.with(|fail| fail.set(true));
    }

    pub(super) fn fail_walk() -> bool {
        FAIL_WALK.with(Cell::take)
    }
}
use validation::Validation;

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

/// Bounds on pending candidate interpretations and their validation workspace.
#[derive(Clone, Copy, Debug)]
pub struct BatchLimits {
    /// Maximum candidates proposed for one checker call.
    pub max_candidates: NonZeroUsize,
    /// Logical payload allowance for pending/result vectors, interpretation words,
    /// and the original-validation workspace's header and actual retained capacity.
    /// Shared theory storage, allocator overhead and checker-private storage are excluded.
    /// It covers every slate pending at once: a slate proposed ahead
    /// ([`StableModels::propose_lookahead`]) is admitted net of the oldest
    /// pending slate's rows.
    pub max_pending_bytes: u64,
}

/// Proposal/check/commit counters, including unresolved work after a failure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct BatchStatistics {
    /// Original-satisfaction evaluation work, including failed attempts. This is
    /// separate from cumulative candidate-search and exact-reduct work.
    pub original_validation_work: u64,
    /// Current original-validation workspace header and actual retained capacity.
    /// Previously temporary validation storage is included in `max_pending_bytes`.
    pub original_validation_bytes: u128,
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
    /// Proposed candidates whose results are still retained, in the oldest
    /// pending slate and in a slate proposed ahead. A failed last exact block
    /// is a delayed terminal error and cannot establish exhaustive coverage.
    pub pending: usize,
    /// Slates proposed ahead of their predecessor's commit
    /// ([`StableModels::propose_lookahead`]).
    pub lookahead_slates: u64,
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
    /// A slate ticket did not name the stream's oldest pending slate: it named
    /// a slate proposed after it, one already committed, or another stream's.
    /// Nothing was committed, and the pending slates remain retryable.
    SlateOrder,
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
            Self::SlateOrder => {
                f.write_str("slate ticket does not name the stream's oldest pending slate")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for BatchError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Search(error) | Self::Limits(error) => Some(error),
            Self::Checker(error) => Some(error),
            Self::Shape { .. } | Self::SlateOrder => None,
        }
    }
}

/// The stream's handle on one hand-out of one of its slates. It is neither
/// `Copy` nor `Clone`, and its commit consumes it, so of several hand-outs of
/// one slate, a retry after a failed check among them, only one can commit it.
#[derive(Debug)]
pub struct SlateTicket {
    slate: SlateId,
}

impl SlateTicket {
    /// The identity of the slate this ticket names.
    #[must_use]
    pub const fn slate(&self) -> SlateId {
        self.slate
    }

    /// Spend the ticket on the commit it was issued for.
    const fn into_slate(self) -> SlateId {
        self.slate
    }
}

/// The slate [`StableModels::propose_slate`] hands out, with its ticket.
#[derive(Debug)]
#[non_exhaustive]
pub enum SlateProposal {
    /// A slate proposed now, under the call's limits.
    New(SlateTicket),
    /// The slate proposed ahead, now the oldest pending slate. It has not been
    /// checked; it keeps the candidate count it was proposed with.
    Ahead(SlateTicket),
    /// The oldest pending slate, whose check did not commit: the same
    /// candidates, in the same order.
    Retry(SlateTicket),
    /// Nothing is pending and the candidate frontier is exhausted.
    Exhausted,
}

/// What [`StableModels::propose_lookahead`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Lookahead {
    /// A slate was proposed ahead, with this identity. Its ticket is issued
    /// when [`StableModels::propose_slate`] hands it out, after the pending
    /// slate commits.
    Proposed(SlateId),
    /// The proposal was empty because the frontier is exhausted. Exhaustion
    /// is established after the pending slate commits.
    Exhausted,
    /// The production stopped before its first candidate. The stop is held
    /// and reported by the proposal after the pending slate commits.
    Stopped,
    /// No slate was proposed ahead, for this reason.
    Declined(LookaheadDeclined),
}

/// Why [`StableModels::propose_lookahead`] proposed no slate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LookaheadDeclined {
    /// The stream expects candidate bounds
    /// ([`StableModels::expect_candidate_bounds`]): a slate produced ahead
    /// would be produced under a bound its predecessor's answers may tighten.
    CandidateBounds,
    /// The pending slate was proposed under the independent protocol, whose
    /// candidates the host validates before its checker; only the joint
    /// protocol proposes ahead.
    IndependentCheck,
    /// No slate is pending.
    NoPendingSlate,
    /// A slate is already proposed ahead.
    AlreadyProposed,
    /// The slate's rows do not fit the pending-bytes limit beside the pending
    /// slate's.
    PendingBytes,
    /// The allocator refused the slate's rows beside the pending slate's
    /// resident rows. The serial schedule reserves the next slate only after
    /// the commit releases those rows, so the refusal need not recur there:
    /// the proposal after the commit reserves as the serial schedule's does.
    Allocation,
    /// The pending slate's own production holds a stop, after which the
    /// serial schedule proposes nothing.
    PendingStop,
}

impl LookaheadDeclined {
    /// A stable snake-case name for reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::CandidateBounds => "candidate_bounds",
            Self::IndependentCheck => "independent_check",
            Self::NoPendingSlate => "no_pending_slate",
            Self::AlreadyProposed => "already_proposed",
            Self::PendingBytes => "pending_bytes",
            Self::Allocation => "allocation",
            Self::PendingStop => "pending_stop",
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct State {
    pub(super) pending: Vec<Interpretation>,
    validation: Validation,
    original_checked: bool,
    exhausted: bool,
    statistics: BatchStatistics,
    /// The oldest pending slate's identity, while a slate is pending.
    id: Option<SlateId>,
    /// How the oldest pending slate was last handed out, or that it was
    /// promoted from ahead and has not been handed out since.
    hand_out: LastHandOut,
    /// A slate proposed ahead of the oldest pending slate's commit.
    ahead: Option<Ahead>,
    /// The identity issued last: slates are numbered in proposal order.
    issued: Option<SlateId>,
    /// The stream expects candidate bounds, so it never proposes ahead.
    pub(super) bounds_expected: bool,
}

impl State {
    /// The next slate's identity, for a stream over `theory`.
    fn issue(&mut self, theory: &Theory) -> Result<SlateId, Incomplete> {
        let id = match self.issued {
            None => SlateId::first(theory),
            Some(last) => last.successor(),
        }
        .ok_or(Incomplete::CounterOverflow)?;
        self.issued = Some(id);
        Ok(id)
    }
}

/// A slate proposed ahead: its identity, issued before its production; its
/// candidates; whether its production exhausted the frontier; and the stop
/// its production holds. An empty one records the exhaustion or the stop it
/// met, and one whose identity could not be issued has none.
#[derive(Debug)]
struct Ahead {
    id: Option<SlateId>,
    candidates: Vec<Interpretation>,
    exhausted: bool,
    stop: Option<Incomplete>,
}

impl Ahead {
    /// An empty slate proposed ahead that records only the exhaustion or the
    /// stop its proposal met.
    const fn held(exhausted: bool, stop: Option<Incomplete>) -> Self {
        Self {
            id: None,
            candidates: Vec::new(),
            exhausted,
            stop,
        }
    }
}

/// What one slate's production found besides its candidates: whether the
/// frontier is exhausted after them, and the stop it holds, which the stream
/// reports after the slate commits.
#[derive(Clone, Copy, Debug, Default)]
struct Production {
    exhausted: bool,
    stop: Option<Incomplete>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OriginalCheck {
    Independent,
    Joint,
}

/// How the oldest pending slate was last handed out. A slate promoted from
/// ahead was proposed under the joint protocol and is handed out next as
/// `Ahead`, not `Retry`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum LastHandOut {
    #[default]
    Independent,
    Joint,
    Promoted,
}

impl From<OriginalCheck> for LastHandOut {
    fn from(original: OriginalCheck) -> Self {
        match original {
            OriginalCheck::Independent => Self::Independent,
            OriginalCheck::Joint => Self::Joint,
        }
    }
}

/// Which slate a hand-out names.
#[derive(Clone, Copy)]
enum Kind {
    New,
    Ahead,
    Retry,
}

/// The outcome of one hand-out: a nonempty slate that passed the call's
/// limits, or an empty proposal, after which exhaustion is established.
enum HandOut {
    Ready(Kind),
    Empty,
}

impl StableModels {
    /// Current explicit proposal/commit accounting. Pending candidates survive
    /// checker failures and candidate-only restrictions; they are never silently dropped.
    #[must_use]
    pub fn batch_statistics(&self) -> BatchStatistics {
        BatchStatistics {
            pending: self.batch.pending.len()
                + self
                    .batch
                    .ahead
                    .as_ref()
                    .map_or(0, |ahead| ahead.candidates.len()),
            original_validation_work: self.batch.validation.work(),
            original_validation_bytes: self.batch.validation.retained_bytes(),
            ..self.batch.statistics
        }
    }

    /// Candidate capacity of the current joint-check pending-memory envelope.
    ///
    /// Uses this theory's interpretation width, the existing pending/result row
    /// charge and actual retained validation storage. The value is the
    /// envelope's whole count, and zero unless the rows the stream holds fit:
    /// the pending vector's capacity and, beside a slate proposed ahead
    /// ([`Self::propose_lookahead`]), both slates' retained rows. The query
    /// allocates nothing, releases no scratch and does not change pending
    /// candidates or counters.
    ///
    /// This is a proposal-sizing bound, not a reservation: allocation can fail
    /// or return excess capacity, which the batch operation still checks. It
    /// does not admit independent original evaluation, checker-private storage,
    /// exact completion, or additional search work.
    #[must_use]
    pub fn joint_batch_capacity(&self, max_pending_bytes: u64) -> usize {
        let Some(available) =
            u128::from(max_pending_bytes).checked_sub(self.batch.validation.retained_bytes())
        else {
            return 0;
        };
        let count = (available / self.pending_row_bytes())
            .min((isize::MAX as usize / size_of::<Interpretation>()) as u128);
        if count < self.held_rows() as u128 {
            return 0;
        }
        usize::try_from(count).unwrap_or(usize::MAX)
    }

    /// Candidate capacity for a slate proposed ahead
    /// ([`Self::propose_lookahead`]): the rows that fit the pending-memory
    /// envelope beside the oldest pending slate's retained rows and the actual
    /// retained validation storage. Zero when no slate is pending, when one is
    /// already proposed ahead, or when the pending slate's rows alone exceed
    /// the envelope. A sizing bound like [`Self::joint_batch_capacity`], not a
    /// reservation; the query allocates nothing.
    #[must_use]
    pub fn lookahead_capacity(&self, max_pending_bytes: u64) -> usize {
        if self.batch.pending.is_empty() || self.batch.ahead.is_some() {
            return 0;
        }
        let row = self.pending_row_bytes();
        let Some(available) = u128::from(max_pending_bytes)
            .checked_sub(self.batch.validation.retained_bytes())
            .and_then(|available| {
                available.checked_sub(row * self.batch.pending.capacity() as u128)
            })
        else {
            return 0;
        };
        let count =
            (available / row).min((isize::MAX as usize / size_of::<Interpretation>()) as u128);
        usize::try_from(count).unwrap_or(usize::MAX)
    }

    /// Rows retained by the pending slates: the oldest pending slate's vector
    /// capacity and, if one is proposed ahead, its vector capacity.
    fn held_rows(&self) -> usize {
        self.batch.pending.capacity()
            + self
                .batch
                .ahead
                .as_ref()
                .map_or(0, |ahead| ahead.candidates.capacity())
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
        self.checked_batch(limits, completion, OriginalCheck::Independent, checker)
    }

    /// Check original satisfaction and frozen-reduct membership together.
    ///
    /// This is a trusted checker protocol like [`Self::next_batch`], with the
    /// additional obligation to check original satisfaction for every candidate.
    /// Every result other than `NotModel`, including `Residual` and `Refuted`,
    /// must establish that the exact candidate satisfies the original theory.
    /// No independent original evaluation precedes this checker. Exact ownership,
    /// pending storage, candidate quotas and residual completion remain checked
    /// by this enumeration. Objective restrictions never enter the theory.
    ///
    /// A device coordinator can discharge this contract with subject-bound
    /// results from a joint original/reduct operation. Arbitrary verdict data
    /// alone is not evidence that the obligation was checked. The two-phase form,
    /// [`Self::propose_slate`] and [`Self::commit_slate`], carries the same
    /// obligation: who decides membership is unchanged, and a slate's identity
    /// binds its verdicts to it against a correct caller's misordering, not
    /// against fabricated verdicts.
    ///
    /// # Errors
    /// Uses the same failures and retry rules as [`Self::next_batch`]. A
    /// `NotModel` result terminates with `InvalidWitness`; it never establishes
    /// exhaustion. After a checker failure, switching to the independent
    /// protocol validates retained candidates before calling its checker.
    pub fn next_batch_with_joint_completion<E>(
        &mut self,
        limits: BatchLimits,
        completion: &mut CompletionExecutor,
        checker: impl FnOnce(&Theory, &[Interpretation]) -> Result<Vec<BatchVerdict>, E>,
    ) -> Result<Vec<Interpretation>, BatchError<E>> {
        self.checked_batch(limits, completion, OriginalCheck::Joint, checker)
    }

    fn checked_batch<E>(
        &mut self,
        limits: BatchLimits,
        completion: &mut CompletionExecutor,
        original: OriginalCheck,
        checker: impl FnOnce(&Theory, &[Interpretation]) -> Result<Vec<BatchVerdict>, E>,
    ) -> Result<Vec<Interpretation>, BatchError<E>> {
        if self.exhausted {
            return Ok(Vec::new());
        }
        if self.terminal {
            return Err(BatchError::Search(Incomplete::ClosedEnumerator));
        }
        let result = match self.hand_out(limits, original) {
            Ok(HandOut::Empty) => Ok(Vec::new()),
            Ok(HandOut::Ready(_)) => match checker(&self.theory, &self.batch.pending) {
                Ok(verdicts) => self.commit_pending(verdicts, completion, original),
                Err(error) => Err(BatchError::Checker(error)),
            },
            Err(error) => Err(error),
        };
        self.terminate_on_search(&result);
        result
    }

    /// A search failure makes the stream terminal without exhaustion and
    /// joins its producers; other failures keep the pending slates retryable.
    fn terminate_on_search<T, E>(&mut self, result: &Result<T, BatchError<E>>) {
        if matches!(result, Err(BatchError::Search(_))) {
            self.terminal = true;
            let _ = self.proposer.finish_production();
        }
    }

    /// The slate the caller checks next, with a ticket for this hand-out of
    /// it: a slate proposed now under `limits`, the slate proposed ahead
    /// ([`Self::propose_lookahead`]), now the oldest pending slate, or the
    /// oldest pending slate again after a check that did not commit. The
    /// candidates are read with [`Self::slate`] and committed with
    /// [`Self::commit_slate`].
    ///
    /// This is the two-phase form of [`Self::next_batch_with_joint_completion`],
    /// with the same trusted joint-checker obligation: the caller's check of
    /// each candidate must establish original satisfaction and decide the
    /// frozen reduct's proper-subset query, as that method's checker does.
    /// Original satisfaction is not checked independently. Exhaustion is
    /// reported as [`SlateProposal::Exhausted`], never by an empty slate.
    ///
    /// # Errors
    /// The same typed failures as the closure form, before any checker runs:
    /// a native failure, including a stop the previous slate's production
    /// held, terminates without exhaustion; a limit refusal keeps the pending
    /// slates retryable under sufficient limits.
    pub fn propose_slate(
        &mut self,
        limits: BatchLimits,
    ) -> Result<SlateProposal, BatchError<Infallible>> {
        loop {
            if self.exhausted {
                return Ok(SlateProposal::Exhausted);
            }
            if self.terminal {
                return Err(BatchError::Search(Incomplete::ClosedEnumerator));
            }
            let result = self.hand_out(limits, OriginalCheck::Joint);
            self.terminate_on_search(&result);
            let kind = match result? {
                // The empty proposal established exhaustion or ended the
                // stream; the next pass reports which.
                HandOut::Empty => continue,
                HandOut::Ready(kind) => kind,
            };
            let slate = if let Some(id) = self.batch.id {
                id
            } else {
                let result = self.batch.issue(&self.theory).map_err(BatchError::Search);
                self.terminate_on_search(&result);
                let id = result?;
                self.batch.id = Some(id);
                id
            };
            let ticket = SlateTicket { slate };
            return Ok(match kind {
                Kind::New => SlateProposal::New(ticket),
                Kind::Ahead => SlateProposal::Ahead(ticket),
                Kind::Retry => SlateProposal::Retry(ticket),
            });
        }
    }

    /// Propose the slate after the oldest pending one while the pending one is
    /// checked, so that its production overlaps that check. Issues no ticket:
    /// the slate is handed out by [`Self::propose_slate`] as
    /// [`SlateProposal::Ahead`] after the pending slate commits.
    ///
    /// Only a slate handed out under the joint protocol, on a stream that
    /// expects no candidate bounds, is followed by one; at most one slate is
    /// proposed ahead. Its rows and validation are admitted against
    /// `limits.max_pending_bytes` net of the pending slate's retained rows.
    /// A stop its production meets is held and reported after the pending
    /// slate commits; an empty proposal establishes exhaustion only then.
    /// Every failure after the slate's identity is issued is held the same
    /// way, beside the candidates produced before it: a failure of the walk,
    /// or an overflow counting the slate. A preparation the pending bytes or
    /// the allocator refuses declines the lookahead
    /// ([`LookaheadDeclined::PendingBytes`], [`LookaheadDeclined::Allocation`]),
    /// so the proposal after the commit reserves as the serial schedule's does.
    ///
    /// # Errors
    /// Only [`Incomplete::ClosedEnumerator`], on a stream already fused.
    pub fn propose_lookahead(
        &mut self,
        limits: BatchLimits,
    ) -> Result<Lookahead, BatchError<Infallible>> {
        if self.terminal {
            return Err(BatchError::Search(Incomplete::ClosedEnumerator));
        }
        let declined = if self.batch.bounds_expected {
            Some(LookaheadDeclined::CandidateBounds)
        } else if self.exhausted || self.batch.pending.is_empty() {
            Some(LookaheadDeclined::NoPendingSlate)
        } else if self.batch.hand_out == LastHandOut::Independent {
            Some(LookaheadDeclined::IndependentCheck)
        } else if self.batch.ahead.is_some() {
            Some(LookaheadDeclined::AlreadyProposed)
        } else if self.pending_error.is_some() {
            Some(LookaheadDeclined::PendingStop)
        } else {
            None
        };
        if let Some(reason) = declined {
            return Ok(Lookahead::Declined(reason));
        }
        if self.batch.exhausted {
            self.batch.ahead = Some(Ahead::held(true, None));
            return Ok(Lookahead::Exhausted);
        }
        let (mut candidates, available) = match self.prepare_lookahead(limits) {
            Ok(prepared) => prepared,
            Err(reason) => return Ok(Lookahead::Declined(reason)),
        };
        let id = match self.batch.issue(&self.theory) {
            Ok(id) => id,
            Err(error) => {
                self.batch.ahead = Some(Ahead::held(false, Some(error)));
                return Ok(Lookahead::Stopped);
            }
        };
        // The walk reads the index the pending slate's proposal built, so
        // `produce` cannot return its index failures here; if it did, the
        // failure is held like a stop the walk meets.
        let mut production = self
            .produce(limits, available, OriginalCheck::Joint, &mut candidates)
            .unwrap_or_else(|error| Production {
                exhausted: false,
                stop: Some(error),
            });
        let proposed = !candidates.is_empty();
        if proposed && let Err(error) = increment(&mut self.batch.statistics.lookahead_slates) {
            // The first failure is the one reported, as `pending_error` keeps it.
            production = Production {
                exhausted: false,
                stop: production.stop.or(Some(error)),
            };
        }
        self.batch.ahead = Some(Ahead {
            id: Some(id),
            candidates,
            exhausted: production.exhausted,
            stop: production.stop,
        });
        Ok(if proposed {
            Lookahead::Proposed(id)
        } else if production.stop.is_some() {
            Lookahead::Stopped
        } else {
            Lookahead::Exhausted
        })
    }

    /// The oldest pending slate's identity, theory and candidates, for the
    /// ticket of one of its hand-outs.
    ///
    /// # Errors
    /// [`BatchError::SlateOrder`] when the ticket names a slate that is not the
    /// oldest pending one: one proposed ahead, one already committed, or one
    /// of another stream.
    pub fn slate(&self, ticket: &SlateTicket) -> Result<SlateRef<'_>, BatchError<Infallible>> {
        match self.batch.id {
            Some(id) if id == ticket.slate && !self.batch.pending.is_empty() => {
                SlateRef::new(id, &self.theory, &self.batch.pending).ok_or(BatchError::SlateOrder)
            }
            _ => Err(BatchError::SlateOrder),
        }
    }

    /// Commit the oldest pending slate with one ordered verdict per candidate,
    /// as the closure form commits its checker's verdicts, and return its
    /// stable members in proposal order. The slate proposed ahead, if any,
    /// then becomes the oldest pending slate. No checker runs here, so the
    /// error has no checker case. The verdicts carry the joint-checker
    /// obligation of [`Self::propose_slate`].
    ///
    /// # Errors
    /// [`BatchError::SlateOrder`] for a ticket that does not name the oldest
    /// pending slate, which commits nothing. Otherwise the closure form's
    /// failures: a shape refusal or completion-scratch limit keeps the slate
    /// retryable, a `NotModel` verdict terminates with `InvalidWitness`, and a
    /// native failure terminates without exhaustion.
    pub fn commit_slate(
        &mut self,
        ticket: SlateTicket,
        verdicts: Vec<BatchVerdict>,
        completion: &mut CompletionExecutor,
    ) -> Result<Vec<Interpretation>, BatchError<Infallible>> {
        if self.terminal {
            return Err(BatchError::Search(Incomplete::ClosedEnumerator));
        }
        let slate = ticket.into_slate();
        if self.batch.id != Some(slate) || self.batch.pending.is_empty() {
            return Err(BatchError::SlateOrder);
        }
        let result = self.commit_pending(verdicts, completion, OriginalCheck::Joint);
        self.terminate_on_search(&result);
        result
    }

    /// Hand out the oldest pending slate, proposing one first when none is
    /// pending, and check it against the call's limits. Independent original
    /// validation runs here; the checker call is counted.
    fn hand_out<E>(
        &mut self,
        limits: BatchLimits,
        original: OriginalCheck,
    ) -> Result<HandOut, BatchError<E>> {
        self.cancellation
            .poll()
            .map_err(|e| BatchError::Search(e.into()))?;
        let kind = if self.batch.pending.is_empty() {
            if let Some(error) = self.pending_error.take() {
                return Err(BatchError::Search(error));
            }
            self.propose(limits, original).map_err(BatchError::Search)?;
            self.batch.hand_out = original.into();
            Kind::New
        } else if self.batch.hand_out == LastHandOut::Promoted {
            Kind::Ahead
        } else {
            Kind::Retry
        };
        if self.batch.hand_out == LastHandOut::Promoted {
            self.batch.hand_out = LastHandOut::Joint;
        }
        if self.batch.pending.is_empty() {
            if let Some(error) = self.pending_error.take() {
                return Err(BatchError::Search(error));
            }
            self.exhausted = self.batch.exhausted;
            self.terminal = self.exhausted;
            return Ok(HandOut::Empty);
        }
        if self.batch.pending.len() > limits.max_candidates.get() {
            return Err(BatchError::Limits(Incomplete::BatchCandidateLimit));
        }
        // Retained vector capacity can exceed the number of rows in a partial
        // batch. Do not silently waive its payload on a smaller retry request.
        let available = self
            .validation_allowance(limits)
            .map_err(BatchError::Limits)?;
        self.batch.validation.release_excess(available);
        if self.batch.validation.retained_bytes() > available as u128 {
            return Err(BatchError::Limits(Incomplete::PendingBytes));
        }
        if original == OriginalCheck::Independent && !self.batch.original_checked {
            for candidate in &self.batch.pending {
                self.batch
                    .validation
                    .check(
                        &self.theory,
                        candidate,
                        validation_limits(self.limits, available),
                        &self.cancellation,
                        &mut self.statistics.phase_timings,
                    )
                    .map_err(|error| {
                        if error == Incomplete::PendingBytes {
                            BatchError::Limits(error)
                        } else {
                            BatchError::Search(error)
                        }
                    })?;
            }
            self.batch.original_checked = true;
        }
        self.batch.hand_out = original.into();
        increment(&mut self.batch.statistics.checker_calls).map_err(BatchError::Search)?;
        Ok(HandOut::Ready(kind))
    }

    /// Certify, complete and commit the oldest pending slate with one ordered
    /// verdict per candidate, then promote the slate proposed ahead, if any.
    fn commit_pending<E>(
        &mut self,
        mut verdicts: Vec<BatchVerdict>,
        completion: &mut CompletionExecutor,
        original: OriginalCheck,
    ) -> Result<Vec<Interpretation>, BatchError<E>> {
        if verdicts.len() != self.batch.pending.len() {
            return Err(BatchError::Shape {
                expected: self.batch.pending.len(),
                actual: verdicts.len(),
            });
        }
        if original == OriginalCheck::Joint && verdicts.contains(&BatchVerdict::NotModel) {
            return Err(BatchError::Search(Incomplete::InvalidWitness));
        }
        self.batch.original_checked = true;
        self.certify_pending(&mut verdicts)
            .map_err(BatchError::Search)?;
        let output = self.commit(&verdicts, completion).map_err(|error| {
            if error == Incomplete::CompletionScratch {
                BatchError::Limits(error)
            } else {
                BatchError::Search(error)
            }
        })?;
        self.promote();
        Ok(output)
    }

    /// After a commit, the slate proposed ahead becomes the oldest pending
    /// slate, with its own exhaustion and held stop; it has not been checked,
    /// and its original satisfaction rests on the joint protocol. Without one,
    /// no slate is pending.
    fn promote(&mut self) {
        let Some(ahead) = self.batch.ahead.take() else {
            self.batch.id = None;
            return;
        };
        self.batch.hand_out = if ahead.candidates.is_empty() {
            LastHandOut::Joint
        } else {
            LastHandOut::Promoted
        };
        self.batch.pending = ahead.candidates;
        self.batch.id = ahead.id;
        self.batch.exhausted = ahead.exhausted;
        self.pending_error = self.pending_error.or(ahead.stop);
        self.batch.original_checked = false;
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

    fn propose(&mut self, limits: BatchLimits, original: OriginalCheck) -> Result<(), Incomplete> {
        if self.batch.exhausted {
            return Ok(());
        }
        let available = self.prepare_pending(limits)?;
        self.batch.id = Some(self.batch.issue(&self.theory)?);
        self.batch.original_checked = original == OriginalCheck::Independent;
        let mut pending = std::mem::take(&mut self.batch.pending);
        let production = self.produce(limits, available, original, &mut pending);
        self.batch.pending = pending;
        let production = production?;
        self.batch.exhausted = production.exhausted;
        self.pending_error = production.stop;
        Ok(())
    }

    /// Produce one slate into `out` under the selected original-check
    /// contract: the joined region producers' fill, or the scalar walk. A stop
    /// is held in the returned production, beside the candidates produced
    /// before it; only a failure to build or read the walk's index is returned.
    fn produce(
        &mut self,
        limits: BatchLimits,
        available: usize,
        original: OriginalCheck,
        out: &mut Vec<Interpretation>,
    ) -> Result<Production, Incomplete> {
        let validation = validation_limits(self.limits, available);
        #[cfg(test)]
        if hooks::fail_walk() {
            return Err(Incomplete::Allocation);
        }
        let index = self.walk_index()?;
        let mut budget = Budget {
            quota: crate::budget::LocalQuota,
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
                out,
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
            return Ok(self.admit_produced(&produced, available, original, out));
        }
        let mut production = Production::default();
        while out.len() < limits.max_candidates.get() {
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
                    if let Err(error) = self.batch.validation.admit(
                        &self.theory,
                        &candidate,
                        validation,
                        &self.cancellation,
                        &mut self.statistics,
                        original,
                    ) {
                        production.stop = Some(error);
                        break;
                    }
                    // The interpretation is retained even if its exact block cannot fit.
                    let started = timing::start(self.statistics.phase_timings.as_ref());
                    let block = self.proposer.exclude(&candidate, &mut budget);
                    timing::finish(
                        &mut self.statistics.phase_timings,
                        Phase::Candidates,
                        started,
                    );
                    out.push(candidate);
                    if let Err(error) = block {
                        production.stop = Some(error);
                        break;
                    }
                }
                Ok(None) => {
                    production.exhausted = true;
                    break;
                }
                Err(error) => {
                    production.stop = Some(error);
                    break;
                }
            }
        }
        self.statistics.search = budget.statistics;
        Ok(production)
    }

    /// The original index the batch's walk reads, built at the walk's one
    /// build site, as in scalar iteration; the positive cursor walks no
    /// region and builds no index. A refused build charge stops the batch.
    fn walk_index(&mut self) -> Result<Option<Arc<IndexedTheory>>, Incomplete> {
        if self.determined_candidates.is_some() {
            return Ok(None);
        }
        let mut budget = Budget {
            quota: crate::budget::LocalQuota,
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

    /// Admit a slate's candidates in `out` once its producers joined. No
    /// producer decides membership. Each owned leaf follows the selected
    /// original-check contract, just as a scalar proposal does. An independent
    /// validation failure retains only the already validated prefix.
    fn admit_produced(
        &mut self,
        produced: &super::region_proposals::Produced,
        available: usize,
        original: OriginalCheck,
        out: &mut Vec<Interpretation>,
    ) -> Production {
        let mut production = Production {
            exhausted: produced.exhausted,
            stop: produced.stopped,
        };
        for (validated, candidate) in out.iter().enumerate() {
            let result = self.batch.validation.admit(
                &self.theory,
                candidate,
                validation_limits(self.limits, available),
                &self.cancellation,
                &mut self.statistics,
                original,
            );
            if let Err(error) = result {
                // Only the validated prefix crossed the proposal boundary.
                // The stopped suffix establishes no coverage, even when
                // production had reached the end of its region frontier.
                out.truncate(validated);
                production.exhausted = false;
                production.stop = Some(error);
                break;
            }
        }
        production
    }

    fn pending_row_bytes(&self) -> u128 {
        (std::mem::size_of::<Interpretation>() as u128) * 2
            + (self.theory.atom_count().div_ceil(64) as u128) * 8
            + std::mem::size_of::<BatchVerdict>() as u128
            + 1
    }

    fn prepare_pending(&mut self, limits: BatchLimits) -> Result<usize, Incomplete> {
        let requested = self.pending_row_bytes() * limits.max_candidates.get() as u128;
        let available = u128::from(limits.max_pending_bytes)
            .checked_sub(requested)
            .ok_or(Incomplete::PendingBytes)?;
        let available = usize::try_from(available).unwrap_or(usize::MAX);
        // Release optional old validation storage before reserving a larger
        // pending vector, so the two allocations cannot exceed the request
        // merely because the previous call retained oversized scratch.
        self.batch.validation.release_excess(available);
        if self.batch.validation.retained_bytes() > available as u128 {
            return Err(Incomplete::PendingBytes);
        }
        self.batch
            .pending
            .try_reserve_exact(limits.max_candidates.get())
            .map_err(|_| Incomplete::Allocation)?;
        let actual_available = self.validation_allowance(limits)?;
        self.batch.validation.release_excess(actual_available);
        if self.batch.validation.retained_bytes() > actual_available as u128 {
            return Err(Incomplete::PendingBytes);
        }
        Ok(actual_available)
    }

    /// Reserve a slate proposed ahead beside the oldest pending slate: its
    /// rows and the validation workspace must fit `limits.max_pending_bytes`
    /// net of the pending slate's retained rows. Either refusal declines the
    /// lookahead: rows that do not fit with `PendingBytes`, a reservation the
    /// allocator refuses with `Allocation`. Serially, the next slate is
    /// reserved only after the commit has dropped the pending rows, so neither
    /// need recur then.
    fn prepare_lookahead(
        &mut self,
        limits: BatchLimits,
    ) -> Result<(Vec<Interpretation>, usize), LookaheadDeclined> {
        let row = self.pending_row_bytes();
        let Some(available) = u128::from(limits.max_pending_bytes)
            .checked_sub(row * self.batch.pending.capacity() as u128)
            .and_then(|available| available.checked_sub(row * limits.max_candidates.get() as u128))
        else {
            return Err(LookaheadDeclined::PendingBytes);
        };
        let available = usize::try_from(available).unwrap_or(usize::MAX);
        self.batch.validation.release_excess(available);
        if self.batch.validation.retained_bytes() > available as u128 {
            return Err(LookaheadDeclined::PendingBytes);
        }
        #[cfg(test)]
        if hooks::refuse_lookahead_reservation() {
            return Err(LookaheadDeclined::Allocation);
        }
        let mut candidates = Vec::new();
        candidates
            .try_reserve_exact(limits.max_candidates.get())
            .map_err(|_| LookaheadDeclined::Allocation)?;
        let Some(actual) = u128::from(limits.max_pending_bytes)
            .checked_sub(row * (self.batch.pending.capacity() + candidates.capacity()) as u128)
        else {
            return Err(LookaheadDeclined::PendingBytes);
        };
        let actual = usize::try_from(actual).unwrap_or(usize::MAX);
        self.batch.validation.release_excess(actual);
        if self.batch.validation.retained_bytes() > actual as u128 {
            return Err(LookaheadDeclined::PendingBytes);
        }
        Ok((candidates, actual))
    }

    fn validation_allowance(&self, limits: BatchLimits) -> Result<usize, Incomplete> {
        let available = u128::from(limits.max_pending_bytes)
            .checked_sub(self.pending_row_bytes() * self.held_rows() as u128)
            .ok_or(Incomplete::PendingBytes)?;
        Ok(usize::try_from(available).unwrap_or(usize::MAX))
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
            quota: crate::budget::LocalQuota,
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
        let mut output = crate::budget::storage(verdicts.len())?;
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
    // The batch owner checks exact ownership before blocking, then establishes
    // original truth under the selected contract before publication.
    Ok(Some(candidate))
}

fn validation_limits(limits: super::Limits, available: usize) -> EvaluationLimits {
    EvaluationLimits {
        max_work: limits.max_verification_work,
        max_bytes: available,
    }
}
