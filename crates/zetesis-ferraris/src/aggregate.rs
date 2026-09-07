//! Finite scalar aggregate compilation with reduct-preserving connectives.

mod lower;
mod extremum;
mod family;

use std::fmt;

pub use extremum::{AggregateExtremum, ExtremumBound, append_extremum};
pub use family::{
    AggregateFamilyBuild, AggregateFamilyLimits, AggregateGuard, append_aggregate_family,
};
pub use lower::append_aggregate;

/// One distinct, already coalesced tuple's weight and eligibility formula.
/// Callers OR every alternative eligibility condition for an equal whole tuple.
/// Equal weights or equal conditions alone do not identify equal tuples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AggregateElement {
    /// Integer first tuple component for `#sum`, `#min` or `#max`;
    /// use one for `#count`.
    pub weight: i32,
    /// Absolute index of an existing formula node, before this append call.
    pub condition: usize,
}

/// Comparison of an aggregate's value with one scalar guard.
/// These are aggregate operators; `Ne` is not default negation of `Eq`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateComparison {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Less than.
    Lt,
    /// Less than or equal.
    Le,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Ge,
}
impl AggregateComparison {
    pub(super) fn holds(self, sum: i128, bound: i128) -> bool {
        match self {
            Self::Eq => sum == bound,
            Self::Ne => sum != bound,
            Self::Lt => sum < bound,
            Self::Le => sum <= bound,
            Self::Gt => sum > bound,
            Self::Ge => sum >= bound,
        }
    }
}

/// Inclusive, per-append compilation ceilings. Zero never means unlimited.
#[derive(Clone, Copy, Debug)]
pub struct AggregateLimits {
    /// Maximum number of distinct input tuple elements.
    pub max_elements: usize,
    /// Maximum total DAG nodes, including the existing prefix.
    pub max_nodes: usize,
    /// Charged prefix checks, element/state/subset visits and node appends.
    pub max_work: u64,
    /// Maximum simultaneously retained algorithm cells, excluding DAG nodes.
    /// Threshold compilation uses two rows; general compilation uses subset bits.
    pub max_states: usize,
    /// Maximum subsets for exact general compilation, including the empty set.
    /// Threshold and extremum compilation do not consume this budget.
    pub max_subsets: u64,
}
impl Default for AggregateLimits {
    fn default() -> Self {
        Self {
            max_elements: 4_096,
            max_nodes: 1_048_576,
            max_work: 10_000_000,
            max_states: 1_048_576,
            max_subsets: 65_536,
        }
    }
}

/// Chosen exact translation; no profile introduces semantic atoms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateProfile {
    /// Nonnegative weighted threshold DAG with sharing of previous-row nodes.
    Threshold,
    /// Conjunction of a subset implication for every failing aggregate subset.
    SubsetImplications,
    /// Numeric min/max using filtered eligibility disjunctions and exact guards.
    Extremum,
}

/// Logical compilation accounting, retained on error as well as success.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregateStatistics {
    /// Charged operations, including validation of the existing DAG prefix.
    pub work: u64,
    /// Peak retained algorithm cells, excluding DAG nodes.
    pub states: usize,
    /// Enumerated complete subsets in the general translation.
    pub subsets: u64,
    /// Appended nodes; on error these nodes have been rolled back.
    pub nodes: usize,
}

/// A completed aggregate root in the caller's extended DAG.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AggregateBuild {
    pub(super) root: usize,
    pub(super) profile: AggregateProfile,
    pub(super) statistics: AggregateStatistics,
}
impl AggregateBuild {
    /// Absolute root index; no assertion or surrounding rule is added.
    #[must_use]
    pub const fn root(self) -> usize {
        self.root
    }
    /// Number of new nodes for the caller's parallel source-origin registry.
    #[must_use]
    pub const fn appended_nodes(self) -> usize {
        self.statistics.nodes
    }
    /// Translation profile selected from the aggregate and its input values.
    #[must_use]
    pub const fn profile(self) -> AggregateProfile {
        self.profile
    }
    /// Exact completed logical accounting.
    #[must_use]
    pub const fn statistics(self) -> AggregateStatistics {
        self.statistics
    }
}

/// A typed aggregate compilation refusal, never a logical contradiction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateErrorKind {
    /// Shared cancellation or deadline, preserved as supplied by the CPU control.
    Control(zetesis_cpu::Stop),
    /// Too many input elements.
    ElementLimit,
    /// Too many requested guards in one aggregate family.
    GuardLimit,
    /// Total node ceiling reached or already exceeded by the prefix.
    NodeLimit,
    /// Charged work ceiling reached.
    WorkLimit,
    /// Required temporary state exceeds its ceiling.
    StateLimit,
    /// The full general subset space exceeds its ceiling.
    SubsetLimit,
    /// An existing edge does not point to an earlier node.
    InvalidPrefix {
        /// Malformed existing node index.
        node: usize,
    },
    /// An element condition does not belong to the existing prefix.
    InvalidCondition {
        /// Input element index for the caller's source-origin registry.
        element: usize,
    },
    /// A storage reservation failed.
    Allocation,
    /// Exact arithmetic cannot be represented on this host.
    ArithmeticOverflow,
}

/// A refusal with partial accounting; the input DAG's length and nodes are intact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AggregateError {
    pub(super) kind: AggregateErrorKind,
    pub(super) statistics: AggregateStatistics,
}
impl AggregateError {
    /// Structured resource or input failure.
    #[must_use]
    pub const fn kind(self) -> AggregateErrorKind {
        self.kind
    }
    /// Accounting before rollback, without claiming a completed formula.
    #[must_use]
    pub const fn statistics(self) -> AggregateStatistics {
        self.statistics
    }
}
impl fmt::Display for AggregateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            AggregateErrorKind::Control(reason) => reason.fmt(f),
            AggregateErrorKind::ElementLimit => f.write_str("aggregate element limit exceeded"),
            AggregateErrorKind::GuardLimit => f.write_str("aggregate family guard limit exceeded"),
            AggregateErrorKind::NodeLimit => f.write_str("aggregate total node limit exceeded"),
            AggregateErrorKind::WorkLimit => f.write_str("aggregate work limit reached"),
            AggregateErrorKind::StateLimit => f.write_str("aggregate state limit exceeded"),
            AggregateErrorKind::SubsetLimit => f.write_str("aggregate subset limit exceeded"),
            AggregateErrorKind::InvalidPrefix { node } => {
                write!(f, "aggregate input DAG has a forward edge at node {node}")
            }
            AggregateErrorKind::InvalidCondition { element } => {
                write!(
                    f,
                    "aggregate element {element} references an absent condition"
                )
            }
            AggregateErrorKind::Allocation => f.write_str("aggregate storage reservation failed"),
            AggregateErrorKind::ArithmeticOverflow => {
                f.write_str("aggregate compilation arithmetic overflow")
            }
        }
    }
}
impl std::error::Error for AggregateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let AggregateErrorKind::Control(reason) = &self.kind {
            Some(reason)
        } else {
            None
        }
    }
}
