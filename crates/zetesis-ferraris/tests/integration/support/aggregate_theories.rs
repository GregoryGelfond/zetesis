//! The comparisons, extrema and node prefix from which the aggregate
//! propositions build their theories.

use zetesis_ferraris::{AggregateComparison as Comparison, AggregateExtremum as Extremum, Node};

/// Every comparison.
pub const COMPARISONS: [Comparison; 6] = [
    Comparison::Eq,
    Comparison::Ne,
    Comparison::Lt,
    Comparison::Le,
    Comparison::Gt,
    Comparison::Ge,
];

/// Both extrema.
pub const EXTREMA: [Extremum; 2] = [Extremum::Min, Extremum::Max];

/// The first nodes of every aggregate theory: false, true, the atoms a and b,
/// not a, not not a, not b, a and b, a or b, a implies b, and a or not a.
pub fn prefix() -> Vec<Node> {
    vec![
        Node::False,
        Node::Implies(0, 0),
        Node::Atom(0),
        Node::Atom(1),
        Node::Implies(2, 0),
        Node::Implies(4, 0),
        Node::Implies(3, 0),
        Node::And(2, 3),
        Node::Or(2, 3),
        Node::Implies(2, 3),
        Node::Or(2, 4),
    ]
}

/// Appends `node` to `nodes`, returning its index.
pub fn push(nodes: &mut Vec<Node>, node: Node) -> usize {
    let index = nodes.len();
    nodes.push(node);
    index
}
