//! Fixed admitted input for the four-atom historical candidate trace.
//!
//! Captured from `1 { p(1..4) } 2.` at
//! `54f1ed4afbb373048300d1934d58190a5e0118c5`; see the adjacent README.
//! Node order is part of this execution fixture, not a grounding contract.

use zetesis_ferraris::Node;

pub(super) const ATOMS: usize = 4;
pub(super) const NODES: [Node; 45] = [
    Node::falsum(),
    Node::implies(0, 0),
    Node::atom(0),
    Node::atom(1),
    Node::atom(2),
    Node::atom(3),
    Node::implies(2, 0),
    Node::or_pair([2, 6]),
    Node::implies(1, 7),
    Node::implies(3, 0),
    Node::or_pair([3, 9]),
    Node::implies(1, 10),
    Node::implies(4, 0),
    Node::or_pair([4, 12]),
    Node::implies(1, 13),
    Node::implies(5, 0),
    Node::or_pair([5, 15]),
    Node::implies(1, 16),
    Node::or_pair([2, 3]),
    Node::or_pair([18, 4]),
    Node::or_pair([19, 5]),
    Node::and_pair([3, 2]),
    Node::and_pair([4, 18]),
    Node::or_pair([21, 22]),
    Node::and_pair([4, 21]),
    Node::and_pair([5, 19]),
    Node::or_pair([23, 25]),
    Node::and_pair([5, 23]),
    Node::or_pair([24, 27]),
    Node::implies(28, 0),
    Node::and_pair([20, 29]),
    Node::implies(30, 0),
    Node::implies(31, 0),
    Node::implies(2, 1),
    Node::implies(33, 0),
    Node::implies(34, 0),
    Node::implies(3, 1),
    Node::implies(36, 0),
    Node::implies(37, 0),
    Node::implies(4, 1),
    Node::implies(39, 0),
    Node::implies(40, 0),
    Node::implies(5, 1),
    Node::implies(42, 0),
    Node::implies(43, 0),
];
pub(super) const ROOTS: [usize; 9] = [8, 11, 14, 17, 32, 35, 38, 41, 44];
