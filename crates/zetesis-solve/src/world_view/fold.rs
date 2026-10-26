//! Questions a program's world view decides, folded over its answer sets as
//! they arrive instead of collecting the family.
//!
//! Each question is a commutative monoid's fold over the answer sets, some with
//! an absorbing value that no further answer set can change. The fold is the
//! same in any enumeration order, and once absorbed it is decided whatever
//! answer sets remain, so the session may stop there. Otherwise the value is
//! known only when the enumeration is exhausted. The Lean module
//! `Zetesis.AxisFolds` states both rules and the world-view corollaries.

use std::fmt;

use zetesis_core::{Atom, Model, ModelError, ValueLimits};

use crate::execution_observation::Ignore;
use crate::{
    AnswerSelection, AnswerSet, Completion, SemanticOutcome, SessionBuilder, SolveFailure, Subject,
};

/// The three-valued answer to a ground literal query over a program's world
/// view, its set of answer sets, as the Gelfond–Kahl reading the agent adopts
/// defines it. Within one answer set a literal is true if present, false if its
/// strong contrary is present, and unknown otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QueryAnswer {
    /// The literal is true in every answer set.
    Yes,
    /// The literal is false in every answer set.
    No,
    /// Neither: some answer set leaves the literal not true, and some leaves it
    /// not false. One answer set in which it is unknown does both.
    Unknown,
}

/// What a fold over a program's world view established.
#[derive(Debug)]
pub enum WorldViewVerdict<V> {
    /// The fold reached its absorbing value, so the world view's value is
    /// decided whatever answer sets remain unenumerated. The witnesses are the
    /// answer sets that drove the fold there: one for whether a literal is
    /// known or possible, at most two for a query's three-valued answer, and
    /// none for cautious consequences, whose absorption can take any number.
    Decided {
        /// The world view's value.
        value: V,
        /// Checked answer sets that decided it, in enumeration order.
        witnesses: Vec<AnswerSet>,
    },
    /// The unrestricted enumeration was exhausted without absorption: the
    /// value over the complete, nonempty family.
    Complete(V),
    /// The exhausted enumeration found no answer set. An empty world view
    /// establishes inconsistency; no fold value is given, since the fold's
    /// identity would assert something of a family that does not exist.
    Inconsistent,
}

impl<V> WorldViewVerdict<V> {
    /// The decided or complete value; `None` for an inconsistent program.
    #[must_use]
    pub const fn value(&self) -> Option<&V> {
        match self {
            Self::Decided { value, .. } | Self::Complete(value) => Some(value),
            Self::Inconsistent => None,
        }
    }
}

/// A world-view question's verdict, with the subject and the session evidence
/// it rests on. A decided verdict's session stopped early, so its outcome
/// records no completion; a complete or inconsistent verdict's records
/// exhaustion of the unrestricted family.
#[derive(Debug)]
pub struct WorldViewDecision<V> {
    subject: Subject,
    verdict: WorldViewVerdict<V>,
    outcome: SemanticOutcome,
}

impl<V> WorldViewDecision<V> {
    /// Original immutable subject shared by every folded answer set.
    #[must_use]
    pub const fn subject(&self) -> &Subject {
        &self.subject
    }

    /// What the fold established.
    #[must_use]
    pub const fn verdict(&self) -> &WorldViewVerdict<V> {
        &self.verdict
    }

    /// Session evidence at the point the question was decided.
    #[must_use]
    pub const fn outcome(&self) -> &SemanticOutcome {
        &self.outcome
    }

    /// Transfer the verdict, discarding the subject and session evidence.
    #[must_use]
    pub fn into_verdict(self) -> WorldViewVerdict<V> {
        self.verdict
    }
}

/// Why a world-view question was not decided. None of these is a logical value.
#[derive(Debug)]
pub enum WorldViewDecisionError {
    /// A typed session setup or execution failure.
    Solve(Box<SolveFailure>),
    /// The session stopped before the fold was absorbed and before the
    /// unrestricted enumeration was exhausted, for example by cancellation or a
    /// search limit. A partial fold establishes nothing.
    NotExhausted,
    /// The accumulated consequence set could not be constructed.
    Consequences(ModelError),
    /// A witness answer set could not be retained.
    Allocation,
}

impl fmt::Display for WorldViewDecisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solve(error) => error.fmt(f),
            Self::NotExhausted => {
                f.write_str("answer-set search stopped before the question was decided")
            }
            Self::Consequences(error) => {
                write!(f, "world-view consequence set could not be built: {error}")
            }
            Self::Allocation => f.write_str("world-view witness could not be retained"),
        }
    }
}

impl std::error::Error for WorldViewDecisionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Solve(error) => Some(error.as_ref()),
            Self::Consequences(error) => Some(error),
            Self::NotExhausted | Self::Allocation => None,
        }
    }
}

/// A world-view question that was not decided, with the subject and the
/// session evidence available. No partial fold is retained.
#[derive(Debug)]
pub struct WorldViewDecisionFailure {
    cause: WorldViewDecisionError,
    subject: Subject,
    outcome: Option<Box<SemanticOutcome>>,
}

impl WorldViewDecisionFailure {
    /// Original setup, execution, coverage or accumulation refusal.
    #[must_use]
    pub const fn cause(&self) -> &WorldViewDecisionError {
        &self.cause
    }

    /// Exact original subject, including when setup failed before any answer.
    #[must_use]
    pub const fn subject(&self) -> &Subject {
        &self.subject
    }

    /// Available session evidence.
    #[must_use]
    pub fn outcome(&self) -> Option<&SemanticOutcome> {
        self.outcome.as_deref()
    }
}

impl fmt::Display for WorldViewDecisionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}

impl std::error::Error for WorldViewDecisionFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

impl SessionBuilder<'_> {
    /// Decide whether a literal holds in every answer set of the program: the
    /// epistemic `K` of the literal over its world view. A literal is an atom
    /// or a strongly negated atom, whose predicate carries the negative sign.
    ///
    /// The request is [`Self::collect`]'s, full answer identities and
    /// [`AnswerSelection::All`], with any requested model count cleared, so
    /// neither a projection, an objective's selection nor `config.models`
    /// narrows the family; collection instead refuses a count that stops it
    /// early. Answer sets
    /// are folded as they arrive, and the session stops at the first one
    /// without the literal, which decides `false` with that answer set as its
    /// witness. Exhaustion without one gives `true` over the complete family;
    /// an exhausted empty family is [`WorldViewVerdict::Inconsistent`].
    ///
    /// Work is the enumeration up to the deciding answer set or exhaustion,
    /// plus one logarithmic membership test per answer set. Retained space is
    /// at most one answer set; no family is collected.
    ///
    /// # Errors
    /// Returns the session's typed setup or execution failure, or
    /// [`WorldViewDecisionError::NotExhausted`] when it stops undecided.
    pub fn known(
        self,
        literal: &Atom,
    ) -> Result<WorldViewDecision<bool>, WorldViewDecisionFailure> {
        decide(self, Membership::every(literal))
    }

    /// Decide whether a literal holds in some answer set of the program: the
    /// epistemic `M` of the literal over its world view.
    ///
    /// The request is [`Self::known`]'s. The session stops at the first answer
    /// set containing the literal, which decides `true` with that answer set as
    /// its witness. Exhaustion without one gives `false` over the complete
    /// family; an exhausted empty family is [`WorldViewVerdict::Inconsistent`].
    /// Work and retained space are [`Self::known`]'s.
    ///
    /// # Errors
    /// Returns [`Self::known`]'s failures.
    pub fn possible(
        self,
        literal: &Atom,
    ) -> Result<WorldViewDecision<bool>, WorldViewDecisionFailure> {
        decide(self, Membership::some(literal))
    }

    /// The program's cautious consequences: the atoms true in every answer
    /// set, the intersection of the world view's full answer sets, including
    /// atoms hidden by display directives.
    ///
    /// The request is [`Self::known`]'s. Once the intersection is empty the
    /// session stops: nothing is cautious, whatever answer sets remain, and the
    /// decided verdict names no witnesses. Otherwise exhaustion gives the
    /// intersection over the complete family; an exhausted empty family is
    /// [`WorldViewVerdict::Inconsistent`], never the set of every atom.
    ///
    /// Work per answer set is one logarithmic membership test per atom still
    /// in the intersection. Retained space is the intersection, a selection of
    /// the first answer set's atom catalog; no answer set is retained.
    ///
    /// # Errors
    /// Returns [`Self::known`]'s failures, and
    /// [`WorldViewDecisionError::Consequences`] when the intersection cannot be
    /// built.
    pub fn cautious(self) -> Result<WorldViewDecision<Model>, WorldViewDecisionFailure> {
        decide(self, Consequences::cautious())
    }

    /// The program's brave consequences: the atoms true in some answer set,
    /// the union of the world view's full answer sets, including atoms hidden
    /// by display directives.
    ///
    /// The request is [`Self::known`]'s. The union's absorbing value is the set
    /// of every atom, which a session does not bound, so the question is
    /// decided only by exhaustion: the union over the complete family, or
    /// [`WorldViewVerdict::Inconsistent`] for an empty one.
    ///
    /// Work per answer set is a merge of the union with its atoms: positions
    /// when both share one atom catalog, otherwise owned copies of the atoms.
    /// Retained space is the union; no answer set is retained.
    ///
    /// # Errors
    /// Returns [`Self::known`]'s failures, and
    /// [`WorldViewDecisionError::Consequences`] when the union cannot be built
    /// or an atom cannot be copied between catalogs.
    pub fn brave(self) -> Result<WorldViewDecision<Model>, WorldViewDecisionFailure> {
        decide(self, Consequences::brave())
    }

    /// The three-valued answer to a ground literal query over the program's
    /// world view: [`QueryAnswer::Yes`] when the literal is true in every
    /// answer set, [`QueryAnswer::No`] when it is false in every one, and
    /// [`QueryAnswer::Unknown`] otherwise. Within one answer set the literal is
    /// true if present, false if its strong contrary is present, and unknown
    /// otherwise.
    ///
    /// The request is [`Self::known`]'s. Answer sets that together leave the
    /// literal not true and not false decide `Unknown`, and the session stops:
    /// they are the witnesses, one answer set in which the literal is unknown
    /// or the first two that left it not true and not false. Otherwise
    /// exhaustion decides `Yes` or `No` over the complete family; an exhausted
    /// empty family is [`WorldViewVerdict::Inconsistent`].
    ///
    /// Work is the enumeration up to the deciding answer sets or exhaustion,
    /// plus two logarithmic membership tests per answer set. Retained space is
    /// at most two answer sets.
    ///
    /// # Errors
    /// Returns [`Self::known`]'s failures.
    pub fn answer(
        self,
        literal: &Atom,
    ) -> Result<WorldViewDecision<QueryAnswer>, WorldViewDecisionFailure> {
        decide(self, Query::new(literal))
    }
}

/// What folding one answer set did to the accumulator.
enum Step {
    /// The value is not yet decided.
    Open,
    /// The accumulator reached the absorbing value.
    Absorbed,
}

/// One reduction folded over answer sets, with its absorbing test and its
/// witnesses. Each implementation is one of `Zetesis.AxisFolds`'s reductions.
trait Fold {
    type Value;

    /// Fold one checked answer set into the accumulator.
    fn step(&mut self, answer: AnswerSet) -> Result<Step, WorldViewDecisionError>;

    /// The absorbed value and the answer sets that drove the fold to it.
    fn decided(self) -> (Self::Value, Vec<AnswerSet>);

    /// The value over every folded answer set; `None` when none was folded.
    fn complete(self) -> Option<Self::Value>;
}

/// Run a fold over the unrestricted request's answer sets, stopping at
/// absorption, and conclude only under `WorldView::collect`'s exhaustion
/// predicate otherwise.
fn decide<F: Fold>(
    request: SessionBuilder<'_>,
    mut fold: F,
) -> Result<WorldViewDecision<F::Value>, WorldViewDecisionFailure> {
    // Owning an unstarted request ensures that every answer set of the session
    // reaches the fold, as `WorldView::collect_request` requires of collection.
    let subject = request.subject();
    let mut session = request
        .full_identity()
        .every_answer()
        .selection(AnswerSelection::All)
        .start_with(&mut Ignore)
        .map_err(|error| WorldViewDecisionFailure {
            outcome: error.semantic().cloned().map(Box::new),
            cause: WorldViewDecisionError::Solve(Box::new(error)),
            subject: subject.clone(),
        })?;
    while let Some(result) = session.pull(&mut Ignore) {
        let step = result
            .map_err(|error| WorldViewDecisionError::Solve(Box::new(error)))
            .and_then(|answer| fold.step(answer));
        match step {
            Ok(Step::Open) => {}
            Ok(Step::Absorbed) => {
                // Absorption decides the world view's value whatever answer
                // sets remain (`AxisFolds.world_view_fold_decided`).
                let outcome = session.stop();
                let (value, witnesses) = fold.decided();
                return Ok(WorldViewDecision {
                    subject,
                    verdict: WorldViewVerdict::Decided { value, witnesses },
                    outcome,
                });
            }
            Err(cause) => {
                return Err(WorldViewDecisionFailure {
                    cause,
                    subject,
                    outcome: Some(Box::new(session.stop())),
                });
            }
        }
    }
    let outcome = session.stop();
    if outcome.completion() != Some(Completion::Exhausted)
        || outcome.selection() != Some(AnswerSelection::All)
    {
        return Err(WorldViewDecisionFailure {
            cause: WorldViewDecisionError::NotExhausted,
            subject,
            outcome: Some(Box::new(outcome)),
        });
    }
    let verdict = fold
        .complete()
        .map_or(WorldViewVerdict::Inconsistent, WorldViewVerdict::Complete);
    Ok(WorldViewDecision {
        subject,
        verdict,
        outcome,
    })
}

/// Retain one witness, reserving its slot fallibly.
fn witness(
    witnesses: &mut Vec<AnswerSet>,
    answer: AnswerSet,
) -> Result<(), WorldViewDecisionError> {
    witnesses
        .try_reserve(1)
        .map_err(|_| WorldViewDecisionError::Allocation)?;
    witnesses.push(answer);
    Ok(())
}

/// Whether a literal is in every answer set (`all`, absorbed by `false`) or in
/// some answer set (`any`, absorbed by `true`).
struct Membership {
    literal: Atom,
    /// The membership that absorbs: `false` for every, `true` for some.
    absorber: bool,
    folded: bool,
    witnesses: Vec<AnswerSet>,
}

impl Membership {
    fn every(literal: &Atom) -> Self {
        Self {
            literal: literal.clone(),
            absorber: false,
            folded: false,
            witnesses: Vec::new(),
        }
    }

    fn some(literal: &Atom) -> Self {
        Self {
            absorber: true,
            ..Self::every(literal)
        }
    }
}

impl Fold for Membership {
    type Value = bool;

    fn step(&mut self, answer: AnswerSet) -> Result<Step, WorldViewDecisionError> {
        self.folded = true;
        if answer.interpretation().contains(&self.literal) != self.absorber {
            return Ok(Step::Open);
        }
        witness(&mut self.witnesses, answer)?;
        Ok(Step::Absorbed)
    }

    fn decided(self) -> (bool, Vec<AnswerSet>) {
        (self.absorber, self.witnesses)
    }

    fn complete(self) -> Option<bool> {
        // Without absorption every answer set held the other membership.
        self.folded.then_some(!self.absorber)
    }
}

/// Cautious consequences (intersection, absorbed by the empty set) or brave
/// consequences (union, whose absorbing set of every atom is not observable).
struct Consequences {
    atoms: Option<Model>,
    cautious: bool,
}

impl Consequences {
    const fn cautious() -> Self {
        Self {
            atoms: None,
            cautious: true,
        }
    }

    const fn brave() -> Self {
        Self {
            atoms: None,
            cautious: false,
        }
    }
}

impl Fold for Consequences {
    type Value = Model;

    fn step(&mut self, answer: AnswerSet) -> Result<Step, WorldViewDecisionError> {
        let next = match self.atoms.take() {
            None => Ok(answer.into_interpretation()),
            Some(atoms) if self.cautious => intersection(&atoms, answer.interpretation()),
            Some(atoms) => union(&atoms, answer.interpretation()),
        }
        .map_err(WorldViewDecisionError::Consequences)?;
        let absorbed = self.cautious && next.atoms().is_empty();
        self.atoms = Some(next);
        Ok(if absorbed { Step::Absorbed } else { Step::Open })
    }

    fn decided(self) -> (Model, Vec<AnswerSet>) {
        // Only the empty intersection absorbs, and it names no witnesses.
        (self.atoms.unwrap_or_default(), Vec::new())
    }

    fn complete(self) -> Option<Model> {
        self.atoms
    }
}

/// The atoms of `atoms` that `answer` also contains, as a selection of the
/// accumulator's own catalog.
fn intersection(atoms: &Model, answer: &Model) -> Result<Model, ModelError> {
    let kept = atoms
        .positions()
        .iter()
        .zip(atoms.atoms().iter())
        .filter(|&(_, atom)| answer.contains(atom))
        .map(|(&position, _)| position);
    Model::from_positions(atoms.catalog(), kept)
}

/// The atoms of either model. Positions are merged when both select from one
/// catalog; otherwise both sequences are merged in canonical order into owned
/// atoms, each copied once.
fn union(atoms: &Model, answer: &Model) -> Result<Model, ModelError> {
    if atoms.catalog().same_owner(answer.catalog()) {
        let positions = atoms.positions().iter().chain(answer.positions()).copied();
        return Model::from_positions(atoms.catalog(), positions);
    }
    let mut merged = Vec::new();
    let capacity = atoms
        .atoms()
        .len()
        .checked_add(answer.atoms().len())
        .ok_or(ModelError::Allocation)?;
    merged
        .try_reserve_exact(capacity)
        .map_err(|_| ModelError::Allocation)?;
    let mut left = atoms.atoms().iter().peekable();
    let mut right = answer.atoms().iter().peekable();
    loop {
        let next = match (left.peek(), right.peek()) {
            (None, None) => break,
            (Some(_), None) => left.next(),
            (None, Some(_)) => right.next(),
            (Some(first), Some(second)) => match first.cmp(second) {
                std::cmp::Ordering::Less => left.next(),
                std::cmp::Ordering::Greater => right.next(),
                std::cmp::Ordering::Equal => {
                    right.next();
                    left.next()
                }
            },
        };
        if let Some(atom) = next {
            merged.push(
                atom.to_atom(ValueLimits::default())
                    .map_err(ModelError::Catalog)?,
            );
        }
    }
    Model::from_ordered(merged)
}

/// The three-valued answer: two flags under componentwise disjunction, whether
/// some answer set leaves the literal not true and whether some leaves it not
/// false, absorbed when both are set.
struct Query {
    literal: Atom,
    contrary: Atom,
    folded: bool,
    not_true: bool,
    not_false: bool,
    witnesses: Vec<AnswerSet>,
}

impl Query {
    fn new(literal: &Atom) -> Self {
        Self {
            literal: literal.clone(),
            contrary: literal.contrary(),
            folded: false,
            not_true: false,
            not_false: false,
            witnesses: Vec::new(),
        }
    }
}

impl Fold for Query {
    type Value = QueryAnswer;

    fn step(&mut self, answer: AnswerSet) -> Result<Step, WorldViewDecisionError> {
        self.folded = true;
        let model = answer.interpretation();
        let is_true = model.contains(&self.literal);
        let is_false = !is_true && model.contains(&self.contrary);
        // An answer set is a witness when it sets a flag not yet set, so at
        // most two are retained, and one when the literal is unknown in it.
        let sets_not_true = !self.not_true && !is_true;
        let sets_not_false = !self.not_false && !is_false;
        self.not_true |= !is_true;
        self.not_false |= !is_false;
        if sets_not_true || sets_not_false {
            witness(&mut self.witnesses, answer)?;
        }
        Ok(if self.not_true && self.not_false {
            Step::Absorbed
        } else {
            Step::Open
        })
    }

    fn decided(self) -> (QueryAnswer, Vec<AnswerSet>) {
        (QueryAnswer::Unknown, self.witnesses)
    }

    fn complete(self) -> Option<QueryAnswer> {
        self.folded.then_some(if !self.not_true {
            QueryAnswer::Yes
        } else if !self.not_false {
            QueryAnswer::No
        } else {
            QueryAnswer::Unknown
        })
    }
}
