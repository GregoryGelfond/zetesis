//! Prepare the fixed activation closure once, independently of any incumbent.
//!
//! Compact nodes use the existing formula representation. Group activations
//! change from original coordinates to this prepared owner's coordinates only
//! after the complete copy succeeds. Atom identities never change.

use zetesis_ferraris::{
    AdmissionError, FormulaNodes, FormulaParts, Node, NodeView, OperandSpan, Theory,
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
        match original
            .view()
            .node(index)
            .map_err(|error| work.error(Kind::Theory(error)))?
        {
            NodeView::And(row) | NodeView::Or(row) => {
                for &child in row {
                    work.tick()?;
                    needed[child] = true;
                }
            }
            NodeView::Implies(left, right) => {
                work.charge(2)?;
                needed[left] = true;
                needed[right] = true;
            }
            NodeView::Atom(_) | NodeView::False => {}
        }
    }
    let (node_count, arena_count) = sizes(original, &needed, work)?;
    let mut nodes = memory.reserve(node_count, work)?;
    let mut operands = memory.reserve(arena_count, work)?;
    for (index, &required) in needed.iter().enumerate() {
        work.tick()?;
        if !required {
            continue;
        }
        let source = original
            .view()
            .node(index)
            .map_err(|error| work.error(Kind::Theory(error)))?;
        let mapped = copy_node(source, &copied, &mut operands, work)?;
        copied[index] = Some(nodes.len());
        nodes.push(mapped);
    }
    // FormulaParts recounts logical occurrences without allocating.
    work.charge(u64::try_from(nodes.len()).map_err(|_| work.error(Kind::Overflow))?)?;
    let parts =
        FormulaParts::new(nodes, operands).map_err(|error| work.error(Kind::Theory(error)))?;
    for group in groups {
        work.tick()?;
        group.activation = copied[group.activation].expect("marked activation");
    }
    // These maps end with preparation; later residual allocation overlaps the
    // retained closure, not the temporary original-to-compact coordinates.
    memory.retire(&needed);
    memory.retire(&copied);
    work.statistics.nodes = parts.view().len();
    Ok(parts)
}

/// Count exact vector lengths before allocating the compact closure. Logical
/// occurrence limits still include inline pairs and implication edges.
fn sizes(
    original: &Theory,
    needed: &[bool],
    work: &mut Work<'_>,
) -> Result<(usize, usize), ObjectiveBoundError> {
    let mut nodes = 0_usize;
    let mut arena = 0_usize;
    let mut occurrences = 0_usize;
    for (index, &required) in needed.iter().enumerate() {
        work.tick()?;
        if !required {
            continue;
        }
        nodes = nodes
            .checked_add(1)
            .ok_or_else(|| work.error(Kind::Overflow))?;
        let edges = match original
            .view()
            .node(index)
            .map_err(|error| work.error(Kind::Theory(error)))?
        {
            NodeView::And(row) | NodeView::Or(row) => {
                if row.len() >= 3 {
                    arena = arena
                        .checked_add(row.len())
                        .ok_or_else(|| work.error(Kind::Overflow))?;
                }
                row.len()
            }
            NodeView::Implies(_, _) => 2,
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
    Ok((nodes, arena))
}

/// Copy only admitted shapes, retaining ordered and repeated operands. The
/// backwards closure and original topology establish every preceding mapping.
fn copy_node(
    source: NodeView<'_>,
    copied: &[Option<usize>],
    operands: &mut Vec<usize>,
    work: &mut Work<'_>,
) -> Result<Node, ObjectiveBoundError> {
    let mapped = |index: usize| copied[index].expect("marked topological child");
    Ok(match source {
        NodeView::Atom(atom) => Node::atom(atom),
        NodeView::False => Node::falsum(),
        NodeView::Implies(left, right) => {
            work.charge(2)?;
            Node::implies(mapped(left), mapped(right))
        }
        NodeView::And(row) | NodeView::Or(row) => {
            work.charge(u64::try_from(row.len()).map_err(|_| work.error(Kind::Overflow))?)?;
            if let &[left, right] = row {
                if matches!(source, NodeView::And(_)) {
                    Node::and_pair([mapped(left), mapped(right)])
                } else {
                    Node::or_pair([mapped(left), mapped(right)])
                }
            } else if row.len() >= 3 {
                let span = OperandSpan {
                    start: operands.len(),
                    length: row.len(),
                };
                for &child in row {
                    operands.push(mapped(child));
                }
                if matches!(source, NodeView::And(_)) {
                    Node::and_span(span)
                } else {
                    Node::or_span(span)
                }
            } else {
                return Err(work.error(Kind::Theory(AdmissionError::Arity)));
            }
        }
    })
}

/// Append the prepared closure after an exact objective prefix. Its atom nodes
/// map to the prefix's canonical atom slots; other nodes keep a local mapping.
/// No operation traverses the original program here.
pub(super) fn append(
    prepared: &FormulaParts,
    nodes: &mut FormulaNodes,
    work: &mut Work<'_>,
) -> Result<Vec<usize>, ObjectiveBoundError> {
    let mut copied = work.reserve(prepared.view().len())?;
    for index in 0..prepared.view().len() {
        work.tick()?;
        let source = prepared
            .view()
            .node(index)
            .map_err(|error| work.error(Kind::Theory(error)))?;
        let mapped = match source {
            NodeView::Atom(atom) => atom,
            NodeView::False => work.node(nodes, NodeView::False)?,
            NodeView::Implies(left, right) => {
                work.node(nodes, NodeView::Implies(copied[left], copied[right]))?
            }
            NodeView::And(row) | NodeView::Or(row) => {
                let mut operands = work.reserve(row.len())?;
                for &child in row {
                    work.tick()?;
                    operands.push(copied[child]);
                }
                work.node(
                    nodes,
                    if matches!(source, NodeView::And(_)) {
                        NodeView::And(&operands)
                    } else {
                        NodeView::Or(&operands)
                    },
                )?
            }
        };
        copied.push(mapped);
    }
    Ok(copied)
}
