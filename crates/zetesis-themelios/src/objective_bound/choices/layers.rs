//! A second necessary objective from the same checked disjoint key groups.
//! Eligibility addresses the unchanged exact prefix; activation uses its one
//! existing compact copy. Separate lower objectives are conjoined, never added.
//!
//! For each active group, cumulative cheaper-key disjunctions select nonnegative
//! weight increments. `ChoiceMinimumLayers.group_lower_le` bounds that value by
//! the group's exact cost under its required eligible-witness premise. Disjoint
//! allocation, coordinate transport, finite-width operations and resource-aware
//! execution are separate Rust correspondence obligations.
use std::borrow::Cow;

use zetesis_ferraris::{AggregateElement, FormulaNodes, NodeView};
use zetesis_objective::Score;

use super::super::bound::{self, Comparison};
use super::{Costs, Group, Kind, ObjectiveBoundError, ObjectiveBoundLimits, ObjectivePlan, Work};

#[derive(Clone, Copy)]
pub(super) struct Request<'a> {
    pub incumbent: &'a Score,
    pub comparison: Comparison,
    pub limits: ObjectiveBoundLimits,
}

pub(super) fn bound(
    plan: &ObjectivePlan,
    costs: &Costs,
    nodes: &mut FormulaNodes,
    copied: &[usize],
    request: Request<'_>,
    work: &mut Work<'_>,
) -> Result<usize, ObjectiveBoundError> {
    let mut levels = terms(plan, costs, nodes, copied, work)?
        .into_iter()
        .peekable();
    bound::lexicographic(
        nodes,
        plan.levels.keys().copied(),
        request.incumbent,
        request.comparison,
        request.limits,
        work,
        |priority, _| {
            if levels
                .peek()
                .is_some_and(|level| level.priority == priority)
            {
                Ok(Cow::Owned(
                    levels.next().expect("matched priority").elements,
                ))
            } else {
                Ok(Cow::Borrowed(
                    plan.levels.get(&priority).map_or(&[][..], Vec::as_slice),
                ))
            }
        },
    )
}

pub(super) struct Level {
    pub priority: i32,
    pub elements: Vec<AggregateElement>,
}

/// Use the existing checked quadratic sort-admission convention. The tariff
/// bounds preparation work; it is not an instruction/comparison-count claim.
pub(super) fn order(
    plan: &ObjectivePlan,
    groups: &mut [Group],
    work: &mut Work<'_>,
) -> Result<(), ObjectiveBoundError> {
    for group in groups {
        work.tick()?;
        let count = u64::try_from(group.keys.len()).map_err(|_| work.error(Kind::Overflow))?;
        work.charge(
            count
                .checked_mul(count)
                .ok_or_else(|| work.error(Kind::Overflow))?,
        )?;
        let original = &plan.levels[&group.priority];
        group
            .keys
            .sort_unstable_by_key(|&key| (original[key].weight, key));
    }
    Ok(())
}

/// At most one tier per allocated key plus the unallocated exact keys. Every
/// temporary row is bounded by its original priority's admitted key population.
pub(super) fn terms(
    plan: &ObjectivePlan,
    costs: &Costs,
    nodes: &mut FormulaNodes,
    copied: &[usize],
    work: &mut Work<'_>,
) -> Result<Vec<Level>, ObjectiveBoundError> {
    let mut result = work.reserve(costs.levels.len())?;
    let mut falsum = None;
    for level in &costs.levels {
        work.tick()?;
        let original = &plan.levels[&level.priority];
        let mut elements = work.reserve(original.len())?;
        for &element in &level.layer_remainder {
            work.tick()?;
            elements.push(element);
        }
        for group in &costs.layered {
            work.tick()?;
            if group.priority == level.priority {
                append_group(
                    original,
                    group,
                    copied[group.activation],
                    nodes,
                    &mut elements,
                    &mut falsum,
                    work,
                )?;
            }
        }
        work.tick()?;
        result.push(Level {
            priority: level.priority,
            elements,
        });
    }
    Ok(result)
}

pub(super) fn append_group(
    original: &[AggregateElement],
    group: &Group,
    activation: usize,
    nodes: &mut FormulaNodes,
    terms: &mut Vec<AggregateElement>,
    falsum: &mut Option<usize>,
    work: &mut Work<'_>,
) -> Result<(), ObjectiveBoundError> {
    work.tick()?;
    let mut previous = group.weight;
    assert!(!group.keys.is_empty(), "a checked group has a witness");
    assert!(
        previous >= 0,
        "only nonnegative priorities supply checked groups"
    );
    if previous != 0 {
        work.tick()?;
        terms.push(AggregateElement {
            weight: previous,
            condition: activation,
        });
    }
    let mut start = 0;
    let mut prefix = None;
    while start < group.keys.len() {
        work.tick()?;
        let mut end = start + 1;
        while end < group.keys.len() {
            work.tick()?;
            if original[group.keys[end]].weight != previous {
                break;
            }
            end += 1;
        }
        // The highest tier never belongs to a cheaper-key prefix.
        if end == group.keys.len() {
            break;
        }
        append_prefix(&group.keys[start..end], original, &mut prefix, nodes, work)?;
        let bottom = if let Some(bottom) = *falsum {
            bottom
        } else {
            let bottom = work.node(nodes, NodeView::False)?;
            *falsum = Some(bottom);
            bottom
        };
        let absent = work.node(
            nodes,
            NodeView::Implies(prefix.expect("a cheaper key exists"), bottom),
        )?;
        let condition = work.node(nodes, NodeView::And(&[activation, absent]))?;
        work.tick()?;
        let next = original[group.keys[end]].weight;
        let weight = next
            .checked_sub(previous)
            .ok_or_else(|| work.error(Kind::Overflow))?;
        work.tick()?;
        terms.push(AggregateElement { weight, condition });
        start = end;
        previous = next;
    }
    Ok(())
}

fn append_prefix(
    keys: &[usize],
    original: &[AggregateElement],
    prefix: &mut Option<usize>,
    nodes: &mut FormulaNodes,
    work: &mut Work<'_>,
) -> Result<(), ObjectiveBoundError> {
    for &key in keys {
        work.tick()?;
        let condition = original[key].condition;
        *prefix = Some(match *prefix {
            None => condition,
            Some(before) => work.node(nodes, NodeView::Or(&[before, condition]))?,
        });
    }
    Ok(())
}
