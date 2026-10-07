use zetesis_cpu::Cancellation;

use super::lower::{Builder, Destination, transaction, validate};
use super::{
    AggregateBuild, AggregateComparison, AggregateElement, AggregateError, AggregateErrorKind,
    AggregateLimits, AggregateProfile,
};
use crate::{FormulaNodes, NodeView};

/// A finite extremum with the source language's empty-set convention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateExtremum {
    /// Minimum eligible value; the empty result is positive infinity (`#sup`).
    Min,
    /// Maximum eligible value; the empty result is negative infinity (`#inf`).
    Max,
}

/// A numeric extremum guard, including the language's two ordered sentinels.
/// Sentinels are distinct from every finite integer, including integer limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExtremumBound {
    /// The least source term, `#inf`.
    NegativeInfinity,
    /// A finite signed numeric guard.
    Number(i64),
    /// The greatest source term, `#sup`.
    PositiveInfinity,
}
impl From<i64> for ExtremumBound {
    fn from(value: i64) -> Self {
        Self::Number(value)
    }
}
impl From<i32> for ExtremumBound {
    fn from(value: i32) -> Self {
        Self::Number(i64::from(value))
    }
}

/// Append an exact finite numeric `#min` or `#max` comparison transactionally.
/// Elements use their numeric `weight` as the first tuple component and retain
/// arbitrary existing eligibility formulas. Equal complete tuples must already
/// be OR-coalesced by the caller, as for [`super::append_aggregate`].
///
/// The empty maximum is `#inf`; the empty minimum is `#sup`. Numeric guards can
/// be passed directly, or an explicit [`ExtremumBound`] can test the sentinels.
/// Non-numeric tuple first components and explicit infinite element values are
/// outside this numeric interface. Source assignment, origin tracking and guard
/// expansion remain caller obligations. Default negation is applied afterward
/// as implication to falsum, never by inverting the aggregate comparison.
///
/// Compilation uses two filtered disjunctions of eligibility formulas, linear
/// in the input and existing prefix. It allocates no DP rows or subset carrier,
/// so `max_states` and `max_subsets` may both be zero. It introduces no semantic
/// atoms and retains an implication for not-equal. Prefix validation, inclusive
/// node/work/element ceilings, cancellation, allocation and rollback follow
/// [`super::append_aggregate`]. The original DAG length and nodes survive errors.
///
/// # Errors
/// Returns a typed input, resource, allocation, cancellation or arithmetic error.
pub fn append_extremum(
    nodes: &mut FormulaNodes,
    elements: &[AggregateElement],
    extremum: AggregateExtremum,
    comparison: AggregateComparison,
    bound: impl Into<ExtremumBound>,
    limits: AggregateLimits,
    cancellation: &Cancellation,
) -> Result<AggregateBuild, AggregateError> {
    append(
        Destination::retained(nodes.transaction()),
        elements,
        extremum,
        comparison,
        bound.into(),
        limits,
        cancellation,
    )
}

pub(super) fn append(
    destination: Destination<'_>,
    elements: &[AggregateElement],
    extremum: AggregateExtremum,
    comparison: AggregateComparison,
    bound: ExtremumBound,
    limits: AggregateLimits,
    cancellation: &Cancellation,
) -> Result<AggregateBuild, AggregateError> {
    transaction(destination, limits, cancellation, |builder| {
        compile(builder, elements, extremum, comparison, bound)
            .map(|root| (root, AggregateProfile::Extremum))
    })
}

fn compile(
    builder: &mut Builder<'_, '_>,
    elements: &[AggregateElement],
    extremum: AggregateExtremum,
    comparison: AggregateComparison,
    bound: ExtremumBound,
) -> Result<usize, AggregateErrorKind> {
    validate(builder, elements)?;
    let falsum = builder.push(NodeView::False)?;
    let truth = builder.push(NodeView::Implies(falsum, falsum))?;
    // G is max >= bound / min <= bound. H is the corresponding strict
    // comparison. Both are monotone in eligible tuples, including sentinels.
    let (inclusive, strict) = witnesses(builder, elements, extremum, bound, falsum, truth)?;
    comparison_root(builder, extremum, comparison, inclusive, strict, falsum)
}

pub(super) fn comparison_root(
    builder: &mut Builder<'_, '_>,
    extremum: AggregateExtremum,
    comparison: AggregateComparison,
    inclusive: usize,
    strict: usize,
    falsum: usize,
) -> Result<usize, AggregateErrorKind> {
    let (positive, negative) = match extremum {
        AggregateExtremum::Max => (AggregateComparison::Ge, AggregateComparison::Lt),
        AggregateExtremum::Min => (AggregateComparison::Le, AggregateComparison::Gt),
    };
    if comparison == positive {
        return Ok(inclusive);
    }
    if comparison == negative {
        return builder.push(NodeView::Implies(inclusive, falsum));
    }
    match comparison {
        AggregateComparison::Eq => {
            let below_strict = builder.push(NodeView::Implies(strict, falsum))?;
            builder.push(NodeView::And(&[inclusive, below_strict]))
        }
        AggregateComparison::Ne => builder.push(NodeView::Implies(inclusive, strict)),
        AggregateComparison::Gt | AggregateComparison::Lt => Ok(strict),
        AggregateComparison::Ge | AggregateComparison::Le => {
            builder.push(NodeView::Implies(strict, falsum))
        }
    }
}

fn witnesses(
    builder: &mut Builder<'_, '_>,
    elements: &[AggregateElement],
    extremum: AggregateExtremum,
    bound: ExtremumBound,
    falsum: usize,
    truth: usize,
) -> Result<(usize, usize), AggregateErrorKind> {
    let empty = match extremum {
        AggregateExtremum::Max => ExtremumBound::NegativeInfinity,
        AggregateExtremum::Min => ExtremumBound::PositiveInfinity,
    };
    let mut inclusive = Vec::new();
    if bound == empty {
        builder.operand(&mut inclusive, truth)?;
    }
    let mut strict = Vec::new();
    for element in elements {
        builder.tick()?;
        let value = ExtremumBound::Number(i64::from(element.weight));
        let order = match extremum {
            AggregateExtremum::Max => value.cmp(&bound),
            AggregateExtremum::Min => bound.cmp(&value),
        };
        if order.is_ge() {
            builder.operand(&mut inclusive, element.condition)?;
        }
        if order.is_gt() {
            builder.operand(&mut strict, element.condition)?;
        }
    }
    Ok((
        builder.group(&inclusive, falsum, false)?,
        builder.group(&strict, falsum, false)?,
    ))
}
