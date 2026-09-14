//! Pure recognition of one exact atomic-choice head in an admitted DAG.

use crate::{Node, Theory};

/// Return the semantic atom of `a or not a`, accepting either operand order.
/// Atom nodes may be distinct occurrences of the same semantic index. This
/// reads a bounded number of already admitted nodes, without allocation or
/// rewriting. Each consumer charges it within its root/producer inspection.
/// Cross-atom alternatives and richer classically equivalent forms decline.
pub(crate) fn atom(theory: &Theory, head: usize) -> Option<usize> {
    let Node::Or(left, right) = theory.nodes()[head] else {
        return None;
    };
    pair(theory, left, right).or_else(|| pair(theory, right, left))
}

fn pair(theory: &Theory, positive: usize, negative: usize) -> Option<usize> {
    let Node::Atom(atom) = theory.nodes()[positive] else {
        return None;
    };
    let Node::Implies(left, right) = theory.nodes()[negative] else {
        return None;
    };
    (theory.nodes()[left] == Node::Atom(atom) && theory.nodes()[right] == Node::False)
        .then_some(atom)
}
