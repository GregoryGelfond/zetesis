//! Native positive numeric bodies retain ranked support through native device rows.

use super::*;
use zetesis_ferraris::{
    ExtremumCutRule, ExtremumRow, FormulaNodes, FormulaParts, NodeView, Threshold,
};

pub(super) fn qualify(oracle: &mut GpuTightOracle) {
    qualify_shared(oracle);
    for (weights, bound) in [
        ([1, 2], 1),
        ([1, 2], 2),
        ([1, 2], 3),
        ([1, 2], 4),
        ([u64::from(u32::MAX), 1], i128::from(u32::MAX) + 1),
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
    for rank_count in [1u64 << 32, (1u64 << 32) + 1] {
        for rule in [ExtremumCutRule::Prefix(1), ExtremumCutRule::Suffix(1)] {
            qualify_ordinal(oracle, rule, rank_count, &[0, rank_count - 1]);
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

// Nonconstant owners follow constant guards; a parent of the owner precedes a
// later guard of the same row. Full-width carries, repeated conditions and zero
// weights must preserve support under both serial and scheduled original truth.
fn qualify_shared(oracle: &mut GpuTightOracle) {
    use zetesis_ferraris::{NumericDomain, NumericRow, ThresholdDescriptor};
    let theory = Theory::new(
        3,
        FormulaParts::from_numeric_raw(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::atom(2),
                Node::threshold(0),
                Node::threshold(1),
                Node::implies(4, 1),
                Node::threshold(2),
                Node::implies(6, 2),
                Node::threshold(3),
            ],
            vec![],
            vec![0, 0, 0],
            vec![u64::MAX, 1, 0],
            vec![NumericRow {
                start: 0,
                length: 3,
                domain: NumericDomain::Weights,
            }],
            [0, 1, i128::from(u64::MAX) + 1, i128::MAX]
                .into_iter()
                .map(|bound| ThresholdDescriptor { row: 0, bound })
                .collect(),
            vec![],
        )
        .unwrap(),
        vec![0, 5, 7],
        AdmissionLimits::default(),
    )
    .unwrap();
    let certificate = TightPlan::compile(
        &theory,
        TightPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    compare_shared_schedules(oracle, &certificate);
    qualify_shared_extrema(oracle);
}

fn qualify_shared_extrema(oracle: &mut GpuTightOracle) {
    let mut nodes = FormulaNodes::new(
        FormulaParts::new(vec![Node::atom(0), Node::atom(1), Node::atom(2)], vec![]).unwrap(),
    );
    let mut tx = nodes.transaction();
    let family = tx
        .push_extremum_cut_family_range(
            ExtremumRow {
                conditions: &[0, 0, 0],
                ranks: &[1u64 << 32, u64::MAX - 1, 0],
                rank_count: u64::MAX,
            },
            &[
                ExtremumCutRule::Prefix(0),
                ExtremumCutRule::Suffix(u64::MAX),
                ExtremumCutRule::Prefix(1),
                ExtremumCutRule::Suffix(1u64 << 32),
                ExtremumCutRule::Prefix(u64::MAX),
                ExtremumCutRule::Suffix(0),
            ],
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    let mut roots = vec![0];
    for (guard, head) in [(family.start + 2, 1), (family.start + 3, 2)] {
        roots.push(
            tx.push(NodeView::Implies(guard, head), usize::MAX, usize::MAX)
                .unwrap(),
        );
    }
    tx.commit();
    let theory = Theory::new(3, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap();
    let certificate = TightPlan::compile(
        &theory,
        TightPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    compare_shared_schedules(oracle, &certificate);
}

// Reuse the same resident graph with exactly the serial work allowance. This
// exercises an early owner's writes to later guards when no level barriers are
// selected, then compares the actual result against the scalar certificate.
fn compare_shared_schedules(oracle: &mut GpuTightOracle, certificate: &TightPlan) {
    assert_eq!(compare(oracle, certificate), 18);
    let previous = *oracle.last_batch_stats().unwrap();
    assert!(previous.truth_levels > 0);
    assert!(previous.candidates > 0);
    let serial_work = previous.work / previous.candidates - u64::from(previous.truth_levels);
    let candidates = inputs(certificate.theory());
    let results = oracle
        .check_batch(
            certificate,
            &candidates,
            TightGpuLimits {
                max_work_per_candidate: serial_work,
                ..TightGpuLimits::default()
            },
            &Cancellation::default(),
        )
        .unwrap();
    let stats = oracle.last_batch_stats().unwrap();
    assert_eq!(stats.truth_levels, 0);
    assert!(!stats.theory_uploaded);
    assert_eq!(results.len(), candidates.len());
    for (candidate, result) in candidates.iter().zip(&results) {
        assert_eq!(result.work(), serial_work);
        let expected = certificate
            .check(
                candidate,
                TightCheckLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(result.verdict(), expected.verdict);
    }
}
