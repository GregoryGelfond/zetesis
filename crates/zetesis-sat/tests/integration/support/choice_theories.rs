//! Independent choices, and a theory over another's atoms, for the
//! propositions that restrict a search.

use zetesis_ferraris::{Node, Theory};

use super::formula_theories::theory;

/// Independent choices: a | not a, for each atom.
pub fn choices(atoms: usize) -> Theory {
    let mut nodes = vec![Node::False];
    let mut roots = Vec::new();
    for atom in 0..atoms {
        let a = nodes.len();
        nodes.push(Node::Atom(atom));
        nodes.push(Node::Implies(a, 0));
        nodes.push(Node::Or(a, a + 1));
        roots.push(a + 2);
    }
    theory(atoms, nodes, roots)
}

/// A theory over the atoms of `original`: a restriction of it.
pub fn theory_over(original: &Theory, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    theory(original.atom_count(), nodes, roots)
}
