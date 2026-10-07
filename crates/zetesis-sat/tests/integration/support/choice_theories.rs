//! Independent choices, and a theory over another's atoms, for the
//! propositions that restrict a search.

use zetesis_ferraris::{Node, Theory};

use zetesis_theory_support::theories::theory;

/// Independent choices: a | not a, for each atom.
pub fn choices(atoms: usize) -> Theory {
    let mut nodes = vec![Node::falsum()];
    let mut roots = Vec::new();
    for atom in 0..atoms {
        let a = nodes.len();
        nodes.push(Node::atom(atom));
        nodes.push(Node::implies(a, 0));
        nodes.push(Node::or_pair([a, a + 1]));
        roots.push(a + 2);
    }
    theory(atoms, nodes, roots)
}

/// A theory over the atoms of `original`: a restriction of it.
pub fn theory_over(original: &Theory, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    theory(original.atom_count(), nodes, roots)
}

/// Three independent choices, a | not a for atoms 0 to 2, laid out with the
/// atoms as the theory's first nodes (unlike `choices(3)`).
pub fn three_choices() -> Theory {
    theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(0, 3),
            Node::or_pair([0, 4]),
            Node::implies(1, 3),
            Node::or_pair([1, 6]),
            Node::implies(2, 3),
            Node::or_pair([2, 8]),
        ],
        vec![5, 7, 9],
    )
}
