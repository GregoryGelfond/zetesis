//! Native positive numeric bodies retain ranked support through physical lowering.

use super::*;
use zetesis_ferraris::{
    ExtremumCutRule, ExtremumRow, FormulaNodes, FormulaParts, NodeView, Threshold,
};

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
    for rule in [
        ExtremumCutRule::Prefix(0),
        ExtremumCutRule::Prefix(2),
        ExtremumCutRule::Prefix(4),
        ExtremumCutRule::Suffix(0),
        ExtremumCutRule::Suffix(2),
        ExtremumCutRule::Suffix(4),
    ] {
        qualify_ordinal(oracle, rule);
    }
}

fn qualify_ordinal(oracle: &mut GpuTightOracle, rule: ExtremumCutRule) {
    let mut nodes = FormulaNodes::new(
        FormulaParts::new(vec![Node::atom(0), Node::atom(1), Node::atom(2)], vec![]).unwrap(),
    );
    let mut transaction = nodes.transaction();
    let mut roots = vec![0];
    for atom in 0..2 {
        let body = transaction
            .push_extremum_cut_family_range(
                ExtremumRow {
                    conditions: &[atom, atom],
                    ranks: &[1, 3],
                    rank_count: 4,
                },
                &[rule],
                usize::MAX,
                usize::MAX,
            )
            .unwrap()
            .start;
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
