//! A declined attempt never publishes a checked owner, even after useful work.

use zetesis_algebra::Meter;

use zetesis_algebra::{Cancellation, Stop};

use crate::test_support::{Allowance, constructed_theory};

use super::super::super::{
    budget::{Budget, BudgetError},
    storage::Storage,
};
use super::super::build;
use super::{budget, chained};

#[test]
fn every_optional_work_prefix_returns_no_fact_owner() {
    let owner = chained();
    let cancellation = Cancellation::default();
    let mut complete_work = Meter::local(u64::MAX);
    let mut complete = budget(&mut complete_work, &cancellation);
    build(&owner, &mut Storage::default(), &mut complete).unwrap();
    for limit in 0..complete.work() {
        let mut meter_work = Meter::local(u64::MAX);
        let mut meter = Budget::new(&mut meter_work, limit, usize::MAX, &cancellation);
        assert!(
            matches!(
                build(&owner, &mut Storage::default(), &mut meter),
                Err(BudgetError::WorkDeclined)
            ),
            "limit {limit}"
        );
        assert!(meter.work() <= limit);
        assert_eq!(meter.phases().total(), meter.work());
    }
}

#[test]
fn every_global_work_prefix_retains_its_typed_stop() {
    let owner = chained();
    let cancellation = Cancellation::default();
    let mut complete_work = Meter::local(u64::MAX);
    let mut complete = budget(&mut complete_work, &cancellation);
    build(&owner, &mut Storage::default(), &mut complete).unwrap();
    for limit in 0..complete.work() {
        let mut meter_work = Meter::local(limit);
        let mut meter = Budget::new(&mut meter_work, u64::MAX, usize::MAX, &cancellation);
        assert!(
            matches!(
                build(&owner, &mut Storage::default(), &mut meter),
                Err(BudgetError::Stopped(Stop::WorkLimit))
            ),
            "limit {limit}"
        );
        assert!(meter.work() <= limit);
        assert_eq!(meter.phases().total(), meter.work());
    }
}

#[test]
fn refused_storage_includes_the_unpublished_requirement() {
    let owner = chained();
    let cancellation = Cancellation::default();
    let mut complete_work = Meter::local(u64::MAX);
    let mut complete = budget(&mut complete_work, &cancellation);
    let facts = build(&owner, &mut Storage::default(), &mut complete).unwrap();
    let limits = [
        0,
        usize::try_from(facts.retained_bytes()).unwrap(),
        usize::try_from(complete.peak_bytes() - 1).unwrap(),
    ];
    for limit in limits {
        let mut meter_work = Meter::local(u64::MAX);
        let mut meter = Budget::new(&mut meter_work, u64::MAX, limit, &cancellation);
        assert!(matches!(
            build(&owner, &mut Storage::default(), &mut meter),
            Err(BudgetError::StorageDeclined { .. })
        ));
        assert!(meter.peak_bytes() > limit as u128);
        assert_eq!(meter.phases().total(), meter.work());
    }
}

#[test]
fn cancellation_precedes_empty_work_and_storage_allowances() {
    let owner = constructed_theory(0, |_| vec![]);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut meter_work = Meter::local(0);
    let mut meter = Budget::new(&mut meter_work, 0, 0, &cancellation);
    assert!(matches!(
        build(&owner, &mut Storage::default(), &mut meter),
        Err(BudgetError::Stopped(Stop::Cancelled))
    ));
    assert_eq!(meter.work(), 0);
    assert_eq!(meter.peak_bytes(), 0);
}

#[test]
fn a_reserved_deadline_refunds_all_unspent_permits() {
    let owner = chained();
    let cancellation = Cancellation::default();
    let mut quota = Allowance {
        remaining: 79,
        failure: Stop::Deadline,
    };
    let spent;
    {
        let mut meter_work = Meter::reserved(&mut quota);
        let mut meter = Budget::new(&mut meter_work, u64::MAX, usize::MAX, &cancellation);
        assert!(matches!(
            build(&owner, &mut Storage::default(), &mut meter),
            Err(BudgetError::Refused(Stop::Deadline))
        ));
        spent = meter.work();
        meter.finish();
    }
    assert_eq!(quota.remaining + spent, 79);
}
