//! Weighted sufficient evidence and shared activation-copy boundaries.

use std::collections::BTreeMap;

use super::*;
use zetesis_ferraris::{AdmissionLimits, FormulaTransaction, Interpretation, Limits, models};
use zetesis_objective::ObjectiveProgram;

fn push(transaction: &mut FormulaTransaction<'_>, node: NodeView<'_>) -> usize {
    transaction.push(node, usize::MAX, usize::MAX).unwrap()
}

fn formula_nodes() -> FormulaNodes {
    let mut nodes = FormulaNodes::default();
    let mut transaction = nodes.transaction();
    for atom in 0..3 {
        push(&mut transaction, NodeView::Atom(atom));
    }
    transaction.commit();
    nodes
}

#[test]
fn sufficient_evidence_uses_weights_and_multiplicity() {
    for facts in 0..4 {
        let mut original = formula_nodes();
        let mut transaction = original.transaction();
        let falsum = push(&mut transaction, NodeView::False);
        let truth = push(&mut transaction, NodeView::Implies(falsum, falsum));
        transaction.commit();
        let original = Theory::new(
            3,
            original.into_parts(),
            (0..2).filter(|atom| facts & (1 << atom) != 0).collect(),
            AdmissionLimits::default(),
        )
        .unwrap();
        let mut nodes = formula_nodes();
        let bounds = [-1, 0, 4, 6, 9, 10];
        let thresholds = nodes
            .push_threshold_family(
                &[0, 0, 1, 2],
                &[2, 3, 4, 0],
                &bounds,
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        let plan = ObjectivePlan {
            original,
            objectives: ObjectiveProgram::none(),
            nodes: nodes.into_parts(),
            levels: BTreeMap::new(),
            choices: None,
            statistics: ObjectiveBoundStatistics::default(),
        };
        let cancellation = Cancellation::default();
        let mut work = Work {
            limits: ObjectivePlanLimits::default(),
            cancellation: &cancellation,
            template: None,
            statistics: ObjectiveBoundStatistics::default(),
        };
        let mut memory = Storage {
            bytes: 0,
            limit: u128::MAX,
        };
        let graph = Implications::new(&plan, &mut memory, &mut work).unwrap();
        let mut proof = Proof::new(&graph, &mut memory, &mut work).unwrap();
        let expected = if facts & 1 != 0 { 5 } else { 0 } + if facts & 2 != 0 { 4 } else { 0 };
        for (&node, &bound) in thresholds.iter().zip(&bounds) {
            proof.assume(&graph, truth, 2, &mut work).unwrap();
            assert_eq!(
                proof
                    .proves(&graph, graph.objective + node, &mut work)
                    .unwrap(),
                expected >= bound,
                "facts={facts}, bound={bound}"
            );
        }
    }
}

#[test]
fn activation_copies_preserve_shared_numeric_rows() {
    let mut original = formula_nodes();
    let thresholds = original
        .push_threshold_family(&[0, 0, 1], &[2, 3, 4], &[4, 6], usize::MAX, usize::MAX)
        .unwrap();
    let original =
        Theory::new(3, original.into_parts(), vec![], AdmissionLimits::default()).unwrap();
    let mut groups: Vec<_> = thresholds
        .iter()
        .map(|&activation| Group {
            priority: 0,
            activation,
            weight: 1,
            keys: vec![],
        })
        .collect();
    let cancellation = Cancellation::default();
    let mut work = Work {
        limits: ObjectivePlanLimits::default(),
        cancellation: &cancellation,
        template: None,
        statistics: ObjectiveBoundStatistics::default(),
    };
    let mut memory = Storage {
        bytes: 0,
        limit: u128::MAX,
    };
    let prepared = activations::prepare(&original, &mut groups, &mut memory, &mut work).unwrap();
    assert_eq!(prepared.nodes().len(), 4);
    assert_eq!(prepared.rows().len(), 1);
    assert_eq!(prepared.conditions(), [0, 0, 1]);
    assert_eq!(memory.bytes, prepared.retained_bytes());
    let mut appended = formula_nodes();
    let mapped = activations::append(&prepared, &mut appended, &mut work).unwrap();
    assert_eq!(appended.parts().rows().len(), 1);
    assert_eq!(appended.parts().conditions(), [3, 3, 4]);
    let copied = Theory::new(
        3,
        appended.into_parts(),
        groups
            .iter()
            .map(|group| mapped[group.activation])
            .collect(),
        AdmissionLimits::default(),
    )
    .unwrap();
    for mask in 0..8 {
        let candidate =
            Interpretation::new(&copied, (0..3).filter(|atom| mask & (1 << atom) != 0)).unwrap();
        let value = if mask & 1 != 0 { 5 } else { 0 } + if mask & 2 != 0 { 4 } else { 0 };
        assert_eq!(
            models(&copied, &candidate, Limits::default(), &cancellation).unwrap(),
            value >= 6
        );
    }
}
