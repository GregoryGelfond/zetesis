//! Small Ferraris theories, and their interpretations chosen by bit mask.

use zetesis_ferraris::{AdmissionLimits, FormulaParts, Interpretation, Node, Theory};

/// The theory over `atoms` atoms with `nodes` and `roots`, under the default
/// admission limits. This small-fixture helper accepts inline nodes; theories
/// with arena-backed groups use [`FormulaParts`] directly.
///
/// # Panics
/// Panics if the theory is refused, as a node naming an absent atom or node
/// is.
#[must_use]
pub fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    let parts = FormulaParts::new(nodes, Vec::new()).unwrap();
    Theory::new(atoms, parts, roots, AdmissionLimits::default()).unwrap()
}

/// The theory of the one fact `a`: one atom, which is its one root.
#[must_use]
pub fn fact() -> Theory {
    theory(1, vec![Node::atom(0)], vec![0])
}

/// The interpretation of `theory` holding the atoms whose bits `mask` sets.
///
/// # Panics
/// Panics if `theory` has more atoms than a mask has bits.
#[must_use]
pub fn interpretation(theory: &Theory, mask: usize) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|atom| mask & (1 << atom) != 0),
    )
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theory_holds_the_atoms_and_nodes_it_is_built_from() {
        let built = theory(
            2,
            vec![Node::atom(0), Node::atom(1), Node::or_pair([0, 1])],
            vec![2],
        );
        assert_eq!(built.atom_count(), 2);
        assert_eq!(built.nodes().len(), 3);
    }

    #[test]
    fn the_fact_theory_roots_its_one_atom() {
        let built = fact();
        assert_eq!(built.atom_count(), 1);
        assert_eq!(built.nodes(), [Node::atom(0)]);
        assert_eq!(built.roots(), [0]);
    }

    #[test]
    fn an_interpretation_holds_the_atoms_its_mask_sets() {
        let built = theory(3, vec![Node::atom(0), Node::atom(1), Node::atom(2)], vec![]);
        let chosen = interpretation(&built, 0b101);
        assert_eq!(chosen.atoms().collect::<Vec<_>>(), [0, 2]);
    }
}
