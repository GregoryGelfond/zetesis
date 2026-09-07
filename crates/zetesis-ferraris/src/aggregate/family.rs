use zetesis_cpu::Control;

use super::lower::{Builder, reserve, subsets, transaction_value, validate};
use super::{
    AggregateComparison, AggregateElement, AggregateError, AggregateErrorKind, AggregateLimits,
    AggregateProfile, AggregateStatistics,
};
use crate::Node;

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
/// For nonnegative weights, one DP computes all monotone thresholds up to the
/// largest requested threshold that is neither constant true nor constant false.
/// Equality uses `G(k) AND NOT G(k+1)`; not-equal retains `G(k) -> G(k+1)`.
/// Negative weights use the full failing-subset formula separately for each
/// guard, with shared validation and cumulative work/subset ceilings.
///
/// Prefix and input validation occur once. An empty guard family still validates
/// input and polls control, then returns no roots and appends no nodes. Output
/// root storage is bounded by `max_guards`; the temporary-state budget counts DP
/// cells or subset bits and excludes this returned vector and DAG storage.
/// Every loop visit and node append is charged and polls cancellation/deadlines.
/// A limit, control, arithmetic or allocation error rolls back every new node;
/// no partial root vector is returned. Existing [`super::append_aggregate`] limits
/// and per-guard behavior are unchanged. Default negation remains a caller wrap.
///
/// # Errors
/// Returns typed input, guard, resource, allocation, cancellation or arithmetic
/// errors. Shared limits apply to the entire family, never independently per root.
pub fn append_aggregate_family(
    nodes: &mut Vec<Node>,
    elements: &[AggregateElement],
    guards: &[AggregateGuard],
    limits: AggregateFamilyLimits,
    control: &Control,
) -> Result<AggregateFamilyBuild, AggregateError> {
    transaction_value(nodes, limits.aggregate, control, |builder| {
        compile(builder, elements, guards, limits.max_guards)
    })
    .map(|((roots, profile), statistics)| AggregateFamilyBuild {
        roots,
        profile,
        statistics,
    })
}

type ResultKind<T> = Result<T, AggregateErrorKind>;

fn compile(
    builder: &mut Builder<'_>,
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
    let mut roots = reserve(guards.len())?;
    let falsum = builder.push(Node::False)?;
    let truth = builder.push(Node::Implies(falsum, falsum))?;
    if nonnegative {
        let maximum = required_threshold(builder, guards, total)?;
        let row = threshold_row(builder, elements, maximum, falsum, truth)?;
        for guard in guards {
            builder.tick()?;
            roots.push(compose(builder, &row, *guard, total, falsum, truth)?);
        }
    } else {
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
    }
    Ok((roots, profile))
}

fn required_threshold(
    builder: &mut Builder<'_>,
    guards: &[AggregateGuard],
    total: i128,
) -> ResultKind<i128> {
    let mut maximum = 0;
    for guard in guards {
        builder.tick()?;
        let bound = i128::from(guard.bound);
        let lower = if matches!(
            guard.comparison,
            AggregateComparison::Gt | AggregateComparison::Le
        ) {
            bound + 1
        } else {
            bound
        };
        if lower > 0 && lower <= total {
            maximum = maximum.max(lower);
        }
        if matches!(
            guard.comparison,
            AggregateComparison::Eq | AggregateComparison::Ne
        ) && bound >= 0
            && bound < total
        {
            maximum = maximum.max(bound + 1);
        }
    }
    Ok(maximum)
}

fn threshold_row(
    builder: &mut Builder<'_>,
    elements: &[AggregateElement],
    maximum: i128,
    falsum: usize,
    truth: usize,
) -> ResultKind<Vec<usize>> {
    if maximum == 0 {
        return Ok(Vec::new());
    }
    let maximum = usize::try_from(maximum).map_err(|_| AggregateErrorKind::StateLimit)?;
    let width = maximum
        .checked_add(1)
        .ok_or(AggregateErrorKind::ArithmeticOverflow)?;
    builder.states(
        width
            .checked_mul(2)
            .ok_or(AggregateErrorKind::ArithmeticOverflow)?,
    )?;
    let mut previous = reserve(width)?;
    let mut current = reserve(width)?;
    for target in 0..width {
        builder.tick()?;
        previous.push(if target == 0 { truth } else { falsum });
        current.push(falsum);
    }
    for element in elements {
        builder.tick()?;
        if element.weight == 0 {
            continue;
        }
        let weight =
            usize::try_from(element.weight).map_err(|_| AggregateErrorKind::ArithmeticOverflow)?;
        current[0] = truth;
        for target in 1..width {
            builder.tick()?;
            let included = builder.push(Node::And(
                element.condition,
                previous[target.saturating_sub(weight)],
            ))?;
            current[target] = builder.push(Node::Or(previous[target], included))?;
        }
        std::mem::swap(&mut previous, &mut current);
    }
    Ok(previous)
}

fn lookup(
    row: &[usize],
    bound: i128,
    total: i128,
    falsum: usize,
    truth: usize,
) -> ResultKind<usize> {
    if bound <= 0 {
        return Ok(truth);
    }
    if bound > total {
        return Ok(falsum);
    }
    let index = usize::try_from(bound).map_err(|_| AggregateErrorKind::StateLimit)?;
    row.get(index)
        .copied()
        .ok_or(AggregateErrorKind::ArithmeticOverflow)
}

fn compose(
    builder: &mut Builder<'_>,
    row: &[usize],
    guard: AggregateGuard,
    total: i128,
    falsum: usize,
    truth: usize,
) -> ResultKind<usize> {
    let bound = i128::from(guard.bound);
    let lower = if matches!(
        guard.comparison,
        AggregateComparison::Gt | AggregateComparison::Le
    ) {
        bound + 1
    } else {
        bound
    };
    let a = lookup(row, lower, total, falsum, truth)?;
    match guard.comparison {
        AggregateComparison::Ge | AggregateComparison::Gt => Ok(a),
        AggregateComparison::Lt | AggregateComparison::Le => builder.push(Node::Implies(a, falsum)),
        AggregateComparison::Eq | AggregateComparison::Ne => {
            let b = lookup(row, bound + 1, total, falsum, truth)?;
            if guard.comparison == AggregateComparison::Eq {
                let negative = builder.push(Node::Implies(b, falsum))?;
                builder.push(Node::And(a, negative))
            } else {
                builder.push(Node::Implies(a, b))
            }
        }
    }
}
