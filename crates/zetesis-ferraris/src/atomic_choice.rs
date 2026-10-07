//! Pure recognition of one exact atomic-choice head in an admitted DAG.

use crate::{NodeView, Theory};

/// Return the semantic atom of `a or not a`, accepting either operand order.
/// Atom nodes may be distinct occurrences of the same semantic index. This
/// reads a bounded number of already admitted nodes, without allocation or
/// rewriting. Each consumer charges it within its root/producer inspection.
/// Cross-atom alternatives and richer classically equivalent forms decline.
pub(crate) fn atom(theory: &Theory, head: usize) -> Option<usize> {
    let node = theory.view().node(head).ok()?;
    let NodeView::Or(operands) = node else {
        return None;
    };
    if operands.len() != 2 {
        return None;
    }
    let left = operands[0];
    let right = operands[1];
    pair(theory, left, right).or_else(|| pair(theory, right, left))
}

fn pair(theory: &Theory, positive: usize, negative: usize) -> Option<usize> {
    let node = theory.view().node(positive).ok()?;
    let NodeView::Atom(atom) = node else {
        return None;
    };
    let node = theory.view().node(negative).ok()?;
    let NodeView::Implies(left, right) = node else {
        return None;
    };
    if !is_atom(theory.view().node(left).ok()?, atom) {
        return None;
    }
    theory.view().node(right).ok()?.is_false().then_some(atom)
}

// Finish the slice-bearing node read before the caller branches on its shape.
fn is_atom(node: NodeView<'_>, atom: usize) -> bool {
    match node {
        NodeView::Atom(other) => other == atom,
        NodeView::False | NodeView::Implies(..) | NodeView::And(_) | NodeView::Or(_) => false,
    }
}
