//! Pure recognition of one exact atomic-choice head in an admitted DAG.

use crate::{Node, Theory};

/// Return the semantic atom of `a or not a`, accepting either operand order.
/// Atom nodes may be distinct occurrences of the same semantic index. This
/// reads a bounded number of already admitted nodes, without allocation or
/// rewriting. Each consumer charges it within its root/producer inspection.
/// Cross-atom alternatives and richer classically equivalent forms decline.
pub(crate) fn atom(theory: &Theory, head: usize) -> Option<usize> {
    let node = theory.nodes()[head];
    let Node::Or(left, right) = node else {
        return None;
    };
    pair(theory, left, right).or_else(|| pair(theory, right, left))
}

fn pair(theory: &Theory, positive: usize, negative: usize) -> Option<usize> {
    let node = theory.nodes()[positive];
    let Node::Atom(atom) = node else {
        return None;
    };
    let node = theory.nodes()[negative];
    let Node::Implies(left, right) = node else {
        return None;
    };
    (theory.nodes()[left] == Node::Atom(atom) && theory.nodes()[right] == Node::False)
        .then_some(atom)
}
