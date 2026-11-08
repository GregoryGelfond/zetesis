//! Independent choices, and a theory over another's atoms, for the
//! propositions that restrict a search.

use zetesis_ferraris::{AdmissionLimits, FormulaNodes, FormulaParts, Node, NodeView, Theory};

use zetesis_theory_support::theories::theory;

/// Independent choices: a | not a, for each atom.
pub(crate) fn choices(atoms: usize) -> Theory {
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
pub(crate) fn theory_over(original: &Theory, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    theory(original.atom_count(), nodes, roots)
}

/// Three independent choices, a | not a for atoms 0 to 2, laid out with the
/// atoms as the theory's first nodes (unlike `choices(3)`).
pub(crate) fn three_choices() -> Theory {
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

/// Three independent choices, a | not a for atoms 0 to 2, laid out with falsum
/// first, then the atoms, then each choice's implication and disjunction.
pub(crate) fn three_choices_falsum_first() -> Theory {
    theory(
        3,
        vec![
            Node::falsum(),
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::implies(1, 0),
            Node::or_pair([1, 4]),
            Node::implies(2, 0),
            Node::or_pair([2, 6]),
            Node::implies(3, 0),
            Node::or_pair([3, 8]),
        ],
        vec![5, 7, 9],
    )
}

/// {a}. {b}. :- a, b. :- not a, not b.
/// The clauses give the nonunit parity row. An equivalent native threshold
/// interval 1 <= count{a,b} <= 1 gives explicit recognized C1 lower/upper rows.
pub(crate) fn exclusive_choices() -> Theory {
    let mut nodes = FormulaNodes::new(
        FormulaParts::new(
            vec![
                Node::atom(0),          // a
                Node::atom(1),          // b
                Node::falsum(),         // 2
                Node::implies(0, 2),    // not a
                Node::implies(1, 2),    // not b
                Node::or_pair([0, 3]),  // a | not a
                Node::or_pair([1, 4]),  // b | not b
                Node::and_pair([0, 1]), // a & b
                Node::and_pair([3, 4]), // not a & not b
                Node::implies(7, 2),    // :- a, b
                Node::implies(8, 2),    // :- not a, not b
            ],
            vec![],
        )
        .unwrap(),
    );
    let mut transaction = nodes.transaction();
    let bounds = transaction
        .push_threshold_family(&[0, 1], &[1, 1], &[1, 2], usize::MAX, usize::MAX)
        .unwrap();
    let at_most_one = transaction
        .push(NodeView::Implies(bounds[1], 2), usize::MAX, usize::MAX)
        .unwrap();
    transaction.commit();
    Theory::new(
        2,
        nodes.into_parts(),
        vec![5, 6, 9, 10, bounds[0], at_most_one],
        AdmissionLimits::default(),
    )
    .unwrap()
}
