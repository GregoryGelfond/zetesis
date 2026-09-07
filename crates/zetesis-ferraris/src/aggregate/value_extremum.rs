use zetesis_core::Value;
use zetesis_cpu::Control;

use super::extremum::comparison_root;
use super::lower::{transaction, validate_elements};
use super::{
    AggregateBuild, AggregateComparison, AggregateError, AggregateExtremum, AggregateLimits,
    AggregateProfile,
};
use crate::Node;

/// One complete tuple's first value and OR-coalesced eligibility formula.
/// Equal first values do not identify equal complete tuples.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueExtremumElement {
    /// Complete first tuple value, compared using ASP term order.
    pub value: Value,
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
/// Structural comparisons charge both traversed value carriers before comparison.
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
    control: &Control,
) -> Result<AggregateBuild, AggregateError> {
    transaction(nodes, limits, control, |builder| {
        validate_elements(
            builder,
            elements.iter().map(|element| (element.condition, 0)),
        )?;
        let falsum = builder.push(Node::False)?;
        let truth = builder.push(Node::Implies(falsum, falsum))?;
        let empty = match extremum {
            AggregateExtremum::Min => Value::Supremum,
            AggregateExtremum::Max => Value::Infimum,
        };
        let mut inclusive = (bound == &empty).then_some(truth);
        let mut strict = None;
        for element in elements {
            for _ in 0..comparison_work(&element.value).saturating_add(comparison_work(bound)) {
                builder.tick()?;
            }
            let order = element.value.compare_terms(bound);
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

fn comparison_work(value: &Value) -> usize {
    match value {
        Value::Structured(value) => value.canonical_bytes(),
        Value::Symbol(value) | Value::String(value) => value.len().saturating_add(1),
        _ => 1,
    }
}
