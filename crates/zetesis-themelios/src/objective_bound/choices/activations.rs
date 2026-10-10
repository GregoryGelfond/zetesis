//! Prepare the fixed activation closure once, independently of any incumbent.
//!
//! Complete-storage copies keep numeric row sharing while condition coordinates
//! change. Semantic atom identities remain those of the original theory.

use zetesis_ferraris::{
    ContributionRow, FormulaNodes, FormulaParts, Node, NodeView, Theory, ThresholdDescriptor,
};

use super::{Group, Kind, ObjectiveBoundError, Resource, Storage, Work};

pub(super) fn prepare(
    original: &Theory,
    groups: &mut [Group],
    memory: &mut Storage,
    work: &mut Work<'_>,
) -> Result<FormulaParts, ObjectiveBoundError> {
    let length = original.view().len();
    let mut needed = memory.reserve(length, work)?;
    let mut copied = memory.reserve(length, work)?;
    for _ in 0..length {
        work.tick()?;
        needed.push(false);
        copied.push(None);
    }
    for group in groups.iter() {
        work.tick()?;
        needed[group.activation] = true;
    }
    for index in (0..length).rev() {
        work.tick()?;
        if !needed[index] {
            continue;
        }
        let pair;
        let children = match original
            .view()
            .node(index)
            .map_err(|error| work.error(Kind::Theory(error)))?
        {
            NodeView::And(row) | NodeView::Or(row) => row,
            NodeView::Threshold(threshold) => threshold.conditions,
            NodeView::Implies(left, right) => {
                pair = [left, right];
                &pair
            }
            NodeView::Atom(_) | NodeView::False => &[],
        };
        for &child in children {
            work.tick()?;
            needed[child] = true;
        }
    }
    let sizes = sizes(original, &needed, memory, work)?;
    let mut next = 0;
    for (index, &required) in needed.iter().enumerate() {
        work.tick()?;
        if required {
            copied[index] = Some(next);
            next += 1;
        }
    }
    memory.check(sizes.payload + sizes.minimum_scratch, work)?;
    work.charge(u64::try_from(sizes.work).map_err(|_| work.error(Kind::Overflow))?)?;
    let (parts, peak) = original
        .view()
        .try_select_with_peak(&copied)
        .map_err(|error| work.error(Kind::Theory(error)))?;
    // Admit the observed output plus hidden row/descriptor map capacities while
    // they coexist. Only the output remains resident after selection returns.
    memory.check(peak, work)?;
    memory.bytes += parts.retained_bytes();
    for group in groups {
        work.tick()?;
        group.activation = copied[group.activation].expect("marked activation");
    }
    memory.retire(&needed);
    memory.retire(&copied);
    work.statistics.nodes = parts.view().len();
    Ok(parts)
}

struct Sizes {
    payload: u128,
    minimum_scratch: u128,
    work: u128,
}

/// Count shared numeric rows and descriptors once while retaining every logical
/// threshold occurrence for the formula's independent admission ceiling.
fn sizes(
    original: &Theory,
    needed: &[bool],
    memory: &mut Storage,
    work: &mut Work<'_>,
) -> Result<Sizes, ObjectiveBoundError> {
    let source = original.parts();
    let mut rows = memory.reserve(source.rows().len(), work)?;
    let mut descriptors = memory.reserve(source.thresholds().len(), work)?;
    for _ in source.rows() {
        work.tick()?;
        rows.push(false);
    }
    for _ in source.thresholds() {
        work.tick()?;
        descriptors.push(false);
    }
    let mut nodes = 0_usize;
    let mut occurrences = 0_usize;
    let mut payload = 0_u128;
    let mut copying =
        needed.len() as u128 * 2 + 2 * (rows.len() as u128 + descriptors.len() as u128);
    for (index, &required) in needed.iter().enumerate() {
        work.tick()?;
        if !required {
            continue;
        }
        nodes = nodes
            .checked_add(1)
            .ok_or_else(|| work.error(Kind::Overflow))?;
        payload += size_of::<Node>() as u128;
        copying += 2;
        let edges = match original
            .view()
            .node(index)
            .map_err(|error| work.error(Kind::Theory(error)))?
        {
            NodeView::And(row) | NodeView::Or(row) => {
                if row.len() >= 3 {
                    payload += row.len() as u128 * size_of::<usize>() as u128;
                }
                copying += row.len() as u128;
                row.len()
            }
            NodeView::Threshold(threshold) => {
                let descriptor = source.nodes()[index]
                    .threshold_descriptor()
                    .expect("threshold storage");
                if !descriptors[descriptor] {
                    descriptors[descriptor] = true;
                    payload += size_of::<ThresholdDescriptor>() as u128;
                    copying += 1;
                    let row = source.thresholds()[descriptor].row;
                    if !rows[row] {
                        rows[row] = true;
                        payload += size_of::<ContributionRow>() as u128
                            + threshold.conditions.len() as u128
                                * (size_of::<usize>() + size_of::<u64>()) as u128;
                        copying += 1 + threshold.conditions.len() as u128 * 2;
                    }
                }
                threshold.conditions.len()
            }
            NodeView::Implies(_, _) => {
                copying += 2;
                2
            }
            NodeView::Atom(_) | NodeView::False => 0,
        };
        occurrences = occurrences
            .checked_add(edges)
            .ok_or_else(|| work.error(Kind::Overflow))?;
    }
    if nodes > work.limits.max_nodes {
        return Err(work.limit(Resource::Nodes));
    }
    if occurrences > work.limits.max_operands {
        return Err(work.limit(Resource::Operands));
    }
    let minimum_scratch =
        (rows.len() as u128 + descriptors.len() as u128) * size_of::<Option<usize>>() as u128;
    memory.retire(&rows);
    memory.retire(&descriptors);
    Ok(Sizes {
        payload,
        minimum_scratch,
        work: copying,
    })
}

/// Append the prepared closure after an exact objective prefix. All compact
/// formula nodes are copied; atom occurrences retain their semantic identities.
/// The numeric arenas remain shared across every bound in the prepared owner.
/// No operation traverses the original program here.
pub(super) fn append(
    prepared: &FormulaParts,
    nodes: &mut FormulaNodes,
    work: &mut Work<'_>,
) -> Result<Vec<usize>, ObjectiveBoundError> {
    let first = nodes.view().len();
    let end = first
        .checked_add(prepared.view().len())
        .ok_or_else(|| work.error(Kind::Overflow))?;
    if end > work.limits.max_nodes {
        return Err(work.limit(Resource::Nodes));
    }
    let mut copied = work.reserve(prepared.view().len())?;
    for index in first..end {
        work.tick()?;
        copied.push(index);
    }
    let cells = prepared.nodes().len() as u128
        + prepared.operands().len() as u128
        + prepared.conditions().len() as u128
        + prepared.weights().len() as u128
        + prepared.rows().len() as u128
        + prepared.thresholds().len() as u128;
    // Complete remapping copy, numeric validation, destination-edge validation
    // and complete arena append each have their own charged visits.
    let target = nodes.parts();
    let prefix_check = if prepared.rows().is_empty() && prepared.thresholds().is_empty() {
        0
    } else {
        target.pending_numeric_validation_work()
    };
    let copying = prefix_check
        + 2 * cells
        + 4 * prepared.nodes().len() as u128
        + 2 * prepared.occurrences() as u128
        + prepared
            .rows()
            .iter()
            .map(|row| 1 + row.length as u128)
            .sum::<u128>()
        + prepared.thresholds().len() as u128;
    work.charge(u64::try_from(copying).map_err(|_| work.error(Kind::Overflow))?)?;
    let parts = prepared
        .view()
        .try_remap(&copied)
        .map_err(|error| work.error(Kind::Theory(error)))?;
    let mut transaction = nodes.transaction();
    transaction
        .append(
            parts.view(),
            work.limits.max_nodes,
            work.limits.max_operands,
        )
        .map_err(|error| work.error(Kind::Theory(error)))?;
    transaction.commit();
    work.statistics.nodes = nodes.view().len();
    Ok(copied)
}
