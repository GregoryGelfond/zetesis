//! Native semantic owners exercise physical threshold packing on the device.

use super::*;
use zetesis_ferraris::{FormulaNodes, FormulaParts};

pub(super) fn qualify(oracle: &mut GpuFormulaOracle) {
    for bound in -1..=8 {
        let mut nodes = FormulaNodes::new(
            FormulaParts::new(
                vec![
                    Node::atom(0),
                    Node::atom(1),
                    Node::falsum(),
                    Node::implies(0, 2),
                    Node::implies(3, 1),
                ],
                vec![],
            )
            .unwrap(),
        );
        let mut transaction = nodes.transaction();
        let guards = transaction
            .push_threshold_family(
                &[0, 0, 1, 3, 4],
                &[1, 2, 0, 2, 1],
                &[bound, bound + 1],
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        let different = transaction
            .push(
                NodeView::Implies(guards[0], guards[1]),
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        let root = transaction
            .push(
                NodeView::Or(&[different, guards[0], 1]),
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        transaction.commit();
        let theory = Theory::new(
            2,
            nodes.into_parts(),
            vec![root],
            zetesis_ferraris::AdmissionLimits::default(),
        )
        .unwrap();
        let candidates = candidates(&theory);
        for inputs in [candidates.clone(), candidates.into_iter().rev().collect()] {
            let actual = oracle
                .propagate_batch(&theory, &inputs, FormulaLimits::default())
                .unwrap();
            for (candidate, result) in inputs.iter().zip(actual) {
                let expected = check(
                    &theory,
                    candidate,
                    Limits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
                assert_eq!(
                    matches!(result.verdict(), FormulaVerdict::NotModel),
                    matches!(expected.verdict(), Verdict::NotModel { .. })
                );
                if matches!(result.verdict(), FormulaVerdict::NoProperSubset) {
                    assert!(expected.accepted());
                }
            }
        }
    }
}
