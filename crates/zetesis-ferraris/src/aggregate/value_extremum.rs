use zetesis_core::{Value, catalog::TermRef};
use zetesis_cpu::Cancellation;

use super::extremum::comparison_root;
use super::lower::{Destination, transaction, validate_prefix};
use super::{
    AggregateBuild, AggregateComparison, AggregateError, AggregateErrorKind, AggregateExtremum,
    AggregateLimits, AggregateProfile,
};
use crate::{FormulaNodes, NodeView};

/// One complete tuple's first value and OR-coalesced eligibility formula.
/// Equal first values do not identify equal complete tuples.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueExtremumElement<V = Value> {
    /// Complete first tuple value, compared using ASP term order.
    pub value: V,
    /// Absolute condition index in the existing DAG prefix.
    pub condition: usize,
}

/// Append an exact min/max comparison over complete ASP values.
///
/// The caller groups equal complete tuples by disjoining eligibility. Empty
/// minimum is `#sup`; empty maximum is `#inf`. Explicit sentinel elements are
/// allowed. Default negation is applied to the resulting formula by the caller.
/// This uses the numeric path's reduct-preserving witness connectives, including
/// implication for not-equal, and introduces no semantic atoms. All appends are
/// transactional; work, element and node limits and cancellation remain explicit.
/// Structural comparisons charge their visited term components.
/// No threshold states or subsets are allocated.
///
/// # Errors
/// Returns typed prefix, condition, resource, allocation or cancellation errors.
pub fn append_value_extremum(
    nodes: &mut FormulaNodes,
    elements: &[ValueExtremumElement],
    extremum: AggregateExtremum,
    comparison: AggregateComparison,
    bound: &Value,
    limits: AggregateLimits,
    cancellation: &Cancellation,
) -> Result<AggregateBuild, AggregateError> {
    append_value_extremum_refs(
        nodes,
        elements.iter().map(|element| ValueExtremumElement {
            value: (&element.value).into(),
            condition: element.condition,
        }),
        extremum,
        comparison,
        bound.into(),
        limits,
        cancellation,
    )
}

/// Append the same reduct-preserving extremum comparison over borrowed values.
///
/// One traversal checks every actual condition against the original DAG prefix
/// before using it. Borrowed canonical terms share their original authority;
/// no typed payload is materialized. Prefix validation, element visits and typed
/// comparison steps consume the aggregate work allowance.
///
/// # Errors
/// Returns the same prefix, condition, resource, allocation and cancellation
/// failures as [`append_value_extremum`], preserving the original DAG on refusal.
pub fn append_value_extremum_refs<'a>(
    nodes: &mut FormulaNodes,
    elements: impl Iterator<Item = ValueExtremumElement<TermRef<'a>>>,
    extremum: AggregateExtremum,
    comparison: AggregateComparison,
    bound: TermRef<'_>,
    limits: AggregateLimits,
    cancellation: &Cancellation,
) -> Result<AggregateBuild, AggregateError> {
    append(
        Destination::retained(nodes.transaction()),
        elements,
        extremum,
        comparison,
        bound,
        limits,
        cancellation,
    )
}

pub(super) fn append<'a>(
    destination: Destination<'_>,
    elements: impl Iterator<Item = ValueExtremumElement<TermRef<'a>>>,
    extremum: AggregateExtremum,
    comparison: AggregateComparison,
    bound: TermRef<'_>,
    limits: AggregateLimits,
    cancellation: &Cancellation,
) -> Result<AggregateBuild, AggregateError> {
    transaction(destination, limits, cancellation, |builder| {
        let prefix = validate_prefix(builder)?;
        let falsum = builder.push(NodeView::False)?;
        let truth = builder.push(NodeView::Implies(falsum, falsum))?;
        let empty = match extremum {
            AggregateExtremum::Min => Value::Supremum,
            AggregateExtremum::Max => Value::Infimum,
        };
        let mut inclusive = Vec::new();
        if bound == TermRef::from(&empty) {
            builder.operand(&mut inclusive, truth)?;
        }
        let mut strict = Vec::new();
        for (index, element) in elements.enumerate() {
            builder.tick()?;
            if index >= limits.max_elements {
                return Err(AggregateErrorKind::ElementLimit);
            }
            if element.condition >= prefix {
                return Err(AggregateErrorKind::InvalidCondition { element: index });
            }
            let order = element.value.compare_terms_with(bound, || builder.tick())?;
            let order = if extremum == AggregateExtremum::Min {
                order.reverse()
            } else {
                order
            };
            if order.is_ge() {
                builder.operand(&mut inclusive, element.condition)?;
            }
            if order.is_gt() {
                builder.operand(&mut strict, element.condition)?;
            }
        }
        let inclusive = builder.group(&inclusive, falsum, false)?;
        let strict = builder.group(&strict, falsum, false)?;
        comparison_root(builder, extremum, comparison, inclusive, strict, falsum)
            .map(|root| (root, AggregateProfile::Extremum))
    })
}
