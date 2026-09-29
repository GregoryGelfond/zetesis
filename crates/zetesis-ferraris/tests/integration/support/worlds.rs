//! The worlds of a two-atom theory: its interpretations chosen by bit mask, and
//! a node's truth in one of them.

use zetesis_ferraris::{Interpretation, Node, Theory};

/// The interpretation of `theory`'s two atoms holding those whose bits `world` sets.
pub fn interpretation(theory: &Theory, world: u8) -> Interpretation {
    Interpretation::new(theory, (0..2).filter(|atom| world & (1 << atom) != 0)).unwrap()
}

/// Whether node `root` holds in `world`: in the reduct by `frozen` when one is
/// given, where every subformula false in `frozen` is false.
pub fn eval(nodes: &[Node], root: usize, world: u8, frozen: Option<u8>) -> bool {
    if frozen.is_some_and(|candidate| !eval(nodes, root, candidate, None)) {
        return false;
    }
    match nodes[root] {
        Node::False => false,
        Node::Atom(atom) => world & (1 << atom) != 0,
        Node::And(a, b) => eval(nodes, a, world, frozen) && eval(nodes, b, world, frozen),
        Node::Or(a, b) => eval(nodes, a, world, frozen) || eval(nodes, b, world, frozen),
        Node::Implies(a, b) => !eval(nodes, a, world, frozen) || eval(nodes, b, world, frozen),
    }
}
