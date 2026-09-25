use zetesis_core::{Value, catalog::TermRef};
use zetesis_cpu::Cancellation;

use super::extremum::comparison_root;
use super::lower::{transaction, validate_prefix};
use super::{
    AggregateBuild, AggregateComparison, AggregateError, AggregateErrorKind, AggregateExtremum,
    AggregateLimits, AggregateProfile,
};
use crate::Node;

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
    nodes: &mut Vec<Node>,
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
    nodes: &mut Vec<Node>,
    elements: impl Iterator<Item = ValueExtremumElement<TermRef<'a>>>,
    extremum: AggregateExtremum,
    comparison: AggregateComparison,
    bound: TermRef<'_>,
    limits: AggregateLimits,
    cancellation: &Cancellation,
) -> Result<AggregateBuild, AggregateError> {
    transaction(nodes, limits, cancellation, |builder| {
        let prefix = validate_prefix(builder)?;
        let falsum = builder.push(Node::False)?;
        let truth = builder.push(Node::Implies(falsum, falsum))?;
        let empty = match extremum {
            AggregateExtremum::Min => Value::Supremum,
            AggregateExtremum::Max => Value::Infimum,
        };
        let mut inclusive = (bound == TermRef::from(&empty)).then_some(truth);
        let mut strict = None;
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
                inclusive = Some(builder.join(inclusive, element.condition, false)?);
            }
            if order.is_gt() {
                strict = Some(builder.join(strict, element.condition, false)?);
            }
        }
        comparison_root(
            builder,
            extremum,
            comparison,
            inclusive.unwrap_or(falsum),
            strict.unwrap_or(falsum),
            falsum,
        )
        .map(|root| (root, AggregateProfile::Extremum))
    })
}
