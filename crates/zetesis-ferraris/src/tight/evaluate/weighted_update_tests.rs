//! Exact incremental sums, including multiplicity, wide weights and interruption.

use super::TightWorkspace;
use crate::test_support::{constructed_theory, world};
use crate::{
    AdmissionLimits, FormulaParts, Node, NumericDomain, NumericRow, Theory, ThresholdDescriptor,
    TightCheckLimits, TightError, TightPlan, TightPlanLimits, TightResource,
};
use zetesis_cpu::Cancellation;

fn prepare(theory: &Theory) -> TightPlan {
    let cancellation = Cancellation::default();
    let mut plan = TightPlan::compile(theory, TightPlanLimits::default(), &cancellation).unwrap();
    assert_eq!(
        plan.prepare_updates(TightPlanLimits::default(), &cancellation)
            .result,
        Ok(true)
    );
    plan
}

/// Shared guards, repeated conditions, a zero weight and a sum beyond u64.
/// The raw rows overlap, their order differs from guard order, and one is unused.
fn overlapping() -> Theory {
    let parts = FormulaParts::from_numeric_raw(
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::threshold(0),
            Node::threshold(1),
            Node::threshold(2),
        ],
        vec![],
        vec![0, 0, 1, 2],
        vec![2, u64::MAX, 0, 3],
        vec![
            NumericRow {
                start: 4,
                length: 0,
                domain: NumericDomain::Weights,
            },
            NumericRow {
                start: 1,
                length: 3,
                domain: NumericDomain::Weights,
            },
            NumericRow {
                start: 0,
                length: 2,
                domain: NumericDomain::Weights,
            },
        ],
        vec![
            ThresholdDescriptor {
                row: 2,
                bound: i128::from(u64::MAX) + 2,
            },
            ThresholdDescriptor {
                row: 1,
                bound: i128::from(u64::MAX) + 1,
            },
            ThresholdDescriptor { row: 2, bound: 2 },
        ],
        vec![],
    )
    .unwrap();
    Theory::new(3, parts, vec![], AdmissionLimits::default()).unwrap()
}

#[test]
fn every_weighted_transition_matches_complete_truth() {
    let theory = overlapping();
    let plan = prepare(&theory);
    let cancellation = Cancellation::default();
    for before in 0..8 {
        for after in 0..8 {
            let mut retained = TightWorkspace::default();
            retained
                .check(
                    &plan,
                    &world(&theory, before),
                    TightCheckLimits::default(),
                    &cancellation,
                )
                .result
                .unwrap();
            let candidate = world(&theory, after);
            let updated = retained
                .check(
                    &plan,
                    &candidate,
                    TightCheckLimits::default(),
                    &cancellation,
                )
                .result
                .unwrap();
            let mut complete = TightWorkspace::default();
            let reference = complete
                .check(
                    &plan,
                    &candidate,
                    TightCheckLimits::default(),
                    &cancellation,
                )
                .result
                .unwrap();
            assert_eq!(updated.verdict, reference.verdict, "{before} -> {after}");
            assert_eq!(retained.values, complete.values, "{before} -> {after}");
            assert!(u128::from(updated.work) <= plan.check_work_bound());
        }
    }
}

fn changed_check(
    plan: &TightPlan,
    max_work: u64,
) -> (TightWorkspace, crate::TightAttempt<crate::TightCheck>) {
    let mut workspace = TightWorkspace::default();
    let cancellation = Cancellation::default();
    workspace
        .check(
            plan,
            &world(&plan.theory, 1),
            TightCheckLimits::default(),
            &cancellation,
        )
        .result
        .unwrap();
    let attempt = workspace.check(
        plan,
        &world(&plan.theory, 6),
        TightCheckLimits {
            max_work,
            ..TightCheckLimits::default()
        },
        &cancellation,
    );
    (workspace, attempt)
}

#[test]
fn every_interrupted_sum_update_can_retry() {
    let theory = overlapping();
    let plan = prepare(&theory);
    let (_, completed) = changed_check(&plan, u64::MAX);
    completed.result.unwrap();
    for max_work in 0..completed.work {
        let (mut workspace, stopped) = changed_check(&plan, max_work);
        assert_eq!(stopped.work, max_work);
        assert_eq!(
            stopped.result.unwrap_err(),
            TightError::Limit(TightResource::Work)
        );
        assert!(workspace.retained.theory.is_none());
        let candidate = world(&theory, 5);
        let cancellation = Cancellation::default();
        let recovered = workspace
            .check(
                &plan,
                &candidate,
                TightCheckLimits::default(),
                &cancellation,
            )
            .result
            .unwrap();
        let mut complete = TightWorkspace::default();
        let reference = complete
            .check(
                &plan,
                &candidate,
                TightCheckLimits::default(),
                &cancellation,
            )
            .result
            .unwrap();
        assert_eq!(recovered.verdict, reference.verdict);
        assert_eq!(workspace.values, complete.values);
    }
}

/// Only condition 0 changes; repetitions of condition 2 stay fixed. Two guards
/// share the row, and another weighted row consumes their changed truth.
fn padded(unchanged: usize) -> Theory {
    constructed_theory(3, |append| {
        let mut conditions = vec![0, 0, 1];
        conditions.extend(std::iter::repeat_n(2, unchanged));
        let mut weights = vec![2, u64::MAX, 0];
        weights.extend(std::iter::repeat_n(1, unchanged));
        let guards = append
            .push_threshold_family_range(
                &conditions,
                &weights,
                &[1, i128::from(u64::MAX)],
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        append
            .push_threshold_family_range(
                &[guards.start, guards.start + 1],
                &[1, 1],
                &[2],
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        vec![]
    })
}

#[test]
fn unchanged_contributions_do_not_add_update_work() {
    let mut work = Vec::new();
    for padding in [4, 256] {
        let theory = padded(padding);
        let plan = prepare(&theory);
        let cancellation = Cancellation::default();
        let mut workspace = TightWorkspace::default();
        workspace
            .check(
                &plan,
                &world(&theory, 0),
                TightCheckLimits::default(),
                &cancellation,
            )
            .result
            .unwrap();
        for mask in [1, 0] {
            let candidate = world(&theory, mask);
            let checked = workspace
                .check(
                    &plan,
                    &candidate,
                    TightCheckLimits::default(),
                    &cancellation,
                )
                .result
                .unwrap();
            assert_eq!(workspace.values[3..], [u8::from(mask != 0); 3]);
            work.push(checked.work);
        }
    }
    assert_eq!(
        &work[..2],
        &work[2..],
        "unchanged row tails must not be rescanned"
    );
}
