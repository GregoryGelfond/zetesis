//! Small formula theories the region propositions share.

use zetesis_ferraris::{Node, Theory};
use zetesis_theory_support::theories::theory;

/// p | q <- d.  d.  r <- not p.  :- q, r.  s | not s.
pub fn mixed() -> Theory {
    let nodes = vec![
        Node::atom(0),           // d
        Node::atom(1),           // p
        Node::atom(2),           // q
        Node::atom(3),           // r
        Node::or_pair([1, 2]),   // p | q
        Node::implies(0, 4),     // d -> p | q
        Node::falsum(),          // 6
        Node::implies(1, 6),     // not p
        Node::implies(7, 3),     // not p -> r
        Node::and_pair([2, 3]),  // q & r
        Node::implies(9, 6),     // :- q, r
        Node::atom(4),           // s
        Node::implies(11, 6),    // not s
        Node::or_pair([11, 12]), // s | not s
    ];
    theory(5, nodes, vec![0, 5, 8, 10, 13])
}
