//! Local allowances and cumulative evidence have independent failure boundaries.

use super::{Account, Failure};
use crate::{ModelConstructionStop, SolveConfig, SolveError};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_test_support::programs::model;

#[test]
fn selection_allowance_is_fresh_and_inclusive() {
    let owner = model(&["a"]);
    let mut account = Account::default();
    let cancellation = Cancellation::default();
    let order = account
        .prepare(owner.catalog(), &SolveConfig::default(), &cancellation)
        .unwrap_or_else(|_| panic!("the small order must prepare"));
    let prepared = account.statistics();
    let first = account
        .select(&order, [0], &SolveConfig::default(), &cancellation)
        .unwrap_or_else(|_| panic!("the first selection must complete"));
    let required = account.statistics().work - prepared.work;
    let exact = SolveConfig {
        max_model_work: required,
        ..SolveConfig::default()
    };
    let second = account
        .select(&order, [0], &exact, &cancellation)
        .unwrap_or_else(|_| panic!("the exact fresh allowance must admit selection"));
    assert_eq!(first, owner);
    assert_eq!(second, owner);
    assert_eq!(account.statistics().work, prepared.work + 2 * required);
    assert_eq!(account.statistics().constructed, 2);

    let below = SolveConfig {
        max_model_work: required - 1,
        ..exact
    };
    assert!(matches!(
        account.select(&order, [0], &below, &cancellation),
        Err(Failure::Interrupted(ModelConstructionStop::Work { observed, limit }))
            if observed == u128::from(required) && limit == required - 1
    ));
    let receipt = account.statistics();
    assert_eq!(receipt.work, prepared.work + 3 * required - 1);
    assert_eq!(receipt.constructed, 2);
    assert_eq!(receipt.prepared_bytes, prepared.prepared_bytes);
    assert!(receipt.peak_bytes >= prepared.peak_bytes);
    assert_eq!(first, owner);
    assert_eq!(second, owner);
}

#[test]
fn cumulative_work_overflow_prevents_publication() {
    let owner = model(&["a"]);
    let mut account = Account::default();
    let cancellation = Cancellation::default();
    let order = account
        .prepare(owner.catalog(), &SolveConfig::default(), &cancellation)
        .unwrap_or_else(|_| panic!("the small order must prepare"));
    let prepared_bytes = account.statistics().prepared_bytes;
    // The first permit is accepted; the next cannot enter the total. This
    // exercises propagation through ModelOrder's actual failure receipt.
    account.statistics.work = u64::MAX - 1;
    assert!(matches!(
        account.select(&order, [0], &SolveConfig::default(), &cancellation),
        Err(Failure::Run(SolveError::ModelStatisticsOverflow))
    ));
    let receipt = account.statistics();
    assert_eq!(receipt.work, u64::MAX);
    assert_eq!(receipt.constructed, 0);
    assert_eq!(receipt.prepared_bytes, prepared_bytes);
    assert!(receipt.peak_bytes > prepared_bytes);
}

#[test]
fn local_refusal_precedes_cumulative_overflow() {
    let owner = model(&["a"]);
    let mut account = Account::default();
    let cancellation = Cancellation::default();
    let order = account
        .prepare(owner.catalog(), &SolveConfig::default(), &cancellation)
        .unwrap_or_else(|_| panic!("the small order must prepare"));
    account.statistics.work = u64::MAX;
    let config = SolveConfig {
        max_model_work: 0,
        ..SolveConfig::default()
    };
    assert!(matches!(
        account.select(&order, [0], &config, &cancellation),
        Err(Failure::Interrupted(ModelConstructionStop::Work {
            observed: 1,
            limit: 0
        }))
    ));
    assert_eq!(account.statistics().work, u64::MAX);
    assert_eq!(account.statistics().constructed, 0);
}

#[test]
fn cancellation_precedes_both_work_boundaries() {
    let owner = model(&["a"]);
    let mut account = Account::default();
    let cancellation = Cancellation::default();
    let order = account
        .prepare(owner.catalog(), &SolveConfig::default(), &cancellation)
        .unwrap_or_else(|_| panic!("the small order must prepare"));
    account.statistics.work = u64::MAX;
    let config = SolveConfig {
        max_model_work: 0,
        ..SolveConfig::default()
    };
    cancellation.cancel();
    assert!(matches!(
        account.select(&order, [0], &config, &cancellation),
        Err(Failure::Interrupted(ModelConstructionStop::Control(
            Stop::Cancelled
        )))
    ));
    assert_eq!(account.statistics().work, u64::MAX);
    assert_eq!(account.statistics().constructed, 0);
}
