use zetesis_cpu::Cancellation;

use super::lower::{Builder, Destination, reserve, subsets, transaction_value, validate};
use super::{
    AggregateComparison, AggregateElement, AggregateError, AggregateErrorKind, AggregateLimits,
    AggregateProfile, AggregateStatistics,
};
use crate::{FormulaNodes, NodeView};

/// One scalar comparison against the same complete tuple eligibility family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AggregateGuard {
    /// Aggregate operator; not-equal is distinct from default-negated equality.
    pub comparison: AggregateComparison,
    /// Evaluated finite numeric guard.
    pub bound: i64,
}

/// Inclusive limits for one transaction and its ordered output-root vector.
#[derive(Clone, Copy, Debug)]
pub struct AggregateFamilyLimits {
    /// Shared node, work, element, temporary-state and cumulative subset limits.
    pub aggregate: AggregateLimits,
    /// Maximum requested guards, including duplicates; bounds output storage.
    pub max_guards: usize,
}
impl Default for AggregateFamilyLimits {
    fn default() -> Self {
        Self {
            aggregate: AggregateLimits::default(),
            max_guards: 4_096,
        }
    }
}

/// All requested aggregate roots from one completed, atomic compilation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateFamilyBuild {
    roots: Vec<usize>,
    profile: AggregateProfile,
    statistics: AggregateStatistics,
}
impl AggregateFamilyBuild {
    /// Absolute roots in input guard order; repeated guards retain entries.
    #[must_use]
    pub fn roots(&self) -> &[usize] {
        &self.roots
    }

    /// Bytes retained by the output-root vector: its inline vector header and
    /// actual allocation capacity. Excludes the shared DAG, temporary compiler
    /// state, and this build's profile and statistics fields.
    #[must_use]
    pub fn root_storage_bytes(&self) -> usize {
        size_of::<Vec<usize>>() + self.roots.capacity() * size_of::<usize>()
    }

    /// Exact shared translation profile.
    #[must_use]
    pub const fn profile(&self) -> AggregateProfile {
        self.profile
    }

    /// Total newly appended nodes for the caller's source-origin registry.
    #[must_use]
    pub const fn appended_nodes(&self) -> usize {
        self.statistics.nodes
    }

    /// Cumulative completed accounting across every requested comparison.
    #[must_use]
    pub const fn statistics(&self) -> AggregateStatistics {
        self.statistics
    }
}

/// Append an ordered family of finite `#count`/`#sum` comparisons atomically.
/// All guards share exactly the same already OR-coalesced complete tuples and
/// eligibility formulas. The caller establishes that this element family is
/// independent of the changing guard, including its source bindings and support
/// universe. This function checks indices and bounds, not that source premise.
///
/// For nonnegative weights, native threshold nodes share one contribution row.
/// Constant, conjunction and disjunction bounds retain their simpler formulas.
/// Equality uses `G(k) AND NOT G(k+1)`; not-equal retains `G(k) -> G(k+1)`.
/// Negative weights use the full failing-subset formula separately for each
/// guard, with shared validation and cumulative work/subset ceilings.
///
/// Prefix and input validation occur once. An empty guard family still validates
/// input and polls control, then returns no roots and appends no nodes. Output
/// root storage is bounded by `max_guards`; the temporary-state budget counts numeric columns, bounds
/// or subset bits and excludes this returned vector and DAG storage.
/// Every loop visit and node append is charged and polls cancellation/deadlines.
/// A limit, control, arithmetic or allocation error rolls back every new node;
/// no partial root vector is returned. Existing [`super::append_aggregate`] limits
/// and per-guard behavior are unchanged. Default negation remains a caller wrap.
///
/// # Errors
/// Returns typed input, guard, resource, allocation, cancellation or arithmetic
/// errors. Shared limits apply to the entire family, never independently per root.
pub fn append_aggregate_family(
    nodes: &mut FormulaNodes,
    elements: &[AggregateElement],
    guards: &[AggregateGuard],
    limits: AggregateFamilyLimits,
    cancellation: &Cancellation,
) -> Result<AggregateFamilyBuild, AggregateError> {
    append(
        Destination::retained(nodes.transaction()),
        elements,
        guards,
        limits,
        cancellation,
    )
}

pub(super) fn append(
    destination: Destination<'_>,
    elements: &[AggregateElement],
    guards: &[AggregateGuard],
    limits: AggregateFamilyLimits,
    cancellation: &Cancellation,
) -> Result<AggregateFamilyBuild, AggregateError> {
    transaction_value(destination, limits.aggregate, cancellation, |builder| {
        compile(builder, elements, guards, limits.max_guards)
    })
    .map(|((roots, profile), statistics)| AggregateFamilyBuild {
        roots,
        profile,
        statistics,
    })
}

type ResultKind<T> = Result<T, AggregateErrorKind>;

pub(super) fn compile(
    builder: &mut Builder<'_, '_>,
    elements: &[AggregateElement],
    guards: &[AggregateGuard],
    max_guards: usize,
) -> ResultKind<(Vec<usize>, AggregateProfile)> {
    let (nonnegative, total) = validate(builder, elements)?;
    if guards.len() > max_guards {
        return Err(AggregateErrorKind::GuardLimit);
    }
    let profile = if nonnegative {
        AggregateProfile::Threshold
    } else {
        AggregateProfile::SubsetImplications
    };
    if guards.is_empty() {
        return Ok((Vec::new(), profile));
    }
    let falsum = builder.push(NodeView::False)?;
    let truth = builder.push(NodeView::Implies(falsum, falsum))?;
    let roots = if nonnegative {
        super::monotone::compile(builder, elements, guards, total, falsum, truth)?
    } else {
        let mut roots = reserve(guards.len())?;
        for guard in guards {
            builder.tick()?;
            roots.push(subsets(
                builder,
                elements,
                guard.comparison,
                i128::from(guard.bound),
                falsum,
                truth,
            )?);
        }
        roots
    };
    Ok((roots, profile))
}
