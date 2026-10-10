//! Native positive thresholds retain ranked support through physical lowering.

use super::*;
use zetesis_ferraris::{FormulaNodes, FormulaParts, NodeView, Threshold};

pub(super) fn qualify(oracle: &mut GpuTightOracle) {
    for bound in [1, 2, 3, 4] {
        let mut nodes = FormulaNodes::new(
            FormulaParts::new(vec![Node::atom(0), Node::atom(1), Node::atom(2)], vec![]).unwrap(),
        );
        let mut transaction = nodes.transaction();
        let mut roots = vec![0];
        for atom in 0..2 {
            let body = transaction
                .push(
                    NodeView::Threshold(Threshold {
                        conditions: &[atom, atom],
                        weights: &[1, 2],
                        bound,
                    }),
                    usize::MAX,
                    usize::MAX,
                )
                .unwrap();
            roots.push(
                transaction
                    .push(NodeView::Implies(body, atom + 1), usize::MAX, usize::MAX)
                    .unwrap(),
            );
        }
        transaction.commit();
        let theory = Theory::new(3, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap();
        let certificate = TightPlan::compile(
            &theory,
            TightPlanLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(compare(oracle, &certificate), 18);
    }
}
