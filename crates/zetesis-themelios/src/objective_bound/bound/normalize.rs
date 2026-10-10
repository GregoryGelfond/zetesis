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
        shape.nonzero += usize::from(weight != 0);
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
    if !shape.fits(elements.len(), shifted, nodes, limits, work) {
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
    fn fits(
        &self,
        count: usize,
        shifted: i64,
        prefix: &FormulaNodes,
        limits: ObjectiveBoundLimits,
        work: &Work<'_>,
    ) -> bool {
        let shifted = i128::from(shifted);
        // Exact maximum requested by this caller's Lt(k), Eq(k) pair. Widen
        // before the successor; out-of-range thresholds are Boolean constants.
        let maximum = if shifted >= 0 && shifted < self.total {
            shifted + 1
        } else if shifted > 0 && shifted == self.total {
            shifted
        } else {
            0
        };
        let Ok(maximum) = u128::try_from(maximum) else {
            return false;
        };
        let count = count as u128;
        let negative = self.negative as u128;
        let prefix_nodes = prefix.view().len() as u128;
        let prefix_operands = prefix.parts().occurrences() as u128;
        let cells = maximum * self.nonzero as u128;
        let states = if maximum == 0 { 0 } else { 2 * (maximum + 1) };
        // Reject before multiplying the work estimate; selected cell counts
        // are bounded by the caller's addressable node capacity.
        if states > limits.aggregate.max_states as u128
            || cells > limits.aggregate.max_nodes as u128 / 2
        {
            return false;
        }
        // Two scans of 2^n subsets are mandatory on the signed path, independent
        // of which subsets fail. Cell count is a scheduling heuristic, not a
        // runtime guarantee; an unrepresentable subset count is already refused.
        let subset_visits = u32::try_from(count)
            .ok()
            .and_then(|power| 1u128.checked_shl(power))
            .and_then(|subsets| subsets.checked_mul(2))
            .unwrap_or(u128::MAX);
        let nodes = prefix_nodes + negative + 2 * cells + 7;
        let operands = prefix_operands + 2 * negative + 4 * cells + 12;
        let row_work = if maximum == 0 {
            0
        } else {
            maximum + 1 + count + 7 * cells
        };
        // Checked construction retains prefix validation. Charge input visits,
        // constants, both guard scans and their connective operand visits.
        let family_work = 18 + count + row_work;
        let normalize_work = count + 3 * negative;
        let remaining = u128::from(limits.max_work - work.statistics.work);
        cells <= subset_visits
            && nodes <= limits.aggregate.max_nodes as u128
            && operands <= limits.aggregate.max_operands as u128
            && family_work <= u128::from(limits.aggregate.max_work)
            // Two suffix appends; admission recounts nodes, checks topology
            // and visits the sole root. Each edge occurrence remains charged.
            && normalize_work + family_work + 6 + 2 * nodes + operands < remaining
    }
}
