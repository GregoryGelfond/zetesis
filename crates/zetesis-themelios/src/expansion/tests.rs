//! Scalar receipt publication is independent of budget enforcement history.

use super::{Budget, ExpansionFailure, ExpansionLimits, ExpansionResource};
use crate::test_support::location;
use crate::{ConstraintAllowance, ConstraintCheckLimits};
use zetesis_algebra::{Cancellation, Stop};

fn budget(allowance: &ConstraintAllowance) -> Budget {
    Budget::new(
        ExpansionLimits {
            max_scalar_bytes: 7,
            ..Default::default()
        },
        0,
    )
    .with_allowance(allowance.clone())
}

#[test]
fn a_scalar_refusal_publishes_its_accepted_prefix() {
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits::default());
    let mut budget = budget(&allowance);
    let result = budget.with_settled_receipt(|budget| {
        budget.charge(ExpansionResource::ScalarBytes, 7, location())?;
        assert_eq!(allowance.statistics().scalar_bytes, 0);
        budget.charge(ExpansionResource::ScalarBytes, 1, location())
    });
    assert!(matches!(
        result,
        Err(ExpansionFailure::Limit {
            resource: ExpansionResource::ScalarBytes,
            limit: 7,
            observed: 8,
            ..
        })
    ));
    assert_eq!(budget.usage().scalar_bytes, 7);
    assert_eq!(allowance.statistics().scalar_bytes, 7);
    drop(budget);
    assert_eq!(allowance.statistics().scalar_bytes, 7);
}

#[test]
fn scalar_interruption_publishes_its_accepted_prefix() {
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits::default());
    let cancellation = Cancellation::default();
    let mut budget = budget(&allowance).with_cancellation(Some(cancellation.clone()));
    let result = budget.with_settled_receipt(|budget| {
        budget.charge(ExpansionResource::ScalarBytes, 3, location())?;
        cancellation.cancel();
        budget.charge(ExpansionResource::ScalarBytes, 0, location())
    });
    assert!(matches!(
        result,
        Err(ExpansionFailure::Interrupted {
            reason: Stop::Cancelled,
            ..
        })
    ));
    assert_eq!(budget.usage().scalar_bytes, 3);
    assert_eq!(allowance.statistics().scalar_bytes, 3);
}

#[test]
fn scalar_unwind_publishes_while_the_budget_lives() {
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits::default());
    let mut budget = budget(&allowance);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        budget.with_settled_receipt(|budget| {
            budget
                .charge(ExpansionResource::ScalarBytes, 3, location())
                .unwrap();
            panic!("controlled unwind after scalar admission");
        });
    }));
    assert!(result.is_err());
    assert_eq!(budget.usage().scalar_bytes, 3);
    assert_eq!(allowance.statistics().scalar_bytes, 3);
    budget.with_settled_receipt(|budget| {
        budget
            .charge(ExpansionResource::ScalarBytes, 2, location())
            .unwrap();
    });
    assert_eq!(allowance.statistics().scalar_bytes, 5);
    drop(budget);
    assert_eq!(allowance.statistics().scalar_bytes, 5);
}

#[test]
fn budget_clones_publish_only_new_charges() {
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits::default());
    let mut original = budget(&allowance);
    original
        .charge(ExpansionResource::ScalarBytes, 3, location())
        .unwrap();
    let mut tentative = original.clone();
    tentative
        .charge(ExpansionResource::ScalarBytes, 2, location())
        .unwrap();
    assert_eq!(tentative.usage().scalar_bytes, 5);
    drop(tentative);
    assert_eq!(allowance.statistics().scalar_bytes, 2);
    drop(original);
    assert_eq!(allowance.statistics().scalar_bytes, 5);
}

#[test]
fn replacing_a_budget_preserves_each_accepted_charge() {
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits::default());
    let mut original = budget(&allowance);
    original
        .charge(ExpansionResource::ScalarBytes, 3, location())
        .unwrap();
    let mut tentative = original.clone();
    tentative
        .charge(ExpansionResource::ScalarBytes, 2, location())
        .unwrap();
    original = tentative;
    assert_eq!(allowance.statistics().scalar_bytes, 3);
    assert_eq!(original.usage().scalar_bytes, 5);
    drop(original);
    assert_eq!(allowance.statistics().scalar_bytes, 5);
}

#[test]
fn allocation_refusal_keeps_its_site_and_cause() {
    use std::error::Error;
    let error = Vec::<u8>::new().try_reserve(usize::MAX).unwrap_err();
    let failure = ExpansionFailure::Allocation {
        error,
        location: location(),
    };
    assert_eq!(failure.site(), Some(location()));
    assert!(failure.source().is_some());
    assert!(failure.to_string().contains("source expansion storage"));
}

#[test]
fn term_work_admits_the_inclusive_ceiling_without_other_charges() {
    let mut budget = Budget::new(
        ExpansionLimits {
            max_term_work: 7,
            ..Default::default()
        },
        0,
    );
    budget
        .charge(ExpansionResource::TermWork, 3, location())
        .unwrap();
    budget
        .charge(ExpansionResource::TermWork, 4, location())
        .unwrap();
    let expected = super::ExpansionUsage {
        term_work: 7,
        ..Default::default()
    };
    assert_eq!(budget.usage(), expected);
    assert_eq!(budget.remaining_term_work(), 0);
    let result = budget.charge(ExpansionResource::TermWork, 1, location());
    assert!(matches!(
        result,
        Err(ExpansionFailure::Limit {
            resource: ExpansionResource::TermWork,
            limit: 7,
            observed: 8,
            location: site,
        }) if site == location()
    ));
    assert_eq!(budget.usage(), expected);
    budget
        .charge(ExpansionResource::TermWork, 0, location())
        .unwrap();
    assert_eq!(budget.usage(), expected);
}

#[test]
fn term_work_keeps_the_saturated_attempt_and_accepted_prefix() {
    for initial in [0, 1, usize::MAX as u128] {
        let mut budget = Budget::new(
            ExpansionLimits {
                max_term_work: usize::MAX,
                ..Default::default()
            },
            0,
        );
        budget
            .charge(ExpansionResource::TermWork, initial, location())
            .unwrap();
        let before = budget.usage();
        let result = budget.charge(ExpansionResource::TermWork, u128::MAX, location());
        assert!(matches!(
            result,
            Err(ExpansionFailure::Limit {
                resource: ExpansionResource::TermWork,
                observed: u128::MAX,
                limit,
                location: site,
            }) if limit == usize::MAX as u128 && site == location()
        ));
        assert_eq!(budget.usage(), before);
    }
}

#[test]
fn term_work_observes_control_before_zero_or_wide_refusal() {
    let cancellation = Cancellation::default();
    let mut budget =
        Budget::new(ExpansionLimits::default(), 0).with_cancellation(Some(cancellation.clone()));
    budget
        .charge(ExpansionResource::TermWork, 1, location())
        .unwrap();
    let before = budget.usage();
    cancellation.cancel();
    for amount in [0, u128::MAX] {
        assert!(matches!(
            budget.charge(ExpansionResource::TermWork, amount, location()),
            Err(ExpansionFailure::Interrupted {
                reason: Stop::Cancelled,
                location: site,
            }) if site == location()
        ));
        assert_eq!(budget.usage(), before);
    }
}

#[test]
fn mixed_budget_forks_keep_work_history_and_only_new_scalar_publication() {
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits::default());
    let mut original = budget(&allowance);
    original
        .charge(ExpansionResource::TermWork, 3, location())
        .unwrap();
    original
        .charge(ExpansionResource::ScalarBytes, 2, location())
        .unwrap();
    let mut tentative = original.clone();
    assert_eq!(tentative.usage(), original.usage());
    tentative
        .charge(ExpansionResource::TermWork, 2, location())
        .unwrap();
    tentative
        .charge(ExpansionResource::ScalarBytes, 1, location())
        .unwrap();
    assert_eq!(tentative.usage().term_work, 5);
    assert_eq!(original.usage().term_work, 3);
    drop(tentative);
    assert_eq!(allowance.statistics().scalar_bytes, 1);
    assert_eq!(allowance.statistics().work, 0);
    drop(original);
    assert_eq!(allowance.statistics().scalar_bytes, 3);
    assert_eq!(allowance.statistics().work, 0);
}

#[test]
fn wide_named_counts_keep_the_saturated_attempt_and_prefix() {
    for resource in [
        ExpansionResource::Templates,
        ExpansionResource::Values,
        ExpansionResource::ScalarBytes,
        ExpansionResource::Origins,
    ] {
        for initial in [0, 1, usize::MAX as u128] {
            let mut budget = Budget::new(
                ExpansionLimits {
                    max_templates: usize::MAX,
                    max_values: usize::MAX,
                    max_scalar_bytes: usize::MAX,
                    max_origin_locations: usize::MAX,
                    ..Default::default()
                },
                usize::MAX,
            );
            budget.charge(resource, initial, location()).unwrap();
            let before = budget.usage();
            budget.charge(resource, 0, location()).unwrap();
            assert_eq!(budget.usage(), before);
            let result = budget.charge(resource, u128::MAX, location());
            assert!(matches!(
                result,
                Err(ExpansionFailure::Limit {
                    resource: refused,
                    observed: u128::MAX,
                    limit,
                    location: site,
                }) if refused == resource && limit == usize::MAX as u128 && site == location()
            ));
            assert_eq!(budget.usage(), before);
        }
    }
}

#[test]
fn wide_named_counts_observe_control_before_refusal() {
    for resource in [
        ExpansionResource::Templates,
        ExpansionResource::Values,
        ExpansionResource::ScalarBytes,
        ExpansionResource::Origins,
    ] {
        let cancellation = Cancellation::default();
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX)
            .with_cancellation(Some(cancellation.clone()));
        budget.charge(resource, 1, location()).unwrap();
        let before = budget.usage();
        cancellation.cancel();
        for amount in [0, u128::MAX] {
            assert!(matches!(
                budget.charge(resource, amount, location()),
                Err(ExpansionFailure::Interrupted {
                    reason: Stop::Cancelled,
                    location: site,
                }) if site == location()
            ));
            assert_eq!(budget.usage(), before);
        }
    }
}

#[test]
fn direct_expansion_refusal_retains_wide_count_and_site() {
    let observed = u128::MAX;
    let site = location();
    let error = super::check(
        ExpansionResource::MetadataStatements,
        observed,
        usize::MAX,
        site,
    )
    .unwrap_err();
    assert!(matches!(error, ExpansionFailure::Limit {
        resource: ExpansionResource::MetadataStatements,
        observed: actual,
        limit,
        location: actual_site,
    } if actual == observed && limit == usize::MAX as u128 && actual_site == site));
}
