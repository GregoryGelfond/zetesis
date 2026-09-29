//! Small formula theories the region propositions share.

use zetesis_ferraris::{Node, Theory};
use zetesis_theory_support::theories::theory;

/// p | q <- d.  d.  r <- not p.  :- q, r.  s | not s.
pub fn mixed() -> Theory {
    let nodes = vec![
        Node::Atom(0),        // d
        Node::Atom(1),        // p
        Node::Atom(2),        // q
        Node::Atom(3),        // r
        Node::Or(1, 2),       // p | q
        Node::Implies(0, 4),  // d -> p | q
        Node::False,          // 6
        Node::Implies(1, 6),  // not p
        Node::Implies(7, 3),  // not p -> r
        Node::And(2, 3),      // q & r
        Node::Implies(9, 6),  // :- q, r
        Node::Atom(4),        // s
        Node::Implies(11, 6), // not s
        Node::Or(11, 12),     // s | not s
    ];
    theory(5, nodes, vec![0, 5, 8, 10, 13])
}
