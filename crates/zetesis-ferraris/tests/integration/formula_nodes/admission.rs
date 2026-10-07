//! Consumed topology evidence removes only already completed child scans.

use proptest::prelude::*;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionError, AdmissionLimits, AggregateFamilyLimits, FormulaNodes, FormulaParts,
    Interpretation, Limits, Node, NodeView, OperandSpan, Theory, models, models_reduct,
};

use super::{copy, push, scan};

fn fixture() -> FormulaNodes {
    let mut nodes = FormulaNodes::default();
    push(&mut nodes, NodeView::False);
    push(&mut nodes, NodeView::Atom(0));
    push(&mut nodes, NodeView::Atom(1));
    push(&mut nodes, NodeView::Implies(1, 0));
    push(&mut nodes, NodeView::Or(&[1, 3, 2, 1]));
    push(&mut nodes, NodeView::And(&[2, 4]));
    push(&mut nodes, NodeView::Implies(5, 3));
    nodes
}

fn raw(
    nodes: &FormulaNodes,
    atoms: usize,
    roots: Vec<usize>,
    limits: AdmissionLimits,
) -> Result<Theory, AdmissionError> {
    Theory::new(atoms, copy(nodes).into_parts(), roots, limits)
}

#[test]
fn admission_reuses_only_completed_topology() {
    let nodes = fixture();
    let n = nodes.view().len() as u128;
    let edges = nodes.parts().occurrences() as u128;
    let raw = copy(&nodes).prepare_admission(2, vec![6, 4], AdmissionLimits::default());
    let checked = nodes.prepare_admission(2, vec![6, 4], AdmissionLimits::default());
    assert_eq!(checked.work(), 2 * n + 2);
    assert_eq!(raw.work() - checked.work(), edges);
}

#[test]
fn admission_transfers_the_supplied_buffers() {
    let nodes = fixture();
    let roots = vec![6, 4, 6];
    let addresses = (
        nodes.parts().nodes().as_ptr(),
        nodes.parts().operands().as_ptr(),
        roots.as_ptr(),
    );
    let theory = nodes
        .prepare_admission(2, roots, AdmissionLimits::default())
        .admit()
        .unwrap();
    assert_eq!(theory.nodes().as_ptr(), addresses.0);
    assert_eq!(theory.operands().as_ptr(), addresses.1);
    assert_eq!(theory.roots().as_ptr(), addresses.2);
}

#[test]
fn final_dimensions_remain_independent() {
    let limits = AdmissionLimits {
        max_atoms: 2,
        max_nodes: 7,
        max_roots: 2,
        max_operands: 10,
    };
    fixture()
        .prepare_admission(2, vec![6, 4], limits)
        .admit()
        .unwrap();
    for limited in [
        AdmissionLimits {
            max_atoms: 1,
            ..limits
        },
        AdmissionLimits {
            max_nodes: 6,
            ..limits
        },
        AdmissionLimits {
            max_roots: 1,
            ..limits
        },
        AdmissionLimits {
            max_operands: 9,
            ..limits
        },
    ] {
        let nodes = fixture();
        assert_eq!(
            raw(&nodes, 2, vec![6, 4], limited).unwrap_err(),
            AdmissionError::Limit
        );
        assert_eq!(
            nodes
                .prepare_admission(2, vec![6, 4], limited)
                .admit()
                .unwrap_err(),
            AdmissionError::Limit
        );
    }
}

#[test]
fn final_admission_preserves_identity_errors() {
    for (atoms, roots, expected) in [
        (1, vec![6], AdmissionError::Atom),
        (2, vec![7], AdmissionError::Root),
        (1, vec![7], AdmissionError::Atom),
    ] {
        let nodes = fixture();
        assert_eq!(
            raw(&nodes, atoms, roots.clone(), AdmissionLimits::default()).unwrap_err(),
            expected
        );
        assert_eq!(
            nodes
                .prepare_admission(atoms, roots, AdmissionLimits::default())
                .admit()
                .unwrap_err(),
            expected
        );
    }
}

#[test]
fn final_padding_refusal_precedes_atom_checks() {
    let limits = AdmissionLimits {
        max_atoms: usize::MAX,
        ..AdmissionLimits::default()
    };
    let nodes = fixture();
    assert_eq!(
        raw(&nodes, usize::MAX, vec![usize::MAX], limits).unwrap_err(),
        AdmissionError::Limit
    );
    assert_eq!(
        nodes
            .prepare_admission(usize::MAX, vec![usize::MAX], limits)
            .admit()
            .unwrap_err(),
        AdmissionError::Limit
    );
}

#[test]
fn raw_reentry_discards_completed_topology() {
    let nodes = fixture();
    let expected = 2 * nodes.view().len() as u128 + nodes.parts().occurrences() as u128 + 1;
    let raw = FormulaNodes::new(nodes.into_parts());
    assert_eq!(
        raw.prepare_admission(2, vec![6], AdmissionLimits::default())
            .work(),
        expected
    );
}

#[test]
fn unchecked_prefix_retains_raw_error_order() {
    let cases = [
        (
            vec![Node::atom(2), Node::implies(1, 0)],
            AdmissionError::Atom,
        ),
        (
            vec![Node::implies(0, 0), Node::atom(2)],
            AdmissionError::Edge,
        ),
        (
            vec![Node::and_span(OperandSpan {
                start: 0,
                length: 2,
            })],
            AdmissionError::Arity,
        ),
        (
            vec![Node::and_span(OperandSpan {
                start: usize::MAX,
                length: 3,
            })],
            AdmissionError::Span,
        ),
    ];
    for (stored, expected) in cases {
        let original = FormulaParts::new(stored.clone(), vec![]).unwrap();
        assert_eq!(
            Theory::new(1, original, vec![0], AdmissionLimits::default()).unwrap_err(),
            expected
        );
        let mut nodes = FormulaNodes::new(FormulaParts::new(stored, vec![]).unwrap());
        // A valid later append cannot certify an unchecked earlier node.
        push(&mut nodes, NodeView::False);
        assert_eq!(
            nodes
                .prepare_admission(1, vec![0], AdmissionLimits::default())
                .admit()
                .unwrap_err(),
            expected
        );
    }
}

fn incomplete_prefix() -> FormulaNodes {
    FormulaNodes::new(
        FormulaParts::new(
            vec![
                Node::falsum(),
                Node::and_span(OperandSpan {
                    start: 0,
                    length: 3,
                }),
            ],
            vec![],
        )
        .unwrap(),
    )
}

fn repair(transaction: &mut zetesis_ferraris::FormulaTransaction<'_>) {
    transaction.push(NodeView::And(&[0, 0, 0]), 10, 10).unwrap();
    transaction
        .append_aggregate_family(
            &[],
            &[],
            AggregateFamilyLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
}

#[test]
fn restored_prefix_cannot_retain_temporary_validation() {
    for detach in [false, true] {
        let mut nodes = incomplete_prefix();
        let mut transaction = nodes.transaction();
        repair(&mut transaction);
        if detach {
            transaction.detach().unwrap();
        } else {
            drop(transaction);
        }
        assert_eq!(
            nodes
                .prepare_admission(0, vec![1], AdmissionLimits::default())
                .admit()
                .unwrap_err(),
            AdmissionError::Span
        );
    }
}

#[test]
fn nested_rollback_restores_admission_evidence() {
    let mut nodes = incomplete_prefix();
    let mut outer = nodes.transaction();
    {
        let mut inner = outer.transaction();
        repair(&mut inner);
    }
    outer.commit();
    assert_eq!(
        nodes
            .prepare_admission(0, vec![1], AdmissionLimits::default())
            .admit()
            .unwrap_err(),
        AdmissionError::Span
    );
}

#[test]
fn checked_raw_spans_retain_independent_occurrence_bounds() {
    // Two roots share one operand row; unused raw arena cells remain bounded.
    for max_operands in [5, 6] {
        let mut nodes = FormulaNodes::new(
            FormulaParts::new(
                vec![
                    Node::falsum(),
                    Node::and_span(OperandSpan {
                        start: 0,
                        length: 3,
                    }),
                    Node::or_span(OperandSpan {
                        start: 0,
                        length: 3,
                    }),
                ],
                vec![0, 0, 0, usize::MAX, usize::MAX],
            )
            .unwrap(),
        );
        scan(&mut nodes, super::AggregateLimits::default()).unwrap();
        let limits = AdmissionLimits {
            max_operands,
            ..AdmissionLimits::default()
        };
        let expected = raw(&nodes, 0, vec![1, 2], limits);
        let actual = nodes.prepare_admission(0, vec![1, 2], limits).admit();
        if max_operands == 5 {
            assert_eq!(expected.unwrap_err(), AdmissionError::Limit);
            assert_eq!(actual.unwrap_err(), AdmissionError::Limit);
        } else {
            assert_eq!(
                actual.unwrap().parts().occurrences(),
                expected.unwrap().parts().occurrences()
            );
        }
    }
}

#[test]
fn unused_arena_cells_remain_bounded() {
    let mut nodes = FormulaNodes::new(FormulaParts::new(vec![Node::falsum()], vec![0; 4]).unwrap());
    scan(&mut nodes, super::AggregateLimits::default()).unwrap();
    let limits = AdmissionLimits {
        max_operands: 3,
        ..AdmissionLimits::default()
    };
    assert_eq!(
        nodes
            .prepare_admission(0, vec![0], limits)
            .admit()
            .unwrap_err(),
        AdmissionError::Limit
    );
}

fn world(theory: &Theory, mask: u8) -> Interpretation {
    Interpretation::new(theory, (0..2).filter(|atom| mask & (1 << atom) != 0)).unwrap()
}

proptest! {
    #[test]
    fn checked_admission_preserves_formula_semantics(
        instructions in prop::collection::vec(any::<(u8, u8, u8)>(), 0..24),
        roots in prop::collection::vec(any::<u8>(), 0..5),
    ) {
        let mut nodes = fixture();
        for (operation, left, width) in instructions {
            let count = nodes.view().len();
            let row: Vec<_> = (0..2 + usize::from(width % 6)).map(|offset| (usize::from(left) + offset) % count).collect();
            push(&mut nodes, match operation % 5 {
                0 => NodeView::False,
                1 => NodeView::Atom(usize::from(left % 2)),
                2 => NodeView::Implies(row[0], row[1]),
                3 => NodeView::And(&row),
                _ => NodeView::Or(&row),
            });
        }
        let roots: Vec<_> = roots.into_iter().map(|root| usize::from(root) % nodes.view().len()).collect();
        let expected = raw(&nodes, 2, roots.clone(), AdmissionLimits::default()).unwrap();
        let actual = nodes.prepare_admission(2, roots, AdmissionLimits::default()).admit().unwrap();
        prop_assert_eq!(actual.nodes(), expected.nodes());
        prop_assert_eq!(actual.operands(), expected.operands());
        prop_assert_eq!(actual.roots(), expected.roots());
        let cancellation = Cancellation::default();
        for candidate in 0..4 {
            prop_assert_eq!(models(&actual, &world(&actual, candidate), Limits::default(), &cancellation).unwrap(),
                models(&expected, &world(&expected, candidate), Limits::default(), &cancellation).unwrap());
            for tested in 0..4 {
                prop_assert_eq!(models_reduct(&actual, &world(&actual, candidate), &world(&actual, tested), Limits::default(), &cancellation).unwrap(),
                    models_reduct(&expected, &world(&expected, candidate), &world(&expected, tested), Limits::default(), &cancellation).unwrap());
            }
        }
    }
}
