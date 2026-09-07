//! Checked tight normal/choice execution over a complete immutable formula DAG.

mod compile;
mod evaluate;

use std::fmt;

use crate::Theory;
use zetesis_cpu::Stop;

/// Admission bounds for the optional class certificate. Logical bytes cover the
/// plan and all construction vectors, excluding the borrowed theory, allocator
/// overhead and stack. A refusal leaves the general reduct algorithm available.
#[derive(Clone, Copy, Debug)]
pub struct TightPlanLimits {
    /// Maximum normal or choice producers, including repeated original roots.
    pub max_producers: usize,
    /// Maximum edges in the positive formula/atom dependency graph.
    pub max_dependencies: usize,
    /// Logical payload bytes for construction and the retained plan.
    pub max_bytes: u64,
    /// Charged node, root, edge and rank operations during certification.
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
    /// Logical bytes for the retained plan and candidate evaluation scratch.
    pub max_bytes: u64,
    /// Charged node evaluations, original-root tests, producers and atom scans.
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
/// share identity, while independent equal theories do not. This certificate
/// says nothing about missing source rules or an incomplete lazy registry.
#[derive(Debug)]
pub struct TightPlan {
    theory: Theory,
    producers: Vec<TightProducer>,
    ranks: Vec<usize>,
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
    /// Plan and evaluation scratch logical payload bytes.
    pub logical_bytes: u64,
}

struct Work<'a> {
    used: u64,
    max: u64,
    control: &'a zetesis_cpu::Control,
}
impl Work<'_> {
    fn tick(&mut self) -> Result<(), TightError> {
        self.control.poll()?;
        if self.used == self.max {
            return Err(TightError::Limit(TightResource::Work));
        }
        self.used += 1;
        Ok(())
    }
}

fn bytes(value: u128, limit: u64) -> Result<u64, TightError> {
    let value = u64::try_from(value).map_err(|_| TightError::Limit(TightResource::Bytes))?;
    if value > limit {
        return Err(TightError::Limit(TightResource::Bytes));
    }
    Ok(value)
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
