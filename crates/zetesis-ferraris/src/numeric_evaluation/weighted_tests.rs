//! Shared nonnegative sums keep per-guard truth and per-pass row identity.

use super::{Measure, NumericRowScratch};
use crate::test_support::world;
use crate::{
    AdmissionLimits, EvaluationError, EvaluationLimits, EvaluationWorkspace, ExtremumCutRule,
    ExtremumRow, FormulaNodes, FormulaParts, FrozenReduct, Limits, Node, NodeView, Theory,
    Threshold, TightCheckLimits, TightPlan, TightPlanLimits, TightVerdict, TightWorkspace,
};
use zetesis_cpu::{Cancellation, Stop};

fn weighted(bounds: &[i128], checked: bool) -> Theory {
    let mut nodes = FormulaNodes::default();
    let mut transaction = nodes.transaction();
    transaction.push(NodeView::Atom(0), 100, 100).unwrap();
    transaction.push(NodeView::Atom(1), 100, 100).unwrap();
    transaction
        .push_threshold_family_range(&[0, 0, 1], &[2, u64::MAX, 0], bounds, 100, 100)
        .unwrap();
    transaction.commit();
    if checked {
        nodes
            .prepare_admission(2, vec![], AdmissionLimits::default())
            .admit()
            .unwrap()
    } else {
        Theory::new(2, nodes.into_parts(), vec![], AdmissionLimits::default()).unwrap()
    }
}

fn threshold(theory: &Theory, node: usize) -> Threshold<'_> {
    let NodeView::Threshold(row) = theory.view().node(node).unwrap() else {
        panic!("threshold fixture");
    };
    row
}

#[test]
fn weighted_family_reads_each_occurrence_once_per_pass() {
    for checked in [false, true] {
        for bounds in [vec![1], vec![0, 2, 3, i128::from(u64::MAX) + 2, i128::MAX]] {
            let theory = weighted(&bounds, checked);
            let mut workspace = EvaluationWorkspace::default();
            for (pass, mask) in [0, 1, 2, 3, 1].into_iter().enumerate() {
                let candidate = world(&theory, mask);
                let attempt = workspace.evaluate(
                    &candidate,
                    EvaluationLimits::default(),
                    &Cancellation::default(),
                );
                // N nodes, three physical occurrences, one validity word;
                // exactly one fresh measure cell on the first pass.
                assert_eq!(
                    attempt.work,
                    u64::try_from(theory.nodes().len() + 4).unwrap() + u64::from(pass == 0)
                );
                let truth = attempt.result.unwrap();
                let sum = if mask & 1 == 0 {
                    0
                } else {
                    i128::from(u64::MAX) + 2
                };
                for (offset, &bound) in bounds.iter().enumerate() {
                    assert_eq!(truth.node_truth(2 + offset), Some(sum >= bound));
                }
            }
        }
    }
}

#[test]
fn weighted_read_failures_do_not_publish_cached_measures() {
    let theory = weighted(&[1, i128::MAX], false);
    let mut scratch = NumericRowScratch::default();
    scratch.reserve(&theory).unwrap();
    scratch.reset(&theory, || Ok::<_, Stop>(())).unwrap();
    let mut reads = 0;
    let result = scratch.threshold(&theory, 2, threshold(&theory, 2), |_| {
        reads += 1;
        if reads == 3 {
            Err(Stop::Cancelled)
        } else {
            Ok(true)
        }
    });
    assert_eq!(result, Err(Stop::Cancelled));
    assert_eq!(
        reads, 3,
        "zero weight remains a charged callback occurrence"
    );
    assert_eq!(scratch.valid, [0]);
    reads = 0;
    assert!(
        scratch
            .threshold(&theory, 2, threshold(&theory, 2), |_| {
                reads += 1;
                Ok::<_, Stop>(true)
            })
            .unwrap()
    );
    assert_eq!(reads, 3);
    assert!(
        !scratch
            .threshold(&theory, 3, threshold(&theory, 3), |_| {
                Err(Stop::Cancelled)
            })
            .unwrap()
    );
}

#[test]
fn changing_an_unmeasured_sum_does_not_publish_it() {
    let theory = weighted(&[3], false);
    let mut scratch = NumericRowScratch::default();
    scratch.reserve(&theory).unwrap();
    scratch.reset(&theory, || Ok::<_, Stop>(())).unwrap();
    scratch.change_condition(&theory, 0, 0, true).unwrap();
    scratch.change_condition(&theory, 0, 1, true).unwrap();
    assert_eq!(scratch.valid, [0]);
    let mut reads = 0;
    assert!(
        scratch
            .threshold(&theory, 2, threshold(&theory, 2), |_| {
                reads += 1;
                Ok::<_, Stop>(true)
            })
            .unwrap()
    );
    assert_eq!(reads, 3);
    // The two repeated occurrences must both leave the sum; the zero-weight
    // occurrence is still present and has its own authoritative position.
    for occurrence in 0..3 {
        scratch
            .change_condition(&theory, 0, occurrence, false)
            .unwrap();
    }
    assert!(
        !scratch
            .threshold(
                &theory,
                2,
                threshold(&theory, 2),
                |_| -> Result<bool, Stop> {
                    panic!("a completed sum update must not rescan its row")
                }
            )
            .unwrap()
    );
}

#[test]
fn every_fresh_weighted_work_prefix_can_retry() {
    let theory = weighted(&[1, 2, 3], true);
    let candidate = world(&theory, 3);
    let expected = u64::try_from(theory.nodes().len() + 5).unwrap();
    for max_work in 0..expected {
        let mut workspace = EvaluationWorkspace::default();
        let attempt = workspace.evaluate(
            &candidate,
            EvaluationLimits {
                max_work,
                ..EvaluationLimits::default()
            },
            &Cancellation::default(),
        );
        assert_eq!(attempt.work, max_work);
        assert_eq!(
            attempt.result.unwrap_err(),
            EvaluationError::Stopped(Stop::WorkLimit)
        );
        let next = world(&theory, 0);
        let attempt =
            workspace.evaluate(&next, EvaluationLimits::default(), &Cancellation::default());
        let truth = attempt.result.unwrap();
        assert!((2..5).all(|node| truth.node_truth(node) == Some(false)));
    }
}

#[test]
fn compact_slot_preserves_wide_sums_and_ordinal_endpoints() {
    assert_eq!(size_of::<Measure>(), 16);
    for value in [0, i128::from(u64::MAX), i128::from(u64::MAX) + 2, i128::MAX] {
        assert_eq!(Measure::sum(value).selected_sum(), value);
    }
    let measure = crate::ExtremaMeasure {
        minimum: u64::MAX,
        maximum: u64::MAX - 1,
    };
    assert_eq!(Measure::ordinal(measure).extrema(), measure);
}

fn mixed() -> Theory {
    let mut nodes = FormulaNodes::new(
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::falsum(),
                Node::implies(0, 1),
                Node::implies(0, 2),
            ],
            vec![],
        )
        .unwrap(),
    );
    let mut transaction = nodes.transaction();
    transaction
        .push_threshold_family_range(
            &[0, 0, 1, 3, 4],
            &[2, 3, 0, 5, 7],
            &[-1, 5, 10, 18],
            100,
            100,
        )
        .unwrap();
    transaction
        .push_extremum_cut_family_range(
            ExtremumRow {
                conditions: &[0, 1, 3, 4],
                ranks: &[0, 4, 1, 3],
                rank_count: 5,
            },
            &[ExtremumCutRule::Prefix(1), ExtremumCutRule::Suffix(4)],
            100,
            100,
        )
        .unwrap();
    transaction.commit();
    Theory::new(2, nodes.into_parts(), vec![], AdmissionLimits::default()).unwrap()
}

/// Independent per-guard scalar traversal; it deliberately performs no sharing.
fn scalar(theory: &Theory, mask: usize, frozen: Option<&[bool]>) -> Vec<bool> {
    let mut values: Vec<bool> = Vec::new();
    for index in 0..theory.nodes().len() {
        let value = match theory.view().node(index).unwrap() {
            NodeView::Atom(atom) => mask & (1 << atom) != 0,
            NodeView::False => false,
            NodeView::Implies(a, b) => !values[a] || values[b],
            NodeView::And(row) => row.iter().all(|&child| values[child]),
            NodeView::Or(row) => row.iter().any(|&child| values[child]),
            NodeView::Threshold(row) => row.evaluate::<Stop>(|child| Ok(values[child])).unwrap(),
            NodeView::ExtremumCut(cut) => cut
                .rule
                .holds(cut.row.reduce::<Stop>(|child| Ok(values[child])).unwrap()),
        };
        values.push(value && frozen.is_none_or(|truth| truth[index]));
    }
    values
}

#[test]
fn mixed_rows_preserve_every_original_and_frozen_guard() {
    let theory = mixed();
    for root in 5..theory.nodes().len() {
        let owner = Theory::new(
            2,
            theory.parts().try_clone().unwrap(),
            vec![root],
            AdmissionLimits::default(),
        )
        .unwrap();
        for m in 0..4 {
            let candidate = world(&owner, m);
            let expected = scalar(&owner, m, None);
            let mut workspace = EvaluationWorkspace::default();
            let attempt = workspace.evaluate(
                &candidate,
                EvaluationLimits::default(),
                &Cancellation::default(),
            );
            assert_eq!(attempt.result.unwrap().truth(), expected);
            let frozen =
                FrozenReduct::new(&candidate, Limits::default(), &Cancellation::default()).unwrap();
            for j in 0..4 {
                assert_eq!(
                    frozen
                        .is_satisfied_by(
                            &world(&owner, j),
                            Limits::default(),
                            &Cancellation::default()
                        )
                        .unwrap(),
                    scalar(&owner, j, Some(&expected))[root]
                );
            }
        }
    }
}

#[test]
fn reused_slot_does_not_cross_domain_or_owner() {
    let weight = weighted(&[3], false);
    let mut nodes =
        FormulaNodes::new(FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap());
    let mut transaction = nodes.transaction();
    transaction
        .push_extremum_cut_family_range(
            ExtremumRow {
                conditions: &[0, 1],
                ranks: &[0, 2],
                rank_count: 3,
            },
            &[ExtremumCutRule::Prefix(1)],
            100,
            100,
        )
        .unwrap();
    transaction.commit();
    let ordinal = Theory::new(2, nodes.into_parts(), vec![], AdmissionLimits::default()).unwrap();
    let mixed = mixed();
    let mut workspace = EvaluationWorkspace::default();
    for (theory, mask) in [
        (&weight, 1),
        (&ordinal, 2),
        (&mixed, 2),
        (&weight, 0),
        (&ordinal, 1),
        (&mixed, 3),
    ] {
        let expected = scalar(theory, mask, None);
        let candidate = world(theory, mask);
        let attempt = workspace.evaluate(
            &candidate,
            EvaluationLimits::default(),
            &Cancellation::default(),
        );
        assert_eq!(attempt.result.unwrap().truth(), expected);
    }
}

#[test]
fn weighted_storage_is_admitted_before_row_initialization() {
    let owner = weighted(&[1], false);
    let candidate = world(&owner, 1);
    let requested = size_of::<EvaluationWorkspace>() as u128
        + EvaluationWorkspace::requested_payload_bytes(&owner);
    assert_eq!(
        requested,
        size_of::<EvaluationWorkspace>() as u128 + owner.nodes().len() as u128 + 16 + 8
    );
    let mut workspace = EvaluationWorkspace::default();
    let attempt = workspace.evaluate(
        &candidate,
        EvaluationLimits {
            max_bytes: usize::try_from(requested - 1).unwrap(),
            ..EvaluationLimits::default()
        },
        &Cancellation::default(),
    );
    assert_eq!(attempt.work, 0);
    assert!(
        matches!(attempt.result, Err(EvaluationError::Storage { required, .. })
        if required == requested)
    );
}

#[test]
fn tight_checks_share_weighted_measures_and_conservative_work() {
    // The plan has no readers index, so every check here evaluates completely;
    // the update route's propositions are in `tight/evaluate/update_tests.rs`
    // and `tight/evaluate/incremental_tests.rs`.
    let owner = weighted(&[1, 2, 3], true);
    let plan =
        TightPlan::compile(&owner, TightPlanLimits::default(), &Cancellation::default()).unwrap();
    let candidate = world(&owner, 0);
    let mut workspace = TightWorkspace::default();
    let allowance = u64::try_from(EvaluationWorkspace::work_bound(&owner)).unwrap() + 1;
    for expected in [allowance - 6, allowance - 7] {
        let attempt = workspace.check(
            &plan,
            &candidate,
            TightCheckLimits {
                max_work: allowance,
                ..TightCheckLimits::default()
            },
            &Cancellation::default(),
        );
        let checked = attempt.result.unwrap();
        assert_eq!(checked.verdict, TightVerdict::Stable);
        // Three guards share the three-occurrence row, saving six reads.
        assert_eq!(attempt.work, expected);
    }
}
