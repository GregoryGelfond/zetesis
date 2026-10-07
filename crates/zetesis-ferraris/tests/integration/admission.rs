//! Admission preserves storage order and reports the earliest structural refusal.

use proptest::prelude::*;
use zetesis_ferraris::{AdmissionError, AdmissionLimits, Node, Theory};

#[test]
fn dimension_refusal_precedes_node_validation() {
    let exact = AdmissionLimits {
        max_atoms: 1,
        max_nodes: 1,
        max_roots: 1,
        max_operands: 0,
    };
    for limits in [
        AdmissionLimits {
            max_atoms: 0,
            ..exact
        },
        AdmissionLimits {
            max_nodes: 0,
            ..exact
        },
        AdmissionLimits {
            max_roots: 0,
            ..exact
        },
    ] {
        assert_eq!(
            Theory::new(
                1,
                zetesis_ferraris::FormulaParts::new(vec![Node::atom(1)], vec![]).unwrap(),
                vec![1],
                limits
            )
            .unwrap_err(),
            AdmissionError::Limit,
        );
    }
}

#[test]
fn padded_count_refusal_precedes_node_validation() {
    let limits = AdmissionLimits {
        max_atoms: usize::MAX,
        max_nodes: 1,
        max_roots: 1,
        max_operands: 0,
    };
    assert_eq!(
        Theory::new(
            usize::MAX - 62,
            zetesis_ferraris::FormulaParts::new(vec![Node::atom(usize::MAX)], vec![]).unwrap(),
            vec![1],
            limits
        )
        .unwrap_err(),
        AdmissionError::Limit,
    );
}

#[test]
fn last_representable_padded_count_is_admitted() {
    let atoms = usize::MAX - 63;
    let theory = Theory::new(
        atoms,
        zetesis_ferraris::FormulaParts::new(vec![], vec![]).unwrap(),
        vec![],
        AdmissionLimits {
            max_atoms: usize::MAX,
            ..AdmissionLimits::default()
        },
    )
    .unwrap();
    assert_eq!(theory.atom_count(), atoms);
}

#[test]
fn either_nonpreceding_operand_is_refused() {
    for node in [
        Node::and_pair([0, 1]),
        Node::and_pair([1, 0]),
        Node::or_pair([0, 1]),
        Node::or_pair([1, 0]),
        Node::implies(0, 1),
        Node::implies(1, 0),
    ] {
        assert_eq!(
            Theory::new(
                0,
                zetesis_ferraris::FormulaParts::new(vec![Node::falsum(), node], vec![]).unwrap(),
                vec![0],
                AdmissionLimits::default()
            )
            .unwrap_err(),
            AdmissionError::Edge,
        );
    }
}

#[test]
fn unasserted_atoms_are_validated() {
    assert_eq!(
        Theory::new(
            0,
            zetesis_ferraris::FormulaParts::new(vec![Node::atom(0)], vec![]).unwrap(),
            vec![],
            AdmissionLimits::default()
        )
        .unwrap_err(),
        AdmissionError::Atom,
    );
}

proptest! {
    #[test]
    fn first_malformed_node_determines_refusal(prefix in 0usize..64, atom_first in any::<bool>()) {
        // Insert two independently known errors after a valid prefix. Reversing
        // their source order must reverse the reported error; invalid roots
        // remain later than either node error.
        let mut nodes = vec![Node::falsum(); prefix];
        let (first, second, expected) = if atom_first {
            (Node::atom(1), Node::and_pair([prefix + 1, 0]), AdmissionError::Atom)
        } else {
            (Node::and_pair([prefix, 0]), Node::atom(1), AdmissionError::Edge)
        };
        nodes.extend([first, second]);
        let roots = vec![nodes.len()];
        prop_assert_eq!(
            Theory::new(1,
zetesis_ferraris::FormulaParts::new(nodes, vec![]).unwrap(), roots, AdmissionLimits::default()).unwrap_err(),
            expected,
        );
    }
}
