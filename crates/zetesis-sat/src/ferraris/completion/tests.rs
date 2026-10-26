//! Completion's measured scratch peak and its merge of worker receipts.

use super::*;

#[test]
fn an_unrepresentable_scratch_peak_saturates_and_is_refused() {
    let mut progress = CompletionStatistics::default();
    assert_eq!(record_peak(&mut progress, u128::from(u64::MAX)), Ok(()));
    assert_eq!(progress.peak_scratch_bytes, u64::MAX);
    assert_eq!(
        record_peak(&mut progress, u128::from(u64::MAX) + 1),
        Err(Incomplete::CounterOverflow),
    );
    assert_eq!(progress.peak_scratch_bytes, u64::MAX);
}

#[test]
fn completion_merges_frozen_positive_receipts_without_losing_failed_work() {
    // One completed attempt and one failed prefix both belong to this receipt.
    let worker = Statistics {
        reduct: crate::ReductStatistics {
            frozen_positive_attempts: 2,
            frozen_positive_completed: 1,
            frozen_positive_preparation_work: 3,
            frozen_positive_work: 11,
            ..crate::ReductStatistics::default()
        },
        ..Statistics::default()
    };
    let mut statistics = worker;
    merge(&mut statistics, &worker).unwrap();
    assert_eq!(statistics.reduct.frozen_positive_attempts, 4);
    assert_eq!(statistics.reduct.frozen_positive_completed, 2);
    assert_eq!(statistics.reduct.frozen_positive_preparation_work, 6);
    assert_eq!(statistics.reduct.frozen_positive_work, 22);
}

#[test]
fn frozen_positive_counter_overflow_refuses_completion_merge() {
    for field in 0..4 {
        let mut statistics = Statistics::default();
        let counter = match field {
            0 => &mut statistics.reduct.frozen_positive_attempts,
            1 => &mut statistics.reduct.frozen_positive_completed,
            2 => &mut statistics.reduct.frozen_positive_preparation_work,
            _ => &mut statistics.reduct.frozen_positive_work,
        };
        *counter = u64::MAX;
        let worker = Statistics {
            reduct: crate::ReductStatistics {
                frozen_positive_attempts: 1,
                frozen_positive_completed: 1,
                frozen_positive_preparation_work: 1,
                frozen_positive_work: 1,
                ..crate::ReductStatistics::default()
            },
            ..Statistics::default()
        };
        assert_eq!(
            merge(&mut statistics, &worker),
            Err(Incomplete::CounterOverflow)
        );
    }
}
