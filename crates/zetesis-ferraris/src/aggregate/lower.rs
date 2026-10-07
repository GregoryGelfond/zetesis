use zetesis_cpu::Cancellation;

use super::{
    AggregateBuild, AggregateComparison, AggregateElement, AggregateError, AggregateErrorKind,
    AggregateLimits, AggregateProfile, AggregateStatistics,
};
use crate::{AdmissionError, FormulaNodes, FormulaTransaction, NodeView};

type Result<T> = std::result::Result<T, AggregateErrorKind>;

/// An exclusive paired append transaction, nested beneath its caller's owner.
pub(super) struct Destination<'a> {
    nodes: FormulaTransaction<'a>,
}
impl<'a> Destination<'a> {
    pub(super) fn retained(nodes: FormulaTransaction<'a>) -> Self {
        Self { nodes }
    }
}

pub(super) struct Builder<'a, 'control> {
    nodes: FormulaTransaction<'a>,
    limits: AggregateLimits,
    cancellation: &'control Cancellation,
    statistics: AggregateStatistics,
}
impl Builder<'_, '_> {
    pub(super) fn tick(&mut self) -> Result<()> {
        tick(&mut self.statistics, self.limits, self.cancellation)
    }

    pub(super) fn push(&mut self, node: NodeView<'_>) -> Result<usize> {
        self.tick()?;
        let (added, edges, copies) = match node {
            NodeView::And([]) => (2, 2, 0),
            NodeView::And([_]) | NodeView::Or([_]) => (0, 0, 1),
            NodeView::And(row) | NodeView::Or(row) => {
                (1, row.len(), if row.len() >= 3 { row.len() } else { 0 })
            }
            NodeView::Implies(_, _) => (1, 2, 0),
            NodeView::Atom(_) | NodeView::False => (1, 0, 0),
        };
        if self.nodes.view().len().saturating_add(added) > self.limits.max_nodes {
            return Err(AggregateErrorKind::NodeLimit);
        }
        if self.nodes.parts().occurrences().saturating_add(edges) > self.limits.max_operands {
            return Err(AggregateErrorKind::OperandLimit);
        }
        // Charge child validation and the copied wide row before publication.
        for _ in 0..edges.saturating_add(copies) {
            self.tick()?;
        }
        let index = self
            .nodes
            .push(node, self.limits.max_nodes, self.limits.max_operands)
            .map_err(|error| match error {
                AdmissionError::Allocation => AggregateErrorKind::Allocation,
                AdmissionError::Limit => AggregateErrorKind::OperandLimit,
                _ => AggregateErrorKind::InvalidPrefix {
                    node: self.nodes.view().len(),
                },
            })?;
        self.statistics.nodes += added;
        self.statistics.operands += edges;
        Ok(index)
    }

    pub(super) fn states(&mut self, count: usize) -> Result<()> {
        if count > self.limits.max_states {
            return Err(AggregateErrorKind::StateLimit);
        }
        self.statistics.states = self.statistics.states.max(count);
        Ok(())
    }

    /// Retain one complete connective row under the same occurrence ceiling.
    /// Growth and publication consume work before the corresponding allocation
    /// or write. These rows are temporary operands, not threshold DP states.
    pub(super) fn operand(&mut self, row: &mut Vec<usize>, child: usize) -> Result<()> {
        if row.len() >= self.limits.max_operands {
            return Err(AggregateErrorKind::OperandLimit);
        }
        if row.len() == row.capacity() {
            for _ in 0..row.len() {
                self.tick()?;
            }
            row.try_reserve(1)
                .map_err(|_| AggregateErrorKind::Allocation)?;
        }
        self.tick()?;
        row.push(child);
        Ok(())
    }

    pub(super) fn group(&mut self, row: &[usize], identity: usize, and: bool) -> Result<usize> {
        match row {
            [] => Ok(identity),
            [only] => Ok(*only),
            _ => self.push(if and {
                NodeView::And(row)
            } else {
                NodeView::Or(row)
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
/// On failure, both node and operand suffixes are removed; their original
/// prefixes remain unchanged, while reserved capacities may change. The caller owns source origins and should restrict `max_nodes` to its
/// remaining origin capacity before appending, then record every new node.
///
/// # Errors
/// Returns a typed input, resource, allocation, cancellation or arithmetic error.
pub fn append_aggregate(
    nodes: &mut FormulaNodes,
    elements: &[AggregateElement],
    comparison: AggregateComparison,
    bound: i64,
    limits: AggregateLimits,
    cancellation: &Cancellation,
) -> std::result::Result<AggregateBuild, AggregateError> {
    append(
        Destination::retained(nodes.transaction()),
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
    compile: impl FnOnce(&mut Builder<'_, '_>) -> Result<(usize, AggregateProfile)>,
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
    compile: impl FnOnce(&mut Builder<'_, '_>) -> Result<T>,
) -> std::result::Result<(T, AggregateStatistics), AggregateError> {
    let mut builder = Builder {
        nodes: destination.nodes,
        limits,
        cancellation,
        statistics: AggregateStatistics::default(),
    };
    let result = compile(&mut builder);
    match result {
        Ok(result) => {
            let statistics = builder.statistics;
            builder.nodes.commit();
            Ok((result, statistics))
        }
        Err(kind) => Err(AggregateError {
            kind,
            statistics: builder.statistics,
        }),
    }
}

fn tick(
    statistics: &mut AggregateStatistics,
    limits: AggregateLimits,
    cancellation: &Cancellation,
) -> Result<()> {
    cancellation.poll().map_err(AggregateErrorKind::Control)?;
    if statistics.work >= limits.max_work {
        return Err(AggregateErrorKind::WorkLimit);
    }
    statistics.work += 1;
    Ok(())
}

pub(super) fn validate(
    builder: &mut Builder<'_, '_>,
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
    builder: &mut Builder<'_, '_>,
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
        if condition >= builder.nodes.view().len() {
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
pub(super) fn validate_prefix(builder: &mut Builder<'_, '_>) -> Result<usize> {
    builder.tick()?;
    let parts = builder.nodes.parts();
    if parts.nodes().len() > builder.limits.max_nodes {
        return Err(AggregateErrorKind::NodeLimit);
    }
    if parts.occurrences() > builder.limits.max_operands
        || parts.operands().len() > builder.limits.max_operands
    {
        return Err(AggregateErrorKind::OperandLimit);
    }
    let prefix = parts.nodes().len();
    for index in builder.nodes.validation_frontier()..prefix {
        builder.tick()?;
        let invalid = || AggregateErrorKind::InvalidPrefix { node: index };
        let node = builder.nodes.view().node(index).map_err(|_| invalid())?;
        let pair;
        let children = match node {
            NodeView::And(row) | NodeView::Or(row) => row,
            NodeView::Implies(left, right) => {
                pair = [left, right];
                &pair
            }
            NodeView::Atom(_) | NodeView::False => &[],
        };
        for &child in children {
            tick(
                &mut builder.statistics,
                builder.limits,
                builder.cancellation,
            )?;
            if child >= index {
                return Err(invalid());
            }
        }
    }
    // Only a completed whole-prefix scan can be retained by a committed append.
    builder.nodes.set_validation_frontier(prefix);
    Ok(prefix)
}

fn compile(
    builder: &mut Builder<'_, '_>,
    elements: &[AggregateElement],
    comparison: AggregateComparison,
    bound: i128,
) -> Result<(usize, AggregateProfile)> {
    let (nonnegative, total) = validate(builder, elements)?;
    let falsum = builder.push(NodeView::False)?;
    let truth = builder.push(NodeView::Implies(falsum, falsum))?;
    if nonnegative {
        let root = thresholds(builder, elements, comparison, bound, total, falsum, truth)?;
        Ok((root, AggregateProfile::Threshold))
    } else {
        let root = subsets(builder, elements, comparison, bound, falsum, truth)?;
        Ok((root, AggregateProfile::SubsetImplications))
    }
}

fn thresholds(
    builder: &mut Builder<'_, '_>,
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
        AggregateComparison::Lt | AggregateComparison::Le => {
            builder.push(NodeView::Implies(a, falsum))
        }
        AggregateComparison::Eq | AggregateComparison::Ne => {
            let b = threshold(builder, elements, bound + 1, total, falsum, truth)?;
            if comparison == AggregateComparison::Eq {
                let negative = builder.push(NodeView::Implies(b, falsum))?;
                builder.push(NodeView::And(&[a, negative]))
            } else {
                // Not-equal is NOT default negation of equality: retain the
                // implication to preserve reduct support at a nonconvex guard.
                builder.push(NodeView::Implies(a, b))
            }
        }
    }
}

fn threshold(
    builder: &mut Builder<'_, '_>,
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
            let included = builder.push(NodeView::And(&[
                element.condition,
                previous[target.saturating_sub(weight)],
            ]))?;
            current[target] = builder.push(NodeView::Or(&[previous[target], included]))?;
        }
        std::mem::swap(&mut previous, &mut current);
    }
    Ok(previous[bound])
}

pub(super) fn subsets(
    builder: &mut Builder<'_, '_>,
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
    let mut clauses = Vec::new();
    let mut antecedent = Vec::new();
    let mut consequent = Vec::new();
    for ordinal in 0..count {
        builder.tick()?;
        builder.statistics.subsets += 1;
        if !comparison.holds(sum, bound) {
            antecedent.clear();
            consequent.clear();
            for (element, &inside) in elements.iter().zip(&selected) {
                builder.tick()?;
                if inside {
                    builder.operand(&mut antecedent, element.condition)?;
                } else {
                    builder.operand(&mut consequent, element.condition)?;
                }
            }
            let left = builder.group(&antecedent, truth, true)?;
            let right = builder.group(&consequent, falsum, false)?;
            let implication = builder.push(NodeView::Implies(left, right))?;
            builder.operand(&mut clauses, implication)?;
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
    builder.group(&clauses, truth, true)
}
