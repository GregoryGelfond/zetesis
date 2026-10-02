use zetesis_cpu::Cancellation;

use super::{
    AggregateBuild, AggregateComparison, AggregateElement, AggregateError, AggregateErrorKind,
    AggregateLimits, AggregateProfile, AggregateStatistics,
};
use crate::Node;

type Result<T> = std::result::Result<T, AggregateErrorKind>;

/// The exclusive node borrow and, for an owned buffer, its retained scan frontier.
pub(super) struct Destination<'a> {
    nodes: &'a mut Vec<Node>,
    validated: Option<&'a mut usize>,
}
impl<'a> Destination<'a> {
    pub(super) fn unchecked(nodes: &'a mut Vec<Node>) -> Self {
        Self {
            nodes,
            validated: None,
        }
    }

    pub(super) fn retained(nodes: &'a mut Vec<Node>, validated: &'a mut usize) -> Self {
        Self {
            nodes,
            validated: Some(validated),
        }
    }
}

pub(super) struct Builder<'a> {
    nodes: &'a mut Vec<Node>,
    limits: AggregateLimits,
    cancellation: &'a Cancellation,
    statistics: AggregateStatistics,
    validated: usize,
}
impl Builder<'_> {
    pub(super) fn tick(&mut self) -> Result<()> {
        self.cancellation
            .poll()
            .map_err(AggregateErrorKind::Control)?;
        if self.statistics.work >= self.limits.max_work {
            return Err(AggregateErrorKind::WorkLimit);
        }
        self.statistics.work += 1;
        Ok(())
    }

    pub(super) fn push(&mut self, node: Node) -> Result<usize> {
        self.tick()?;
        if self.nodes.len() >= self.limits.max_nodes {
            return Err(AggregateErrorKind::NodeLimit);
        }
        self.nodes
            .try_reserve(1)
            .map_err(|_| AggregateErrorKind::Allocation)?;
        let index = self.nodes.len();
        self.nodes.push(node);
        self.statistics.nodes += 1;
        Ok(index)
    }

    pub(super) fn states(&mut self, count: usize) -> Result<()> {
        if count > self.limits.max_states {
            return Err(AggregateErrorKind::StateLimit);
        }
        self.statistics.states = self.statistics.states.max(count);
        Ok(())
    }

    pub(super) fn join(
        &mut self,
        previous: Option<usize>,
        next: usize,
        and: bool,
    ) -> Result<usize> {
        match previous {
            None => Ok(next),
            Some(previous) => self.push(if and {
                Node::And(previous, next)
            } else {
                Node::Or(previous, next)
            }),
        }
    }
}

pub(super) fn reserve<T>(count: usize) -> Result<Vec<T>> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| AggregateErrorKind::Allocation)?;
    Ok(result)
}

/// Append a finite, scalar `#count`/`#sum` aggregate formula transactionally.
/// Conditions may be arbitrary existing formulas. Each input entry must identify
/// a different complete tuple; callers coalesce duplicate tuple eligibility with
/// OR before calling. `#count` uses weight one. The caller supplies one evaluated
/// scalar guard and applies default negation as implication to falsum afterward.
///
/// Nonnegative weights use shared monotone threshold formulas. Mixed/negative
/// weights use every failing subset's implication without complementing weights.
/// Neither path introduces semantic atoms or relies on classical-only rewriting.
/// Existing edge topology and condition IDs are checked; atom-universe admission
/// remains the responsibility of [`crate::Theory::new`].
///
/// On failure, original nodes and length are restored; reserved capacity may
/// change. The caller owns source origins and should restrict `max_nodes` to its
/// remaining origin capacity before appending, then record every new node.
///
/// # Errors
/// Returns a typed input, resource, allocation, cancellation or arithmetic error.
pub fn append_aggregate(
    nodes: &mut Vec<Node>,
    elements: &[AggregateElement],
    comparison: AggregateComparison,
    bound: i64,
    limits: AggregateLimits,
    cancellation: &Cancellation,
) -> std::result::Result<AggregateBuild, AggregateError> {
    append(
        Destination::unchecked(nodes),
        elements,
        comparison,
        bound,
        limits,
        cancellation,
    )
}

pub(super) fn append(
    destination: Destination<'_>,
    elements: &[AggregateElement],
    comparison: AggregateComparison,
    bound: i64,
    limits: AggregateLimits,
    cancellation: &Cancellation,
) -> std::result::Result<AggregateBuild, AggregateError> {
    transaction(destination, limits, cancellation, |builder| {
        compile(builder, elements, comparison, i128::from(bound))
    })
}

pub(super) fn transaction(
    destination: Destination<'_>,
    limits: AggregateLimits,
    cancellation: &Cancellation,
    compile: impl FnOnce(&mut Builder<'_>) -> Result<(usize, AggregateProfile)>,
) -> std::result::Result<AggregateBuild, AggregateError> {
    transaction_value(destination, limits, cancellation, compile).map(
        |((root, profile), statistics)| AggregateBuild {
            root,
            profile,
            statistics,
        },
    )
}

pub(super) fn transaction_value<T>(
    destination: Destination<'_>,
    limits: AggregateLimits,
    cancellation: &Cancellation,
    compile: impl FnOnce(&mut Builder<'_>) -> Result<T>,
) -> std::result::Result<(T, AggregateStatistics), AggregateError> {
    let original = destination.nodes.len();
    let mut builder = Builder {
        nodes: destination.nodes,
        validated: destination.validated.as_deref().copied().unwrap_or(0),
        limits,
        cancellation,
        statistics: AggregateStatistics::default(),
    };
    let result = match compile(&mut builder) {
        Ok(result) => Ok((result, builder.statistics)),
        Err(kind) => {
            builder.nodes.truncate(original);
            Err(AggregateError {
                kind,
                statistics: builder.statistics,
            })
        }
    };
    if let Some(validated) = destination.validated {
        *validated = builder.validated.min(builder.nodes.len());
    }
    result
}

pub(super) fn validate(
    builder: &mut Builder<'_>,
    elements: &[AggregateElement],
) -> Result<(bool, i128)> {
    validate_elements(
        builder,
        elements
            .iter()
            .map(|element| (element.condition, element.weight)),
    )
}

pub(super) fn validate_elements(
    builder: &mut Builder<'_>,
    elements: impl ExactSizeIterator<Item = (usize, i32)>,
) -> Result<(bool, i128)> {
    validate_prefix(builder)?;
    if elements.len() > builder.limits.max_elements {
        return Err(AggregateErrorKind::ElementLimit);
    }
    let mut nonnegative = true;
    let mut total = 0i128;
    for (index, (condition, weight)) in elements.enumerate() {
        builder.tick()?;
        if condition >= builder.nodes.len() {
            return Err(AggregateErrorKind::InvalidCondition { element: index });
        }
        nonnegative &= weight >= 0;
        total = total
            .checked_add(i128::from(weight))
            .ok_or(AggregateErrorKind::ArithmeticOverflow)?;
    }
    Ok((nonnegative, total))
}

/// Validate the existing DAG before any aggregate-specific appends.
pub(super) fn validate_prefix(builder: &mut Builder<'_>) -> Result<usize> {
    builder.tick()?;
    if builder.nodes.len() > builder.limits.max_nodes {
        return Err(AggregateErrorKind::NodeLimit);
    }
    let prefix = builder.nodes.len();
    for index in builder.validated..prefix {
        builder.tick()?;
        if let Node::And(a, b) | Node::Or(a, b) | Node::Implies(a, b) = builder.nodes[index]
            && (a >= index || b >= index)
        {
            return Err(AggregateErrorKind::InvalidPrefix { node: index });
        }
    }
    // Publish only a complete scan. Failed scans retain their earlier frontier.
    builder.validated = prefix;
    Ok(prefix)
}

fn compile(
    builder: &mut Builder<'_>,
    elements: &[AggregateElement],
    comparison: AggregateComparison,
    bound: i128,
) -> Result<(usize, AggregateProfile)> {
    let (nonnegative, total) = validate(builder, elements)?;
    let falsum = builder.push(Node::False)?;
    let truth = builder.push(Node::Implies(falsum, falsum))?;
    if nonnegative {
        let root = thresholds(builder, elements, comparison, bound, total, falsum, truth)?;
        Ok((root, AggregateProfile::Threshold))
    } else {
        let root = subsets(builder, elements, comparison, bound, falsum, truth)?;
        Ok((root, AggregateProfile::SubsetImplications))
    }
}

fn thresholds(
    builder: &mut Builder<'_>,
    elements: &[AggregateElement],
    comparison: AggregateComparison,
    bound: i128,
    total: i128,
    falsum: usize,
    truth: usize,
) -> Result<usize> {
    // Bound is widened from i64, so even a strict comparison at i64::MAX has
    // a representable successor. All threshold arithmetic stays in i128.
    let lower = match comparison {
        AggregateComparison::Gt | AggregateComparison::Le => bound + 1,
        _ => bound,
    };
    let a = threshold(builder, elements, lower, total, falsum, truth)?;
    match comparison {
        AggregateComparison::Ge | AggregateComparison::Gt => Ok(a),
        AggregateComparison::Lt | AggregateComparison::Le => builder.push(Node::Implies(a, falsum)),
        AggregateComparison::Eq | AggregateComparison::Ne => {
            let b = threshold(builder, elements, bound + 1, total, falsum, truth)?;
            if comparison == AggregateComparison::Eq {
                let negative = builder.push(Node::Implies(b, falsum))?;
                builder.push(Node::And(a, negative))
            } else {
                // Not-equal is NOT default negation of equality: retain the
                // implication to preserve reduct support at a nonconvex guard.
                builder.push(Node::Implies(a, b))
            }
        }
    }
}

fn threshold(
    builder: &mut Builder<'_>,
    elements: &[AggregateElement],
    bound: i128,
    total: i128,
    falsum: usize,
    truth: usize,
) -> Result<usize> {
    builder.tick()?;
    if bound <= 0 {
        return Ok(truth);
    }
    if bound > total {
        return Ok(falsum);
    }
    let bound = usize::try_from(bound).map_err(|_| AggregateErrorKind::StateLimit)?;
    let width = bound
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
    Ok(previous[bound])
}

pub(super) fn subsets(
    builder: &mut Builder<'_>,
    elements: &[AggregateElement],
    comparison: AggregateComparison,
    bound: i128,
    falsum: usize,
    truth: usize,
) -> Result<usize> {
    let exponent = u32::try_from(elements.len()).map_err(|_| AggregateErrorKind::SubsetLimit)?;
    let count = 1u64
        .checked_shl(exponent)
        .filter(|count| {
            count
                .checked_add(builder.statistics.subsets)
                .is_some_and(|cumulative| cumulative <= builder.limits.max_subsets)
        })
        .ok_or(AggregateErrorKind::SubsetLimit)?;
    builder.states(elements.len())?;
    let mut selected = reserve(elements.len())?;
    for _ in elements {
        builder.tick()?;
        selected.push(false);
    }
    let mut sum = 0i128;
    let mut root = None;
    for ordinal in 0..count {
        builder.tick()?;
        builder.statistics.subsets += 1;
        if !comparison.holds(sum, bound) {
            let mut antecedent = None;
            let mut consequent = None;
            for (element, &inside) in elements.iter().zip(&selected) {
                builder.tick()?;
                if inside {
                    antecedent = Some(builder.join(antecedent, element.condition, true)?);
                } else {
                    consequent = Some(builder.join(consequent, element.condition, false)?);
                }
            }
            let implication = builder.push(Node::Implies(
                antecedent.unwrap_or(truth),
                consequent.unwrap_or(falsum),
            ))?;
            root = Some(builder.join(root, implication, true)?);
        }
        if ordinal + 1 < count {
            for (inside, element) in selected.iter_mut().zip(elements) {
                builder.tick()?;
                *inside = !*inside;
                sum = if *inside {
                    sum.checked_add(i128::from(element.weight))
                } else {
                    sum.checked_sub(i128::from(element.weight))
                }
                .ok_or(AggregateErrorKind::ArithmeticOverflow)?;
                if *inside {
                    break;
                }
            }
        }
    }
    Ok(root.unwrap_or(truth))
}
