//! Actual capacity observations retain the original reservation-failure priority.

use super::{ErrorKind, Resource, Statistics, Stop, observe_capacity};

#[test]
fn failed_reservation_retains_representable_capacity() {
    let mut statistics = Statistics {
        work: 9,
        storage_peak_bytes: 2,
        ..Statistics::default()
    };
    assert_eq!(
        observe_capacity(
            13,
            Err(ErrorKind::Stopped(Stop::Allocation)),
            10,
            &mut statistics
        ),
        Err(ErrorKind::Stopped(Stop::Allocation)),
    );
    assert_eq!(
        statistics,
        Statistics {
            work: 9,
            storage_peak_bytes: 13,
            ..Statistics::default()
        }
    );
    let before = statistics;
    assert_eq!(
        observe_capacity(
            u128::from(u64::MAX) + 1,
            Err(ErrorKind::Stopped(Stop::Allocation)),
            0,
            &mut statistics
        ),
        Err(ErrorKind::Stopped(Stop::Allocation)),
    );
    assert_eq!(statistics, before);
    assert_eq!(
        observe_capacity(u128::from(u64::MAX) + 1, Ok(()), 0, &mut statistics),
        Err(ErrorKind::Overflow),
    );
    assert_eq!(statistics, before);
}

#[test]
fn capacity_refusal_uses_the_observation_not_historical_peak() {
    let mut statistics = Statistics {
        work: 9,
        storage_peak_bytes: 20,
        ..Statistics::default()
    };
    let before = statistics;
    assert_eq!(observe_capacity(5, Ok(()), 5, &mut statistics), Ok(()));
    assert_eq!(statistics, before);
    assert_eq!(
        observe_capacity(6, Ok(()), 5, &mut statistics),
        Err(ErrorKind::Limit(Resource::Bytes)),
    );
    assert_eq!(statistics, before);
    assert_eq!(
        observe_capacity(21, Ok(()), 5, &mut statistics),
        Err(ErrorKind::Limit(Resource::Bytes)),
    );
    assert_eq!(statistics.storage_peak_bytes, 21);
    assert_eq!(statistics.work, 9);
}
