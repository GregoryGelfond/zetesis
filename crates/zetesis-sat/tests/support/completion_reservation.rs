//! Failed workspace reservation must retain its measured prefix and first cause.

use super::*;
use crate::{AdmissionLimits, SearchStatistics};
use zetesis_ferraris::Node;

#[test]
fn unrepresentable_progress_preserves_an_earlier_reservation_error() {
    for reservation in [Ok(()), Err(Incomplete::Allocation)] {
        let mut progress = CompletionStatistics::default();
        assert_eq!(
            record_reservation(&mut progress, u128::from(u64::MAX), reservation),
            reservation,
        );
        assert_eq!(progress.peak_scratch_bytes, u64::MAX);
        assert_eq!(
            record_reservation(&mut progress, u128::from(u64::MAX) + 1, reservation),
            Err(reservation.err().unwrap_or(Incomplete::CounterOverflow)),
        );
        assert_eq!(progress.peak_scratch_bytes, u64::MAX);
    }
}

#[test]
fn partial_reservation_records_capacity_and_preserves_allocation_failure() {
    let original = Theory::new(
        1,
        vec![Node::Atom(0)],
        vec![0],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let candidates = vec![Interpretation::new(&original, [0]).unwrap(); 3];
    let verdicts = vec![BatchVerdict::Residual; 3];
    let limits = Limits::default();
    let required = CompletionExecutor::scratch_requirements(&original, limits, 3).unwrap();
    // Each reservation first allocates finite clause/mask/node vectors. The
    // final non-ZST strict-subset vector necessarily exceeds isize::MAX bytes.
    let failing = Theory::new(
        usize::MAX / 2,
        vec![Node::False; 256],
        vec![],
        zetesis_ferraris::AdmissionLimits {
            max_atoms: usize::MAX,
            ..Default::default()
        },
    )
    .unwrap();
    for failure_at in 0..3 {
        let ceiling = required.result_bytes + 3 * required.query_bytes;
        let mut executor =
            CompletionExecutor::with_scratch_limit(NonZeroUsize::new(3).unwrap(), ceiling).unwrap();
        let control = Control::default();
        let mut budget = Budget {
            quota: LocalQuota,
            control: &control,
            limits: limits.search,
            statistics: SearchStatistics::default(),
        };
        let mut statistics = Statistics::default();
        let mut accepted = Vec::new();
        let mut calls = 0_usize;
        let mut dynamic_bytes = 0;
        let result = executor.complete_with(
            Input {
                theory: &original,
                candidates: &candidates,
                verdicts: &verdicts,
                limits,
            },
            &mut budget,
            &mut statistics,
            &mut accepted,
            |workspace, theory, admission| {
                let result = if calls == failure_at {
                    workspace.reserve(
                        &failing,
                        AdmissionLimits {
                            max_variables: usize::MAX,
                            max_clauses: 4,
                            max_literals: 8,
                        },
                    )
                } else {
                    workspace.reserve(theory, admission)
                };
                calls += 1;
                let bytes = workspace.retained_bytes() - std::mem::size_of_val(workspace) as u128;
                assert!(bytes > 0);
                dynamic_bytes += bytes;
                result
            },
        );
        assert_eq!(result, Err(Incomplete::Allocation));
        assert_eq!(calls, failure_at + 1);
        let progress = executor.last_statistics().unwrap();
        let minimum_peak = u128::from(required.result_bytes)
            + 3 * std::mem::size_of::<reduct_query::Workspace>() as u128
            + dynamic_bytes
            + calls as u128 * scratch::transient(&original, limits).unwrap();
        assert!(u128::from(progress.peak_scratch_bytes) >= minimum_peak);
        // This must preserve Allocation even when the observed prefix is above
        // the ceiling; no CompletionScratch error may replace the first cause.
        assert!(progress.peak_scratch_bytes > ceiling);
        assert_eq!(progress.requested_scratch_bytes, ceiling);
        assert_eq!(
            (progress.candidates, progress.completed, progress.failed),
            (0, 0, 0)
        );
        assert_eq!(budget.statistics, SearchStatistics::default());
        assert_eq!(statistics, Statistics::default());
        assert!(accepted.is_empty());
    }
}
