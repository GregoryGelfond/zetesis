//! Exact work, decision and reservation boundaries are independent of quota
//! ownership.

use crate::budget::{Budget, LocalQuota, Quota, SharedBudget};
use crate::{Cancellation, Incomplete, SearchLimits, SearchStatistics};

/// One budget operation of a scripted search: a tick, a decision, a batched
/// grant spending part of what it is granted, or a whole-operation
/// reservation reporting part of its allowance as used.
#[derive(Clone, Copy, Debug)]
enum Operation {
    Tick,
    Decide,
    Grant { wanted: u64, spent: u64 },
    Reservation { wanted: u64, used: u64 },
}

const SCRIPT: [Operation; 10] = [
    Operation::Tick,
    Operation::Decide,
    Operation::Grant {
        wanted: 3,
        spent: 3,
    },
    Operation::Grant {
        wanted: 4,
        spent: 2,
    },
    Operation::Decide,
    Operation::Tick,
    Operation::Reservation { wanted: 3, used: 2 },
    Operation::Tick,
    Operation::Decide,
    Operation::Grant {
        wanted: 2,
        spent: 2,
    },
];

/// Run `script` until its first refusal, as a search stops at one, and return
/// each step's outcome.
fn run(budget: &mut Budget<'_, impl Quota>, script: &[Operation]) -> Vec<Result<(), Incomplete>> {
    let mut outcomes = Vec::new();
    for operation in script {
        let outcome = match *operation {
            Operation::Tick => budget.tick(),
            Operation::Decide => budget.decide(),
            Operation::Grant { wanted, spent } => budget.reserve_up_to(wanted).map(|granted| {
                budget.refund(granted - spent.min(granted));
            }),
            Operation::Reservation { wanted, used } => {
                budget.with_reservation(wanted, |granted| ((), used.min(granted)))
            }
        };
        let refused = outcome.is_err();
        outcomes.push(outcome);
        if refused {
            break;
        }
    }
    outcomes
}

/// Run `script` on a local budget and on a single lease of a shared budget,
/// both seeded with `spent`, and check that they agree step by step and in
/// total. Returns the local budget's statistics.
fn compare(
    script: &[Operation],
    limits: SearchLimits,
    spent: SearchStatistics,
) -> SearchStatistics {
    let cancellation = Cancellation::default();
    let mut local = Budget {
        quota: LocalQuota,
        limits,
        cancellation: &cancellation,
        statistics: spent,
    };
    let expected = run(&mut local, script);
    let shared = SharedBudget::new(limits, spent);
    let mut worker = Budget {
        quota: shared.lease(&cancellation),
        limits,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    assert_eq!(run(&mut worker, script), expected);
    assert_eq!(worker.statistics.work, local.statistics.work - spent.work);
    assert_eq!(
        worker.statistics.decisions,
        local.statistics.decisions - spent.decisions
    );
    let mut joined = worker.statistics;
    drop(worker.quota);
    shared.record(&mut joined);
    assert_eq!(joined, local.statistics);
    local.statistics
}

#[test]
fn local_and_single_job_shared_budgets_agree_at_every_small_work_and_decision_ceiling() {
    let spent = SearchStatistics {
        work: 7,
        decisions: 3,
    };
    let mut checked = 0;
    for rotation in 0..SCRIPT.len() {
        let mut script = SCRIPT;
        script.rotate_left(rotation);
        let complete = compare(&script, SearchLimits::default(), spent);
        for max_work in spent.work..=complete.work + 1 {
            for max_decisions in spent.decisions..=complete.decisions + 1 {
                let stats = compare(
                    &script,
                    SearchLimits {
                        max_work,
                        max_decisions,
                    },
                    spent,
                );
                assert!(stats.work <= max_work);
                assert!(stats.decisions <= max_decisions);
            }
        }
        checked += 1;
    }
    assert_eq!(checked, SCRIPT.len());
}

#[test]
fn failed_decision_reserves_work_first_and_cancellation_precedes_either_quota() {
    let cancellation = Cancellation::default();
    let limits = SearchLimits {
        max_work: 2,
        max_decisions: 0,
    };
    let mut local = Budget {
        quota: LocalQuota,
        limits,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let shared = SharedBudget::new(limits, SearchStatistics::default());
    let mut worker = Budget {
        quota: shared.lease(&cancellation),
        limits,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    for remaining in [false, true] {
        assert_eq!(local.decide(), Err(Incomplete::DecisionLimit));
        assert_eq!(worker.decide(), Err(Incomplete::DecisionLimit));
        assert_eq!(local.statistics, worker.statistics);
        assert_eq!(local.statistics.work, u64::from(remaining) + 1);
        assert_eq!(local.statistics.decisions, 0);
    }
    assert_eq!(local.decide(), Err(Incomplete::WorkLimit));
    assert_eq!(worker.decide(), Err(Incomplete::WorkLimit));
    cancellation.cancel();
    assert_eq!(local.tick(), Err(Incomplete::Cancelled));
    assert_eq!(worker.tick(), Err(Incomplete::Cancelled));
    assert_eq!(local.decide(), Err(Incomplete::Cancelled));
    assert_eq!(worker.decide(), Err(Incomplete::Cancelled));
    let mut joined = SearchStatistics::default();
    drop(worker.quota);
    shared.record(&mut joined);
    assert_eq!(joined, local.statistics);
    assert_eq!(joined, worker.statistics);
    assert_eq!(joined.work, 2);
    assert_eq!(joined.decisions, 0);
}

#[test]
fn cancellation_precedes_an_expired_deadline_and_exhausted_quotas() {
    let cancellation = Cancellation::with_deadline(std::time::Instant::now()).unwrap();
    let limits = SearchLimits {
        max_work: 0,
        max_decisions: 0,
    };
    let mut local = Budget {
        quota: LocalQuota,
        limits,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let shared = SharedBudget::new(limits, SearchStatistics::default());
    let mut worker = Budget {
        quota: shared.lease(&cancellation),
        limits,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    assert_eq!(local.tick(), Err(Incomplete::Deadline));
    assert_eq!(worker.tick(), Err(Incomplete::Deadline));
    cancellation.clone().cancel();
    assert_eq!(local.decide(), Err(Incomplete::Cancelled));
    assert_eq!(worker.decide(), Err(Incomplete::Cancelled));
    assert_eq!(local.statistics, SearchStatistics::default());
    assert_eq!(worker.statistics, local.statistics);
    drop(worker.quota);
    let mut joined = SearchStatistics::default();
    shared.record(&mut joined);
    assert_eq!(joined, SearchStatistics::default());
}

#[test]
fn the_last_representable_local_or_shared_charge_never_wraps() {
    let cancellation = Cancellation::default();
    let limits = SearchLimits {
        max_work: u64::MAX,
        max_decisions: u64::MAX,
    };
    let spent = SearchStatistics {
        work: u64::MAX - 1,
        decisions: u64::MAX - 1,
    };
    let shared = SharedBudget::new(limits, spent);
    let mut local = Budget {
        quota: LocalQuota,
        limits,
        cancellation: &cancellation,
        statistics: spent,
    };
    let mut worker = Budget {
        quota: shared.lease(&cancellation),
        limits,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    assert_eq!(local.decide(), Ok(()));
    assert_eq!(worker.decide(), Ok(()));
    assert_eq!(worker.statistics.work, 1);
    assert_eq!(worker.statistics.decisions, 1);
    assert_eq!(local.decide(), Err(Incomplete::WorkLimit));
    assert_eq!(worker.decide(), Err(Incomplete::WorkLimit));
    let mut joined = worker.statistics;
    drop(worker.quota);
    shared.record(&mut joined);
    assert_eq!(joined, local.statistics);
    assert_eq!(joined.work, u64::MAX);
    assert_eq!(joined.decisions, u64::MAX);
}

#[test]
fn local_batched_grants_stop_at_the_ceiling() {
    use super::Quota as _;
    let quota = LocalQuota;
    assert_eq!(quota.reserve_up_to(0, 10, 256), Ok(10));
    assert_eq!(quota.reserve_up_to(7, 10, 2), Ok(2));
    assert_eq!(quota.reserve_up_to(10, 10, 1), Err(Incomplete::WorkLimit));
}
