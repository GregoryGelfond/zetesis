//! Scalar receipt publication is independent of budget enforcement history.

use super::{Budget, ExpansionFailure, ExpansionLimits, ExpansionResource};
use crate::test_support::location;
use crate::{ConstraintAllowance, ConstraintCheckLimits};
use zetesis_cpu::{Cancellation, Stop};

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
