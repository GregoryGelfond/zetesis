//! Retained finite aggregate operations over already-coalesced whole tuples.
//!
//! A group retains its complete keys, eligibility node indices and immutable
//! theory identity. Admission checks shape and uniqueness; it does not assert
//! that a source program or theory contains this aggregate. Mask reduction is
//! separate from acquisition of actual original/frozen formula truth. Neither
//! operation decides stable-model membership or changes the original theory.

use std::fmt;

use zetesis_core::Value as Term;
use zetesis_cpu::{Cancellation, Stop};

use crate::{AggregateComparison, Theory};

mod admission;
mod eligibility;
mod reduction;
pub use eligibility::{Eligibility, EligibilityLimits};
pub use reduction::{Evaluation, Reduction, ReductionLimits, Value};

/// Finite aggregate function; no floating-point arithmetic is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Function {
    /// Number of selected complete tuple keys, including an empty tuple.
    Count,
    /// Checked signed sum of selected numeric first components.
    Sum,
    /// Checked sum of strictly positive numeric first components.
    SumPlus,
    /// Minimum selected first component in ASP term order; empty result is `#sup`.
    Min,
    /// Maximum selected first component in ASP term order; empty result is `#inf`.
    Max,
}

/// One complete tuple and its already OR-coalesced eligibility formula.
#[derive(Debug)]
pub struct Tuple {
    /// Complete logical key. Equal weights or equal conditions do not identify
    /// equal keys. Empty keys remain present, even for a neutral contribution.
    pub key: Vec<Term>,
    /// Absolute node index in the group's original immutable theory.
    pub condition: usize,
}

/// An integer guard can exceed source i32 width without becoming a term sentinel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bound {
    /// Exact wide integer.
    Integer(i128),
    /// Ordered ASP term, including genuine `#inf` and `#sup` endpoints.
    Term(Term),
}

/// One aggregate comparison. Several guards are combined by conjunction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Guard {
    /// Aggregate comparison, not default negation of another comparison.
    pub comparison: AggregateComparison,
    /// Right-hand value of the comparison.
    pub bound: Bound,
}

/// Bounds on accepting transferred tuple/guard storage and duplicate checking.
#[derive(Clone, Copy, Debug)]
pub struct AdmissionLimits {
    /// Number of unique complete tuples, including neutral contributions.
    pub max_tuples: usize,
    /// Total scalar/structured value occurrences in complete keys.
    pub max_tuple_values: usize,
    /// Total preorder nodes across keys and term-valued guards; scalars count one.
    pub max_value_nodes: usize,
    /// Number of guards, including repeated guard occurrences.
    pub max_guards: usize,
    /// Logical transferred vector capacities, referenced term payload and
    /// duplicate-check scratch. Shared term payload is conservatively charged
    /// per occurrence; allocator overhead and extra string capacity are excluded.
    pub max_bytes: u64,
    /// Charged shape visits, index moves and comparison-carrier operations.
    pub max_work: u64,
}

impl Default for AdmissionLimits {
    fn default() -> Self {
        Self {
            max_tuples: 65_536,
            max_tuple_values: 262_144,
            max_value_nodes: 1_048_576,
            max_guards: 64,
            max_bytes: 64 * 1024 * 1024,
            max_work: 100_000_000,
        }
    }
}

/// Resource whose inclusive ceiling refused the operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    /// Complete tuple count.
    Tuples,
    /// Key value occurrences.
    TupleValues,
    /// Term preorder-node occurrences.
    ValueNodes,
    /// Guard occurrences.
    Guards,
    /// Logical storage payload.
    Bytes,
    /// Charged operation budget.
    Work,
}

/// Which eligibility observation has an invalid occurrence count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Truth of the original element formulas in M.
    Original,
    /// Truth in J of those formulas frozen in M.
    Frozen,
}

/// Typed refusal; none denotes aggregate truth, stable membership or UNSAT.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// An inclusive finite resource ceiling was exceeded.
    Limit(Resource),
    /// Eligibility references a node outside the retained theory.
    Condition {
        /// Input tuple occurrence.
        tuple: usize,
        /// Invalid formula node index.
        node: usize,
    },
    /// Equal whole keys have not been OR-coalesced by the caller.
    Duplicate {
        /// First input occurrence in deterministic index order.
        first: usize,
        /// Second equal-key input occurrence.
        second: usize,
    },
    /// A supplied eligibility mask has the wrong tuple occurrence count.
    Mask {
        /// Original or frozen mask.
        phase: Phase,
        /// Exact required count.
        expected: usize,
        /// Supplied count.
        actual: usize,
    },
    /// Original/tested interpretation belongs to a different admitted theory.
    WrongTheory,
    /// Checked integer, dimension or accounting overflow.
    Overflow,
    /// Cancellation, deadline or fallible allocation stopped the operation.
    Stopped(Stop),
}

/// Deterministic accounting local to one admission, acquisition or reduction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    /// Charged operations, including the prefix before a refusal.
    pub work: u64,
    /// Planned retained logical vector/value payload for this operation.
    pub resident_bytes: u64,
    /// Planned simultaneous retained and temporary payload; allocation may fail.
    pub peak_bytes: u64,
}

/// Incomplete operation retaining typed reason and accounted prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    kind: ErrorKind,
    statistics: Statistics,
}

impl Error {
    /// Typed refusal, separate from a completed false aggregate guard.
    #[must_use]
    pub const fn kind(self) -> ErrorKind {
        self.kind
    }

    /// Local operation accounting before the refusal.
    #[must_use]
    pub const fn statistics(self) -> Statistics {
        self.statistics
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ErrorKind::Stopped(stop) => stop.fmt(f),
            ErrorKind::Limit(resource) => write!(f, "native aggregate {resource:?} limit reached"),
            ErrorKind::Condition { tuple, node } => {
                write!(f, "aggregate tuple {tuple} references absent node {node}")
            }
            ErrorKind::Duplicate { first, second } => write!(
                f,
                "aggregate tuples {first} and {second} have an uncoalesced equal key"
            ),
            ErrorKind::Mask {
                phase,
                expected,
                actual,
            } => write!(
                f,
                "aggregate {phase:?} mask has {actual} occurrences; expected {expected}"
            ),
            ErrorKind::WrongTheory => {
                f.write_str("aggregate interpretation belongs to another theory")
            }
            ErrorKind::Overflow => {
                f.write_str("native aggregate arithmetic or accounting overflow")
            }
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let ErrorKind::Stopped(stop) = &self.kind {
            Some(stop)
        } else {
            None
        }
    }
}

/// Immutable finite operation with transferred keys and checked condition IDs.
/// Its existence establishes neither source completeness nor theory entailment.
#[derive(Debug)]
pub struct Group {
    theory: Theory,
    function: Function,
    tuples: Vec<Tuple>,
    guards: Vec<Guard>,
    first_costs: Vec<u64>,
    guard_costs: Vec<u64>,
    statistics: Statistics,
}

impl Group {
    /// Accept already-coalesced keys without altering their occurrence order.
    ///
    /// Transfers vectors; their prior allocation/construction is the caller's
    /// cost. Rejects equal full keys even if their conditions are identical.
    /// Duplicate checking uses two bounded index vectors and a bottom-up merge
    /// sort; comparisons charge both whole key carriers before term comparison.
    /// No formula lowering, search, source recognition or key coalescing occurs.
    /// The retained theory clone shares existing immutable storage.
    ///
    /// # Errors
    /// Refuses duplicate keys, condition IDs, resources, control or allocation.
    pub fn new(
        theory: &Theory,
        function: Function,
        tuples: Vec<Tuple>,
        guards: Vec<Guard>,
        limits: AdmissionLimits,
        cancellation: &Cancellation,
    ) -> Result<Self, Error> {
        admission::build(theory, function, tuples, guards, limits, cancellation)
    }

    /// Immutable formula subject defining all eligibility node identities.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        &self.theory
    }

    /// Original complete keys and condition IDs in exact input occurrence order.
    #[must_use]
    pub fn tuples(&self) -> &[Tuple] {
        &self.tuples
    }

    /// Guard occurrences; none are silently removed or reordered.
    #[must_use]
    pub fn guards(&self) -> &[Guard] {
        &self.guards
    }

    /// Aggregate function used by every reduction of this group.
    #[must_use]
    pub const fn function(&self) -> Function {
        self.function
    }

    /// Completed shape/uniqueness admission accounting.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.statistics
    }
}

struct Work<'a> {
    maximum: u64,
    cancellation: &'a Cancellation,
    statistics: Statistics,
}
impl Work<'_> {
    fn poll(&self) -> Result<(), ErrorKind> {
        self.cancellation.poll().map_err(ErrorKind::Stopped)
    }

    fn charge(&mut self, amount: u64) -> Result<(), ErrorKind> {
        self.poll()?;
        let remaining = self.maximum - self.statistics.work;
        if amount > remaining {
            return Err(ErrorKind::Limit(Resource::Work));
        }
        self.statistics.work += amount;
        Ok(())
    }

    fn failure(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            statistics: self.statistics,
        }
    }
}

fn storage<T>(count: usize) -> Result<Vec<T>, ErrorKind> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| ErrorKind::Stopped(Stop::Allocation))?;
    Ok(values)
}

fn bytes<T>(count: usize) -> Result<u64, ErrorKind> {
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(ErrorKind::Overflow)?;
    u64::try_from(bytes).map_err(|_| ErrorKind::Overflow)
}

fn add(left: u64, right: u64) -> Result<u64, ErrorKind> {
    left.checked_add(right).ok_or(ErrorKind::Overflow)
}
