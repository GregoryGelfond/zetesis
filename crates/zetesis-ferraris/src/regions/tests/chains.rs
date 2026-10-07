//! Chain preparation agrees with the maximal-tree definition of each chain.

use proptest::prelude::*;

use crate::{AdmissionLimits, Node, NodeView, Theory};

use super::super::{chain_position, chains};

/// The declarative reference keeps explicit distinct-parent lists. Unlike the
/// production collector, it expands a small chain recursively and deduplicates
/// by searching the already emitted leaves. Generated theories have at most 67
/// nodes, so this deliberately simple reference has bounded depth and cost.
fn parent_rows(theory: &Theory) -> Vec<Vec<usize>> {
    let mut parents = vec![Vec::new(); theory.nodes().len()];
    for parent in 0..theory.view().len() {
        let pair;
        let operands = match theory.view().node(parent).unwrap() {
            NodeView::And(operands) | NodeView::Or(operands) => operands,
            NodeView::Implies(a, b) => {
                pair = [a, b];
                &pair
            }
            NodeView::Atom(_) | NodeView::False => continue,
        };
        for &operand in operands {
            if !parents[operand].contains(&parent) {
                parents[operand].push(parent);
            }
        }
    }
    parents
}

fn interior(theory: &Theory, parents: &[Vec<usize>], index: usize) -> bool {
    let [parent] = parents[index].as_slice() else {
        return false;
    };
    !theory.roots().contains(&index)
        && matches!(
            (
                theory.view().node(index).unwrap(),
                theory.view().node(*parent).unwrap()
            ),
            (NodeView::And(..), NodeView::And(..)) | (NodeView::Or(..), NodeView::Or(..))
        )
}

fn leaves(theory: &Theory, parents: &[Vec<usize>], index: usize, result: &mut Vec<usize>) {
    let (NodeView::And(operands) | NodeView::Or(operands)) = theory.view().node(index).unwrap()
    else {
        panic!("a chain has a connective at its root");
    };
    for &operand in operands {
        if interior(theory, parents, operand) {
            leaves(theory, parents, operand, result);
        } else if !result.contains(&operand) {
            result.push(operand);
        }
    }
}

fn compare_definition(theory: &Theory) {
    let parents = parent_rows(theory);
    let (actual, links, absorbed) = chains(theory);
    let mut expected = Vec::new();
    let mut expected_links = vec![None; theory.nodes().len()];
    for root in 0..theory.view().len() {
        let node = theory.view().node(root).unwrap();
        assert_eq!(absorbed[root], interior(theory, &parents, root));
        if !absorbed[root] && matches!(node, NodeView::And(..) | NodeView::Or(..)) {
            let mut operands = Vec::new();
            leaves(theory, &parents, root, &mut operands);
            expected_links[root] = Some(expected.len());
            expected.push((root, matches!(node, NodeView::Or(..)), operands));
        }
    }
    assert_eq!(
        actual
            .into_iter()
            .map(|chain| (chain.root, chain.disjunction, chain.operands))
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        links
            .into_iter()
            .map(|link| link.map(chain_position))
            .collect::<Vec<_>>(),
        expected_links
    );
}

proptest! {
    #[test]
    fn chains_equal_the_maximal_tree_definition(
        descriptions in prop::collection::vec(any::<(u8, u8, u8)>(), 0..64),
        roots in prop::collection::vec(any::<u8>(), 0..16),
    ) {
        // Separate nodes can carry the same atom; leaf identity is the node.
        let mut nodes = vec![Node::atom(0), Node::atom(0), Node::atom(1), Node::falsum()];
        for (kind, left, right) in descriptions {
            let a = usize::from(left) % nodes.len();
            let b = usize::from(right) % nodes.len();
            nodes.push(match kind % 5 {
                0 => Node::and_pair([a, b]),
                1 => Node::or_pair([a, b]),
                2 => Node::implies(a, b),
                3 => Node::atom(a % 2),
                _ => Node::falsum(),
            });
        }
        let roots = roots.into_iter().map(|root| usize::from(root) % nodes.len()).collect();
        let theory = Theory::new(2,
crate::FormulaParts::new(nodes, vec![]).unwrap(), roots, AdmissionLimits::default()).unwrap();
        compare_definition(&theory);
    }
}

#[test]
fn chain_operands_keep_first_occurrence_order() {
    let theory = Theory::new(
        2,
        crate::FormulaParts::new(
            vec![
                crate::Node::atom(0),
                crate::Node::atom(0),
                crate::Node::atom(1),
                crate::Node::or_pair([0, 1]),
                crate::Node::or_pair([3, 2]),
                crate::Node::or_pair([3, 4]),
                crate::Node::or_pair([5, 5]),
            ],
            vec![],
        )
        .unwrap(),
        vec![6],
        AdmissionLimits::default(),
    )
    .unwrap();
    let (actual, _, _) = chains(&theory);
    // Shared node 3 stays opaque in chain 6, even though both parents belong
    // to that chain. Its first occurrence precedes leaf 2; the later occurrence
    // and repeated direct child 5 contribute no additional operand.
    assert_eq!(actual.len(), 2);
    assert_eq!(actual[0].root, 3);
    assert_eq!(actual[0].operands, [0, 1]);
    assert_eq!(actual[1].root, 6);
    assert_eq!(actual[1].operands, [3, 2]);
    compare_definition(&theory);
}

#[test]
fn deep_chains_preserve_operand_order() {
    let atoms = 16_384;
    for disjunction in [false, true] {
        for reversed in [false, true] {
            let mut nodes: Vec<_> = (0..atoms).map(Node::atom).collect();
            let mut root = 0;
            for atom in 1..atoms {
                let (a, b) = if reversed { (atom, root) } else { (root, atom) };
                root = nodes.len();
                nodes.push(if disjunction {
                    Node::or_pair([a, b])
                } else {
                    Node::and_pair([a, b])
                });
            }
            let theory = Theory::new(
                atoms,
                crate::FormulaParts::new(nodes, vec![]).unwrap(),
                vec![root],
                AdmissionLimits::default(),
            )
            .unwrap();
            let (actual, links, absorbed) = chains(&theory);
            assert_eq!(actual.len(), 1);
            assert_eq!(actual[0].root, root);
            assert_eq!(actual[0].disjunction, disjunction);
            let mut expected: Vec<_> = (0..atoms).collect();
            if reversed {
                expected.reverse();
            }
            assert_eq!(actual[0].operands, expected);
            assert_eq!(links.iter().filter(|link| link.is_some()).count(), 1);
            assert_eq!(links[root].map(chain_position), Some(0));
            assert_eq!(absorbed.iter().filter(|&&value| value).count(), atoms - 2);
        }
    }
}

#[test]
fn native_wide_groups_preserve_shared_parent_boundaries() {
    for width in [3, 65, 129] {
        let mut operands: Vec<_> = (0..width).map(|position| position % 2).collect();
        operands.extend((0..width).map(|position| if position % 2 == 0 { 3 } else { 2 }));
        let theory = Theory::new(
            3,
            crate::FormulaParts::new(
                vec![
                    Node::atom(0),
                    Node::atom(1),
                    Node::atom(2),
                    Node::or_span(crate::OperandSpan {
                        start: 0,
                        length: width,
                    }),
                    Node::or_span(crate::OperandSpan {
                        start: width,
                        length: width,
                    }),
                    Node::or_pair([3, 4]),
                ],
                operands,
            )
            .unwrap(),
            vec![5],
            AdmissionLimits::default(),
        )
        .unwrap();
        let (actual, _, _) = chains(&theory);
        assert_eq!(theory.nodes().len(), 6);
        assert_eq!(actual.len(), 2);
        assert_eq!(
            (actual[0].root, actual[0].operands.as_slice()),
            (3, &[0, 1][..])
        );
        assert_eq!(
            (actual[1].root, actual[1].operands.as_slice()),
            (5, &[3, 2][..])
        );
        compare_definition(&theory);
    }
}
