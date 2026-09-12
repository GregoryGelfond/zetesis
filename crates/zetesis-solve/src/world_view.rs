//! Bounded collection with original-program enumeration provenance.

use std::fmt;

use zetesis_cpu::Control;

use crate::execution_observation::ExecutionSink;
use crate::{
    AnswerSelection, AnswerSet, Completion, PreparedInput, SemanticOutcome, Session,
    SessionBuilder, SolveConfig, SolveFailure, Subject,
};

/// Storage ceilings for explicitly collecting a complete answer-set family.
/// Zero is a real ceiling. Search, scoring and batch limits remain in
/// [`SolveConfig`]; these ceilings do not bound the search engine or process RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldViewLimits {
    /// Maximum retained full answer sets, including an empty answer set.
    pub max_answer_sets: usize,
    /// Maximum summed atom counts across retained full answer sets.
    pub max_atoms: usize,
    /// Maximum canonical answer payload bytes: each answer's entire referenced
    /// atom catalog, selected positions and optional score priorities. Shared
    /// catalogs are conservatively recounted. Excludes shared subjects, spare
    /// vector capacity, allocator/Arc overhead, execution state and the one
    /// yielded answer being considered for admission.
    pub max_bytes: usize,
}

impl Default for WorldViewLimits {
    fn default() -> Self {
        Self {
            max_answer_sets: 100_000,
            max_atoms: 1_000_000,
            max_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Why a complete original answer-set family could not be returned.
/// An incomplete search and a full collection ceiling are distinct failures.
#[derive(Debug)]
pub enum WorldViewError {
    /// A typed session setup or execution failure.
    Solve(Box<SolveFailure>),
    /// The session stopped without exhaustive original-program coverage.
    /// The retained outcome distinguishes requested models and interruptions.
    NotExhausted,
    /// Another answer would exceed the number of retained answer sets.
    AnswerSets,
    /// Another answer would exceed the summed full-atom ceiling.
    Atoms,
    /// Another answer would exceed the canonical payload-byte ceiling.
    Bytes,
    /// Collection payload arithmetic exceeded the machine's integer range.
    Overflow,
    /// The collection could not reserve another answer slot.
    Allocation,
}

impl fmt::Display for WorldViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solve(error) => error.fmt(f),
            Self::NotExhausted => f.write_str("original answer-set search was not exhausted"),
            Self::AnswerSets => f.write_str("world-view answer-set limit reached"),
            Self::Atoms => f.write_str("world-view atom limit reached"),
            Self::Bytes => f.write_str("world-view payload-byte limit reached"),
            Self::Overflow => f.write_str("world-view payload arithmetic overflowed"),
            Self::Allocation => f.write_str("world-view answer storage could not be reserved"),
        }
    }
}

impl std::error::Error for WorldViewError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Solve(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

/// Partial checked answers and semantic evidence from a failed collection.
/// The prefix is never a complete world view. Verified accounting can include
/// an answer whose scoring or collection admission stopped before retention.
#[derive(Debug)]
pub struct WorldViewFailure {
    cause: WorldViewError,
    subject: Subject,
    answer_sets: Vec<AnswerSet>,
    outcome: Option<Box<SemanticOutcome>>,
}

impl WorldViewFailure {
    /// Original setup, execution, coverage or collection refusal.
    #[must_use]
    pub const fn cause(&self) -> &WorldViewError {
        &self.cause
    }

    /// Exact original subject, including when setup failed before any answer.
    #[must_use]
    pub const fn subject(&self) -> &Subject {
        &self.subject
    }

    /// Checked answers retained before failure; no completeness is implied.
    #[must_use]
    pub fn answer_sets(&self) -> &[AnswerSet] {
        &self.answer_sets
    }

    /// Available session evidence, independent of collection admission.
    #[must_use]
    pub fn outcome(&self) -> Option<&SemanticOutcome> {
        self.outcome.as_deref()
    }

    /// Transfer the retained checked prefix, explicitly discarding failure and
    /// coverage context. Each answer retains its original subject association.
    #[must_use]
    pub fn into_answer_sets(self) -> Vec<AnswerSet> {
        self.answer_sets
    }
}

impl fmt::Display for WorldViewFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}

impl std::error::Error for WorldViewFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// All full answer sets of one original immutable program, obtained only by
/// completed unrestricted enumeration. Objectives annotate members with scores;
/// they do not remove nonoptimal answers. An empty family establishes
/// inconsistency; it differs from the singleton family containing an empty answer.
///
/// Materialization is opt-in and bounded. [`Session::enumerate`] remains the
/// streaming alternative. No public constructor can attach an arbitrary vector
/// or detached completion report to this complete-family claim.
///
/// ```compile_fail
/// use zetesis_solve::{AnswerSet, SemanticOutcome, Subject, WorldView};
/// fn attach(subject: Subject, answer_sets: Vec<AnswerSet>, outcome: SemanticOutcome) -> WorldView {
///     WorldView { subject, answer_sets, outcome }
/// }
/// ```
#[derive(Debug)]
pub struct WorldView {
    subject: Subject,
    answer_sets: Vec<AnswerSet>,
    outcome: SemanticOutcome,
}

impl WorldView {
    /// Enumerate and retain the complete original family from a fresh session.
    ///
    /// Use `config.models = 0` to request exhaustive enumeration. A positive
    /// yield limit remains effective and may prevent completion. Objective
    /// pruning and incumbent selection are disabled, while objective evaluation
    /// and its cumulative budgets remain active. Members are moved into the
    /// collection with no repeated membership check or full-model clone.
    /// To supply caller-owned execution resources or observe the entire solve,
    /// use [`SessionBuilder::collect`] or [`SessionBuilder::collect_observed`].
    /// This convenience operation uses that same request and collection loop.
    ///
    /// Work is the ordinary search and scoring plus a scan of every returned
    /// answer's payload. Retained space is the full family within `limits`, in
    /// addition to the session's independent budgets. The family can be
    /// exponentially large. Fallible vector reservation does not change the
    /// existing raw atom-set representation's infallible tree allocation.
    ///
    /// # Errors
    /// Returns the retained checked prefix on any setup/execution failure,
    /// incomplete enumeration or exceeded collection ceiling. A rejected answer
    /// is dropped; verified counters can therefore exceed the prefix length.
    /// No partial family, failed score or bare iterator end establishes a world
    /// view. Cancellation remains cooperative at the session's polling points.
    pub fn collect(
        input: PreparedInput<'_>,
        config: SolveConfig,
        limits: WorldViewLimits,
        control: Control,
    ) -> Result<Self, WorldViewFailure> {
        Session::builder(input, config, control).collect(limits)
    }

    pub(crate) fn collect_request(
        request: SessionBuilder<'_>,
        limits: WorldViewLimits,
        observations: &mut impl ExecutionSink,
    ) -> Result<Self, WorldViewFailure> {
        // Owning an unstarted request ensures that no answer can have escaped
        // retention before this loop begins. Exhaustion alone would not repair
        // the missing prefix of an already consumed session.
        let subject = request.subject();
        let mut session = request
            .selection(AnswerSelection::All)
            .start_with(observations)
            .map_err(|error| WorldViewFailure {
                outcome: error.semantic().cloned().map(Box::new),
                cause: WorldViewError::Solve(Box::new(error)),
                subject: subject.clone(),
                answer_sets: Vec::new(),
            })?;
        let mut collection = Collection::default();
        while let Some(result) = session.pull(observations) {
            let admission = result
                .map_err(|error| WorldViewError::Solve(Box::new(error)))
                .and_then(|answer| collection.retain(answer, limits));
            if let Err(cause) = admission {
                return Err(WorldViewFailure {
                    cause,
                    subject,
                    answer_sets: collection.answer_sets,
                    outcome: Some(Box::new(session.stop())),
                });
            }
        }
        let outcome = session.stop();
        if outcome.completion() != Some(Completion::Exhausted)
            || outcome.selection() != Some(AnswerSelection::All)
        {
            return Err(WorldViewFailure {
                cause: WorldViewError::NotExhausted,
                subject,
                answer_sets: collection.answer_sets,
                outcome: Some(Box::new(outcome)),
            });
        }
        Ok(Self {
            subject,
            answer_sets: collection.answer_sets,
            outcome,
        })
    }

    /// Original immutable subject shared by every member and the coverage result.
    #[must_use]
    pub const fn subject(&self) -> &Subject {
        &self.subject
    }

    /// Every full answer set, in enumeration order rather than display order.
    /// Borrowing is constant time and neither copies nor recomputes a member.
    #[must_use]
    pub fn answer_sets(&self) -> &[AnswerSet] {
        &self.answer_sets
    }

    /// Exhaustive original-program evidence from the owned collection session.
    /// Collection and publication are distinct; this has no sink acknowledgements.
    #[must_use]
    pub const fn outcome(&self) -> &SemanticOutcome {
        &self.outcome
    }

    /// Number of distinct full answer sets, including an empty answer if present.
    #[must_use]
    pub fn len(&self) -> usize {
        self.answer_sets.len()
    }

    /// Whether the original program has no answer sets. Completeness makes this
    /// an inconsistency result, unlike an empty prefix of an unfinished session.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.answer_sets.is_empty()
    }

    /// Transfer the full checked family, discarding its enclosing coverage value.
    /// Each member keeps its original subject, interpretation and optional score.
    #[must_use]
    pub fn into_answer_sets(self) -> Vec<AnswerSet> {
        self.answer_sets
    }
}

#[derive(Default)]
struct Collection {
    answer_sets: Vec<AnswerSet>,
    atoms: usize,
    bytes: usize,
}

impl Collection {
    fn retain(&mut self, answer: AnswerSet, limits: WorldViewLimits) -> Result<(), WorldViewError> {
        if self.answer_sets.len() >= limits.max_answer_sets {
            return Err(WorldViewError::AnswerSets);
        }
        let atoms = self
            .atoms
            .checked_add(answer.interpretation().atoms().len())
            .ok_or(WorldViewError::Overflow)?;
        if atoms > limits.max_atoms {
            return Err(WorldViewError::Atoms);
        }
        let bytes = self
            .bytes
            .checked_add(payload_bytes(&answer)?)
            .ok_or(WorldViewError::Overflow)?;
        if bytes > limits.max_bytes {
            return Err(WorldViewError::Bytes);
        }
        self.answer_sets
            .try_reserve(1)
            .map_err(|_| WorldViewError::Allocation)?;
        self.answer_sets.push(answer);
        self.atoms = atoms;
        self.bytes = bytes;
        Ok(())
    }
}

fn payload_bytes(answer: &AnswerSet) -> Result<usize, WorldViewError> {
    // One optional-score tag; a present score adds u64 length and i32/i64 pairs.
    const SCORE_LEVEL_BYTES: usize = 4 + 8;
    const SCORE_HEADER_BYTES: usize = 1 + 8;
    let model = crate::optimization::payload_bytes(answer.interpretation())
        .map_err(|_| WorldViewError::Overflow)?;
    let score = match answer.score() {
        None => 1,
        Some(score) => score
            .costs()
            .len()
            .checked_mul(SCORE_LEVEL_BYTES)
            .and_then(|bytes| bytes.checked_add(SCORE_HEADER_BYTES))
            .ok_or(WorldViewError::Overflow)?,
    };
    model.checked_add(score).ok_or(WorldViewError::Overflow)
}
