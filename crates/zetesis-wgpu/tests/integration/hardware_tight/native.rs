//! Native positive numeric bodies retain ranked support through native device rows.

use super::*;
use zetesis_ferraris::{
    ExtremumCutRule, ExtremumRow, FormulaNodes, FormulaParts, NodeView, Threshold,
};

pub(super) fn qualify(oracle: &mut GpuTightOracle) {
    for (weights, bound) in [
        ([1, 2], 1),
        ([1, 2], 2),
        ([1, 2], 3),
        ([1, 2], 4),
        ([u64::MAX, u64::MAX], i128::MIN),
        ([u64::MAX, 0], 0),
        ([u64::MAX, u64::MAX], 1i128 << 32),
        ([u64::MAX, u64::MAX], i128::from(u64::MAX) + 1),
        ([u64::MAX, u64::MAX], i128::from(u64::MAX) * 2),
        ([u64::MAX, u64::MAX], i128::MAX),
    ] {
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
                        weights: &weights,
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
        qualify_ordinal(oracle, rule, 4, &[1, 3]);
    }
    for bound in [0, 1, 1u64 << 32, (1u64 << 32) + 1, u64::MAX - 1, u64::MAX] {
        for rule in [
            ExtremumCutRule::Prefix(bound),
            ExtremumCutRule::Suffix(bound),
        ] {
            qualify_ordinal(oracle, rule, u64::MAX, &[1u64 << 32, u64::MAX - 1]);
        }
    }
}

fn qualify_ordinal(
    oracle: &mut GpuTightOracle,
    rule: ExtremumCutRule,
    rank_count: u64,
    ranks: &[u64],
) {
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
                    ranks,
                    rank_count,
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
