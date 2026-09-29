//! Small Ferraris theories.

use zetesis_ferraris::{AdmissionLimits, Node, Theory};

/// The theory over `atoms` atoms with `nodes` and `roots`, under the default
/// admission limits.
///
/// # Panics
/// Panics if the theory is refused, as a node naming an absent atom or node
/// is.
#[must_use]
pub fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(atoms, nodes, roots, AdmissionLimits::default()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theory_holds_the_atoms_and_nodes_it_is_built_from() {
        let built = theory(
            2,
            vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)],
            vec![2],
        );
        assert_eq!(built.atom_count(), 2);
        assert_eq!(built.nodes().len(), 3);
    }
}
