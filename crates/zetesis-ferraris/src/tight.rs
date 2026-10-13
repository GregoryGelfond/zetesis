//! Checked tight normal/choice execution over a complete immutable formula DAG.

mod compile;
mod evaluate;
mod readers;
#[cfg(test)]
mod tests;

pub use evaluate::TightWorkspace;

use std::fmt;

use crate::Theory;
use zetesis_cpu::{Cancellation, ChargeError, Meter, Refusal, Stop};

/// Admission bounds for the optional class certificate. Logical bytes cover the
/// plan and all construction vectors, excluding the borrowed theory, allocator
/// overhead and stack. A refusal leaves the general reduct algorithm available.
#[derive(Clone, Copy, Debug)]
pub struct TightPlanLimits {
    /// Maximum normal or choice producers, including repeated original roots.
    pub max_producers: usize,
    /// Maximum edges in the positive formula/atom dependency graph.
    pub max_dependencies: usize,
    /// Logical payload bytes for construction and the retained plan, including
    /// the readers index [`TightPlan::prepare_updates`] adds to it.
    pub max_bytes: u64,
    /// Charged node, root, edge and rank operations during certification, and
    /// the node, operand, condition, producer and root visits of each pass
    /// that builds the readers index.
    pub max_work: u64,
}

impl Default for TightPlanLimits {
    fn default() -> Self {
        Self {
            max_producers: 262_144,
            max_dependencies: 4_194_304,
            max_bytes: 256 * 1024 * 1024,
            max_work: 100_000_000,
        }
    }
}

/// Per-candidate bounds, independent of enumeration and exact-query budgets.
#[derive(Clone, Copy, Debug)]
pub struct TightCheckLimits {
    /// Logical bytes for the retained plan and actual workspace-vector
    /// capacities, including the evaluation a workspace keeps for its next check
    /// and capacity retained from earlier checks. Vector headers, the shared
    /// theory and allocator overhead are excluded.
    pub max_bytes: u64,
    /// Charged node evaluations, visited operands, original-root tests,
    /// producers and packed candidate-word scans. With the plan's readers
    /// index, a workspace also charges each candidate word and queued-mark word
    /// it keeps for the next check, and an update charges each candidate word
    /// it compares, each changed word and atom, each atom-node and reader entry
    /// it visits, each guard it queues, each node it re-evaluates with that
    /// node's operands, the producers, roots and words it summarizes once per
    /// complete evaluation, and the words its verdict scans.
    pub max_work: u64,
}

impl Default for TightCheckLimits {
    fn default() -> Self {
        Self {
            max_bytes: 256 * 1024 * 1024,
            max_work: 100_000_000,
        }
    }
}

/// The particular certificate resource whose admission stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TightResource {
    /// Producer count.
    Producers,
    /// Positive dependency edges.
    Dependencies,
    /// Logical storage payload.
    Bytes,
    /// Charged certification or evaluation work.
    Work,
}

/// A refused optional certificate or incomplete evaluation. None is UNSAT.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TightError {
    /// An original root is outside the checked normal/choice/negation grammar.
    UnsupportedRoot {
        /// Original asserted node that remains unaccounted for.
        root: usize,
    },
    /// A producer contains an unnegated implication in its body.
    UnsupportedBody {
        /// Original asserted producer root.
        root: usize,
        /// Its body node, whose checked nested-expression classification failed.
        body: usize,
    },
    /// Positive dependencies are cyclic; no rank certificate exists.
    PositiveCycle {
        /// An atom whose incoming dependency remains after topological removal.
        atom: usize,
    },
    /// An external rank vector has a wrong length or an out-of-range rank.
    RankShape,
    /// An external rank fails a checked producer dependency.
    RankOrder {
        /// Producer atom that does not strictly exceed its positive inputs.
        head: usize,
    },
    /// Explicit optimization resource ceiling.
    Limit(TightResource),
    /// Shared control, identity or allocation refusal.
    Stopped(Stop),
}

impl fmt::Display for TightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedRoot { root } => write!(f, "root {root} has no tight producer plan"),
            Self::UnsupportedBody { root, body } => {
                write!(f, "root {root} has an unsupported nested body at {body}")
            }
            Self::PositiveCycle { atom } => {
                write!(f, "positive dependency cycle reaches atom {atom}")
            }
            Self::RankShape => f.write_str("tight rank vector has an invalid shape"),
            Self::RankOrder { head } => write!(f, "tight rank does not cover producer atom {head}"),
            Self::Limit(resource) => write!(f, "tight plan {resource:?} limit reached"),
            Self::Stopped(stop) => stop.fmt(f),
        }
    }
}
impl std::error::Error for TightError {}
impl From<Stop> for TightError {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

impl Refusal for TightError {
    fn is_resource_refusal(&self) -> bool {
        match self {
            Self::Limit(_) => true,
            Self::Stopped(stop) => stop.is_resource_refusal(),
            Self::UnsupportedRoot { .. }
            | Self::UnsupportedBody { .. }
            | Self::PositiveCycle { .. }
            | Self::RankShape
            | Self::RankOrder { .. } => false,
        }
    }

    fn from_stop(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

/// A producer's original head form. Choice has the original `h ∨ not h`
/// consequent, whose reduct requires h exactly when the candidate contains h.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TightProducerKind {
    /// Ordinary atomic head.
    Normal,
    /// Atomic choice head; no rewriting of the original theory is performed.
    Choice,
}

/// An immutable producer extracted from one exact original root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TightProducer {
    pub(super) head: usize,
    pub(super) body: Option<usize>,
    pub(super) root: usize,
    pub(super) kind: TightProducerKind,
}
impl TightProducer {
    /// Semantic atom index supplied by the producer.
    #[must_use]
    pub fn head(self) -> usize {
        self.head
    }
    /// Original body node, or `None` for a fact/unconditional choice.
    #[must_use]
    pub fn body(self) -> Option<usize> {
        self.body
    }
    /// Asserted original root from which this producer was extracted.
    #[must_use]
    pub fn root(self) -> usize {
        self.root
    }
    /// Original ordinary or choice head form.
    #[must_use]
    pub fn kind(self) -> TightProducerKind {
        self.kind
    }
}

/// Construction accounting for a successfully checked class certificate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TightPlanStatistics {
    /// Charged certification operations.
    pub work: u64,
    /// Positive dependency graph edges, including formula-node edges.
    pub dependencies: usize,
    /// Conservative simultaneous logical payload bound during construction.
    pub construction_bytes: u64,
    /// Retained producer and rank payload; shared theory storage is excluded.
    pub resident_bytes: u64,
}

/// A checked complete-theory certificate for tight normal/choice producers.
///
/// Every original root is either an extracted producer or a frozen constraint.
/// Every unnegated producer-body atom has strictly lower rank than the head.
/// Construction is the only way to obtain this value; clones of its theory
/// share identity, while independent equal theories do not. The plan owns its
/// readers index: [`Self::prepare_updates`] adds it while the plan's owner holds
/// it exclusively, before sharing it, and a plan shared without the index keeps
/// checking every candidate completely. This certificate
/// says nothing about missing source rules or an incomplete lazy registry.
#[derive(Debug)]
pub struct TightPlan {
    theory: Theory,
    producers: Vec<TightProducer>,
    ranks: Vec<usize>,
    /// Who reads each node's truth, present once [`Self::prepare_updates`]
    /// built it; a reused workspace then updates its retained evaluation.
    readers: Option<readers::Readers>,
    statistics: TightPlanStatistics,
}
impl TightPlan {
    /// Complete immutable theory whose original roots the certificate covers.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        &self.theory
    }
    /// Original normal and choice producers in root order.
    #[must_use]
    pub fn producers(&self) -> &[TightProducer] {
        &self.producers
    }
    /// Checked positive rank for each atom, including unsupported carrier atoms.
    #[must_use]
    pub fn ranks(&self) -> &[usize] {
        &self.ranks
    }
    /// Certificate construction and retained payload accounting.
    #[must_use]
    pub fn statistics(&self) -> TightPlanStatistics {
        self.statistics
    }

    /// Build the reverse index that lets a reused [`TightWorkspace`] bring its
    /// retained complete evaluation up to the next candidate instead of
    /// evaluating it completely. Two charged passes over nodes, operands,
    /// numeric conditions, producers and root positions; the index is checked
    /// against `limits.max_bytes` together with the plan's resident bytes before
    /// its entries are allocated. Returns whether the index exists afterwards: a
    /// plan whose index does not fit keeps complete evaluation, and building it
    /// again is a no-op. Construction work and resident bytes include the index.
    ///
    /// # Errors
    /// Refuses the work limit, cancellation, deadlines and allocation; the plan
    /// is unchanged then.
    pub fn prepare_updates(
        &mut self,
        limits: TightPlanLimits,
        cancellation: &Cancellation,
    ) -> TightAttempt<bool> {
        if self.readers.is_some() {
            return TightAttempt {
                result: Ok(true),
                work: 0,
            };
        }
        let base = u128::from(self.statistics.resident_bytes);
        let metered =
            cancellation.metered(limits.max_work, |meter| {
                match readers::Readers::build(
                    &self.theory,
                    &self.producers,
                    base,
                    limits.max_bytes,
                    meter,
                ) {
                    Ok(readers) => Ok(Some(readers)),
                    Err(TightError::Limit(TightResource::Bytes)) => Ok(None),
                    Err(error) => Err(error),
                }
            });
        let result = metered.result.and_then(|readers| {
            let Some(readers) = readers else {
                return Ok(false);
            };
            self.statistics.resident_bytes = bytes(base + readers.bytes(), limits.max_bytes)?;
            self.statistics.work = self
                .statistics
                .work
                .checked_add(metered.charged)
                .ok_or(TightError::Limit(TightResource::Work))?;
            self.readers = Some(readers);
            Ok(true)
        });
        TightAttempt {
            result,
            work: metered.charged,
        }
    }

    /// The most work any one [`TightWorkspace`] check can charge under this
    /// plan: the larger of its complete route's and its update route's terms.
    /// Without the readers index every check evaluates completely. With it, a
    /// check that retained an evaluation first counts the changed atoms, one
    /// charged operation per candidate word, and then chooses: when more atoms
    /// changed than an update admits (an eighth of the atoms, at least 64), it
    /// evaluates completely and keeps the result, so the complete route's term
    /// includes that counting pass; otherwise it derives the summaries if they
    /// are missing, applies the changed words and propagates. An update
    /// evaluates each node at most once, reads each operand and numeric
    /// condition at most once per evaluation, and visits each reader entry and
    /// each guard of a withdrawn row at most once per change of its input.
    #[must_use]
    pub fn check_work_bound(&self) -> u128 {
        self.complete_work_bound()
            .max(self.update_work_bound().unwrap_or(0))
    }

    /// The most work a complete check charges: every node, operand occurrence
    /// and root, numeric cache resets, every producer and every 32-bit support
    /// word. With the readers index it may follow a counting pass that found
    /// too many changed atoms, and it keeps its candidate's words and clears
    /// the queued marks.
    fn complete_work_bound(&self) -> u128 {
        let theory = &self.theory;
        let complete = crate::EvaluationWorkspace::work_bound(theory)
            + self.producers.len() as u128
            + theory.atom_count().div_ceil(32) as u128;
        if self.readers.is_none() {
            return complete;
        }
        let words = theory.atom_count().div_ceil(64) as u128;
        complete + 2 * words + theory.nodes().len().div_ceil(64) as u128
    }

    /// The most work an update charges, when the plan has the readers index:
    /// the summaries derived once per complete evaluation, the difference's
    /// two passes over the candidate words with each changed atom and its atom
    /// nodes, propagation, and the verdict's word scans.
    fn update_work_bound(&self) -> Option<u128> {
        let readers = self.readers.as_ref()?;
        let theory = &self.theory;
        let atoms = theory.atom_count() as u128;
        let words = theory.atom_count().div_ceil(64) as u128;
        let nodes = theory.nodes().len() as u128;
        let roots = theory.roots().len() as u128;
        let occurrences = theory.parts().occurrences() as u128;
        let summaries = self.producers.len() as u128 + roots + words;
        let difference = 2 * words + atoms + readers.atom_node_count() as u128;
        let propagation = 2 * nodes + 2 * occurrences + readers.reader_count() as u128;
        let verdict = theory.roots().len().div_ceil(64) as u128 + words;
        Some(summaries + difference + propagation + verdict)
    }
}

/// Sound partial membership result suitable for exact residual completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TightVerdict {
    /// Original satisfaction and ranked support prove reduct minimality.
    Stable,
    /// Original satisfaction failed at an asserted root.
    NotModel {
        /// Original false asserted node.
        root: usize,
    },
    /// A present atom has no true producer. No countermodel is published;
    /// callers using a witness-based rejection protocol must complete this case.
    Residual {
        /// First unsupported candidate atom in ascending order.
        unsupported_atom: usize,
    },
}

/// Completed candidate evaluation and charged logical work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TightCheck {
    /// A sound exact success or original failure, or a request for completion.
    pub verdict: TightVerdict,
    /// Charged operations actually performed before this result.
    pub work: u64,
    /// Plan and admitted evaluation-vector payload bytes. This includes the
    /// complete support vector even when an earlier original-root failure
    /// makes its allocation unnecessary; retained payload is available from
    /// [`TightWorkspace::retained_bytes`].
    pub logical_bytes: u64,
}

/// A certificate operation with work retained even when the result is refused.
/// The result alone cannot establish membership when it is an error. Callers
/// composing algorithms must charge `work` before selecting a fallback.
#[derive(Debug)]
pub struct TightAttempt<T> {
    /// Completed value or the exact reason the operation stopped.
    pub result: Result<T, TightError>,
    /// Charged primitive operations, including work preceding failure.
    pub work: u64,
}

/// A certificate operation's charge, in the certificate's error vocabulary.
///
/// The other metered contexts name this charge `tick` on a context that holds
/// their meter. The certificate has no such context: its operations take the
/// meter itself, so a per-node loop reaches the count through one reference
/// instead of two, which a complete check of a wide theory measurably feels.
trait Tick {
    fn tick(&mut self) -> Result<(), TightError>;
}

impl Tick for Meter<'_> {
    #[inline]
    fn tick(&mut self) -> Result<(), TightError> {
        self.charge().map_err(|error| match error {
            ChargeError::Stopped(stop) => stop.into(),
            ChargeError::WorkLimit => TightError::Limit(TightResource::Work),
        })
    }
}

/// Admit `value` logical bytes under `limit`.
fn bytes(value: u128, limit: u64) -> Result<u64, TightError> {
    match u64::try_from(value) {
        Ok(value) if value <= limit => Ok(value),
        _ => Err(TightError::Limit(TightResource::Bytes)),
    }
}

fn reserve<T>(count: usize) -> Result<Vec<T>, TightError> {
    let mut vector = Vec::new();
    vector
        .try_reserve_exact(count)
        .map_err(|_| Stop::Allocation)?;
    Ok(vector)
}

fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>, TightError> {
    let mut vector = reserve(count)?;
    vector.resize(count, value);
    Ok(vector)
}
