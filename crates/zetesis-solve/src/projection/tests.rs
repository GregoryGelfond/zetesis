use super::{ProjectionError, ProjectionLimits, ProjectionResource, ProjectionStatistics, Work};
use zetesis_algebra::{Cancellation, Stop};

fn statistics(work: u64) -> ProjectionStatistics {
    ProjectionStatistics {
        representatives: 3,
        duplicates: 7,
        work,
        retained_bytes: 11,
        peak_bytes: 13,
        complete: true,
    }
}

fn charge(
    stats: &mut ProjectionStatistics,
    amount: u64,
    limit: u64,
    cancellation: &Cancellation,
) -> Result<(), ProjectionError> {
    Work {
        limits: ProjectionLimits {
            max_work: limit,
            ..ProjectionLimits::default()
        },
        stats,
        cancellation,
    }
    .charge(amount)
}

#[test]
fn native_counter_overflow_precedes_work_refusal() {
    let cancellation = Cancellation::default();
    for limit in [0, u64::MAX] {
        for amount in [1, u64::MAX] {
            let before = statistics(u64::MAX);
            let mut stats = before;
            assert_eq!(
                charge(&mut stats, amount, limit, &cancellation),
                Err(ProjectionError::Overflow)
            );
            assert_eq!(stats, before);
        }
    }
}

#[test]
fn cancellation_precedes_zero_charge_and_overflow() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    for amount in [0, 1] {
        let before = statistics(u64::MAX);
        let mut stats = before;
        assert_eq!(
            charge(&mut stats, amount, 0, &cancellation),
            Err(ProjectionError::Control(Stop::Cancelled))
        );
        assert_eq!(stats, before);
    }
}

#[test]
fn complete_work_batches_preserve_other_receipts() {
    let cancellation = Cancellation::default();
    let before = statistics(u64::MAX - 3);
    let mut stats = before;
    assert_eq!(charge(&mut stats, 3, u64::MAX, &cancellation), Ok(()));
    assert_eq!(stats, statistics(u64::MAX));

    let mut stats = before;
    assert_eq!(
        charge(&mut stats, 3, u64::MAX - 1, &cancellation),
        Err(ProjectionError::Limit {
            resource: ProjectionResource::Work,
            observed: u128::from(u64::MAX),
            limit: u128::from(u64::MAX - 1),
        })
    );
    assert_eq!(stats, before);
}

#[test]
fn zero_work_rechecks_the_current_ceiling() {
    let cancellation = Cancellation::default();
    let before = statistics(5);
    let mut stats = before;
    assert_eq!(
        charge(&mut stats, 0, 4, &cancellation),
        Err(ProjectionError::Limit {
            resource: ProjectionResource::Work,
            observed: 5,
            limit: 4,
        })
    );
    assert_eq!(stats, before);
    assert_eq!(charge(&mut stats, 0, 5, &cancellation), Ok(()));
    assert_eq!(stats, before);
}

fn observe(
    stats: &mut ProjectionStatistics,
    retained: u128,
    temporary: u128,
    limit: usize,
    cancellation: &Cancellation,
) -> Result<(), ProjectionError> {
    Work {
        limits: ProjectionLimits {
            max_bytes: limit,
            ..ProjectionLimits::default()
        },
        stats,
        cancellation,
    }
    .observe(retained, temporary)
}

#[test]
fn live_overflow_keeps_the_actual_retained_capacity() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let before = statistics(5);
    let mut stats = before;
    assert_eq!(
        observe(&mut stats, u128::MAX, 1, 0, &cancellation),
        Err(ProjectionError::Overflow)
    );
    assert_eq!(
        stats,
        ProjectionStatistics {
            retained_bytes: u128::MAX,
            ..before
        }
    );
}

#[test]
fn refused_observation_publishes_its_capacity_receipt() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let before = statistics(5);
    let mut stats = before;
    assert_eq!(
        observe(&mut stats, 16, 9, 20, &cancellation),
        Err(ProjectionError::Limit {
            resource: ProjectionResource::Bytes,
            observed: 25,
            limit: 20,
        })
    );
    assert_eq!(
        stats,
        ProjectionStatistics {
            retained_bytes: 16,
            peak_bytes: 25,
            ..before
        }
    );
}

#[test]
fn smaller_observations_preserve_the_historical_peak() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    for (retained, limit) in [(3, 3), (0, 0)] {
        let before = statistics(5);
        let mut stats = before;
        // Observation has no control poll, including for the zero total.
        assert_eq!(
            observe(&mut stats, retained, 0, limit, &cancellation),
            Ok(())
        );
        assert_eq!(
            stats,
            ProjectionStatistics {
                retained_bytes: retained,
                ..before
            }
        );
    }
}

#[test]
fn requested_capacity_neither_polls_nor_observes_storage() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let before = statistics(5);
    let mut stats = before;
    for (required, expected) in [
        (0, Ok(())),
        (20, Ok(())),
        (
            21,
            Err(ProjectionError::Limit {
                resource: ProjectionResource::Bytes,
                observed: 21,
                limit: 20,
            }),
        ),
    ] {
        let result = Work {
            limits: ProjectionLimits {
                max_bytes: 20,
                ..ProjectionLimits::default()
            },
            stats: &mut stats,
            cancellation: &cancellation,
        }
        .admit_bytes(required);
        assert_eq!(result, expected);
        assert_eq!(stats, before);
    }
}
