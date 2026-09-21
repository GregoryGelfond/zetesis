//! Failed workspace reservation must retain its measured prefix and first cause.

use super::*;
use crate::{Literal, PreparedReduct, ReductPreparationLimits, SearchStatistics};
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
    let prepared = PreparedReduct::prepare(
        &original,
        ReductPreparationLimits::default(),
        &Cancellation::default(),
    )
    .result
    .unwrap();
    let required = prepared.scratch_requirements(3).unwrap();
    let larger = larger_query();
    for failure_at in 0..3 {
        let ceiling = required.shared_bytes + required.result_bytes + 3 * required.query_bytes;
        let mut executor =
            CompletionExecutor::with_scratch_limit(NonZeroUsize::new(3).unwrap(), ceiling).unwrap();
        let cancellation = Cancellation::default();
        let mut budget = Budget {
            quota: LocalQuota,
            cancellation: &cancellation,
            limits: limits.search,
            statistics: SearchStatistics::default(),
        };
        let mut statistics = Statistics::default();
        let mut accepted = Vec::new();
        let mut state = crate::prepared_reduct::State::new(crate::SearchMethod::Clauses);
        state
            .ensure(&original, limits, &mut budget, &mut statistics)
            .unwrap();
        let before_budget = budget.statistics;
        let before_statistics = statistics;
        let mut calls = 0_usize;
        let mut dynamic_bytes = 0;
        let result = executor.complete_with(
            Input {
                theory: &original,
                candidates: &candidates,
                verdicts: &verdicts,
                limits,
                prepared: None,
                query: None,
            },
            &mut budget,
            &mut statistics,
            &mut accepted,
            &mut state,
            |workspace, owner, limit, cancellation| {
                let result = if calls == failure_at {
                    workspace.reserve(&larger, limit, cancellation).unwrap();
                    let mut refused = Vec::<Literal>::new();
                    refused
                        .try_reserve_exact(usize::MAX)
                        .map_err(|_| Incomplete::Allocation)
                } else {
                    workspace.reserve(owner, limit, cancellation)
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
        let minimum_peak = u128::from(required.shared_bytes)
            + u128::from(required.result_bytes)
            + 3 * std::mem::size_of::<ReductWorkspace>() as u128
            + dynamic_bytes
            + calls as u128 * scratch::transient(&prepared);
        assert!(u128::from(progress.peak_scratch_bytes) >= minimum_peak);
        // This must preserve Allocation even when the observed prefix is above
        // the ceiling; no CompletionScratch error may replace the first cause.
        assert!(progress.peak_scratch_bytes > ceiling);
        assert_eq!(progress.requested_scratch_bytes, ceiling);
        assert_eq!(
            (progress.candidates, progress.completed, progress.failed),
            (0, 0, 0)
        );
        assert_eq!(budget.statistics, before_budget);
        assert_eq!(statistics, before_statistics);
        assert!(accepted.is_empty());
    }
}

/// A real finite query whose retained worker arrays exceed the one-atom fixture.
/// The reservation callback grows to this shape before its deterministic Vec
/// capacity-overflow request, separating an actual allocated prefix from refusal.
fn larger_query() -> PreparedReduct {
    let theory = Theory::new(
        8,
        vec![Node::False; 256],
        vec![],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    PreparedReduct::prepare(
        &theory,
        ReductPreparationLimits::default(),
        &Cancellation::default(),
    )
    .result
    .unwrap()
}
