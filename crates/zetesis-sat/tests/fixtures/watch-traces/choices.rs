//! Fixed admitted input for the four-atom historical candidate trace.
//!
//! Captured from `1 { p(1..4) } 2.` at
//! `54f1ed4afbb373048300d1934d58190a5e0118c5`; see the adjacent README.
//! Node order is part of this execution fixture, not a grounding contract.

use zetesis_ferraris::Node;

pub(super) const ATOMS: usize = 4;
pub(super) const NODES: [Node; 45] = [
    Node::False,
    Node::Implies(0, 0),
    Node::Atom(0),
    Node::Atom(1),
    Node::Atom(2),
    Node::Atom(3),
    Node::Implies(2, 0),
    Node::Or(2, 6),
    Node::Implies(1, 7),
    Node::Implies(3, 0),
    Node::Or(3, 9),
    Node::Implies(1, 10),
    Node::Implies(4, 0),
    Node::Or(4, 12),
    Node::Implies(1, 13),
    Node::Implies(5, 0),
    Node::Or(5, 15),
    Node::Implies(1, 16),
    Node::Or(2, 3),
    Node::Or(18, 4),
    Node::Or(19, 5),
    Node::And(3, 2),
    Node::And(4, 18),
    Node::Or(21, 22),
    Node::And(4, 21),
    Node::And(5, 19),
    Node::Or(23, 25),
    Node::And(5, 23),
    Node::Or(24, 27),
    Node::Implies(28, 0),
    Node::And(20, 29),
    Node::Implies(30, 0),
    Node::Implies(31, 0),
    Node::Implies(2, 1),
    Node::Implies(33, 0),
    Node::Implies(34, 0),
    Node::Implies(3, 1),
    Node::Implies(36, 0),
    Node::Implies(37, 0),
    Node::Implies(4, 1),
    Node::Implies(39, 0),
    Node::Implies(40, 0),
    Node::Implies(5, 1),
    Node::Implies(42, 0),
    Node::Implies(43, 0),
];
pub(super) const ROOTS: [usize; 9] = [8, 11, 14, 17, 32, 35, 38, 41, 44];
