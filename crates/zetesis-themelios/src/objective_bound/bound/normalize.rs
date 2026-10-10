//! Classical candidate guards only: never rewrite the original reduct theory.

use std::borrow::Cow;

use zetesis_ferraris::{AggregateElement, FormulaNodes, NodeView};

use super::{Kind, ObjectiveBoundError, ObjectiveBoundLimits, Work};

#[derive(Default)]
struct Shape {
    offset: i128,
    total: i128,
    negative: usize,
    nonzero: usize,
    minimum: Option<i128>,
}

pub(in crate::objective_bound) fn prepare<'a>(
    elements: &'a [AggregateElement],
    bound: i64,
    falsum: usize,
    nodes: &mut FormulaNodes,
    limits: ObjectiveBoundLimits,
    work: &mut Work<'_>,
) -> Result<(Cow<'a, [AggregateElement]>, i64), ObjectiveBoundError> {
    let original = || (Cow::Borrowed(elements), bound);
    work.tick()?;
    // Leave the kernel's typed element refusal intact, before any temporary copy.
    if elements.len() > limits.aggregate.max_elements {
        return Ok(original());
    }
    let mut shape = Shape::default();
    for element in elements {
        work.tick()?;
        let weight = i128::from(element.weight);
        if element.weight == i32::MIN {
            return Ok(original());
        }
        if weight < 0 {
            shape.offset = shape
                .offset
                .checked_add(weight)
                .ok_or_else(|| work.error(Kind::Overflow))?;
            shape.negative += 1;
        }
        if weight != 0 {
            shape.nonzero += 1;
            shape.minimum = Some(shape.minimum.map_or(weight.abs(), |n| n.min(weight.abs())));
        }
        shape.total = shape
            .total
            .checked_add(weight.abs())
            .ok_or_else(|| work.error(Kind::Overflow))?;
    }
    if shape.negative == 0 {
        return Ok(original());
    }
    let shifted = i128::from(bound)
        .checked_sub(shape.offset)
        .ok_or_else(|| work.error(Kind::Overflow))?;
    let Ok(shifted) = i64::try_from(shifted) else {
        return Ok(original());
    };
    work.tick()?;
    if !shape.fits(shifted, limits) {
        return Ok(original());
    }
    let mut normalized = work.reserve(elements.len())?;
    for element in elements {
        work.tick()?;
        normalized.push(if element.weight < 0 {
            AggregateElement {
                weight: element
                    .weight
                    .checked_neg()
                    .ok_or_else(|| work.error(Kind::Overflow))?,
                condition: work.node(nodes, NodeView::Implies(element.condition, falsum))?,
            }
        } else {
            *element
        });
    }
    // Each entry is a distinct original complete key, even if its transformed
    // weight or eligibility matches another entry. Never deduplicate this list.
    Ok((Cow::Owned(normalized), shifted))
}

impl Shape {
    fn fits(&self, shifted: i64, limits: ObjectiveBoundLimits) -> bool {
        // Lt(k), Eq(k) request G(k), G(k), G(k+1). Extreme thresholds
        // reduce to constants, conjunctions or disjunctions without numeric
        // state. Interior thresholds share the two contribution columns.
        let minimum = self.minimum.unwrap_or(0);
        let shifted = i128::from(shifted);
        let thresholds = [shifted, shifted, shifted + 1]
            .into_iter()
            .filter(|&bound| bound > minimum && bound <= self.total - minimum)
            .count();
        let states = if thresholds == 0 {
            0
        } else {
            2 * self.nonzero as u128 + thresholds as u128
        };
        // Only representation compatibility decides this choice. Actual node,
        // operand and work admission belongs to the shared builder, so an
        // estimate cannot reject a construction at its exact inclusive limit.
        states <= limits.aggregate.max_states as u128
    }
}
