use super::super::Budget;
use crate::{Limits, Resource, Statistics, Stop};
use std::cell::Cell;

fn statistics(work: u64) -> Statistics {
    Statistics {
        work,
        predicates: 1,
        positions: 2,
        links: 3,
        value_entries: 4,
        rounds: 5,
        inspected_bytes: 6,
        widened: 7,
    }
}

fn budget(work: u64, limit: u64) -> Budget {
    Budget {
        limits: Limits {
            max_work: limit,
            ..Limits::default()
        },
        statistics: Cell::new(statistics(work)),
    }
}

#[test]
fn work_refusal_reports_the_complete_wide_demand() {
    let budget = budget(u64::MAX, u64::MAX);
    assert_eq!(
        budget.work(u64::MAX),
        Err(Stop {
            resource: Resource::Work,
            limit: u128::from(u64::MAX),
            observed: u128::from(u64::MAX) * 2,
        })
    );
    assert_eq!(budget.statistics(), statistics(u64::MAX));
}

#[test]
fn accepted_work_changes_only_its_complete_receipt() {
    let budget = budget(u64::MAX - 3, u64::MAX);
    assert_eq!(budget.work(3), Ok(()));
    assert_eq!(budget.statistics(), statistics(u64::MAX));
    assert_eq!(
        budget.work(1),
        Err(Stop {
            resource: Resource::Work,
            limit: u128::from(u64::MAX),
            observed: u128::from(u64::MAX) + 1,
        })
    );
    assert_eq!(budget.statistics(), statistics(u64::MAX));
}

#[test]
fn zero_work_rechecks_the_current_ceiling() {
    let below = budget(5, 4);
    assert_eq!(
        below.work(0),
        Err(Stop {
            resource: Resource::Work,
            limit: 4,
            observed: 5,
        })
    );
    assert_eq!(below.statistics(), statistics(5));
    let exact = budget(5, 5);
    assert_eq!(exact.work(0), Ok(()));
    assert_eq!(exact.statistics(), statistics(5));
}

#[test]
fn inspected_byte_refusal_retains_the_complete_wide_demand() {
    let mut budget = budget(9, 9);
    budget.limits.max_inspected_bytes = u64::MAX;
    let before = Statistics {
        inspected_bytes: u64::MAX,
        ..budget.statistics()
    };
    budget.statistics.set(before);
    assert_eq!(budget.bytes(0), Ok(()));
    assert_eq!(
        budget.bytes(u64::MAX),
        Err(Stop {
            resource: Resource::InspectedBytes,
            limit: u128::from(u64::MAX),
            observed: u128::from(u64::MAX) * 2,
        })
    );
    assert_eq!(budget.statistics(), before);
}

#[test]
fn accepted_inspected_bytes_change_only_their_receipt() {
    let mut budget = budget(9, 9);
    budget.limits.max_inspected_bytes = 10;
    let before = budget.statistics();
    assert_eq!(budget.bytes(4), Ok(()));
    let after = Statistics {
        inspected_bytes: 10,
        ..before
    };
    assert_eq!(budget.statistics(), after);
    assert_eq!(
        budget.bytes(1),
        Err(Stop {
            resource: Resource::InspectedBytes,
            limit: 10,
            observed: 11,
        })
    );
    assert_eq!(budget.statistics(), after);
}
