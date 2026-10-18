//! One shared preparation has observable applicability and exact work charges.

use super::super::{GeneratorOrderStatus, open};
use super::budget;
use crate::{Cancellation, Incomplete};

#[test]
fn prepared_order_work_is_included_once_in_reading_work() {
    let theory = crate::ferraris::fixtures::choice_beside_a_loop();
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, u64::MAX);
    let opened = open(&theory, u64::MAX, &mut budget).unwrap();
    let receipt = opened.statistics.generator_order;
    assert_eq!(receipt.status, GeneratorOrderStatus::Prepared);
    assert!(receipt.work > 0);
    assert!(receipt.peak_bytes > receipt.retained_bytes);
    assert!(receipt.retained_bytes > 0);
    assert_eq!(opened.statistics.counts.work, budget.statistics.work);
    assert!(opened.statistics.counts.work > receipt.work);
    assert!(
        opened
            .parts
            .subject(&theory)
            .generator_order()
            .unwrap()
            .belongs_to(&theory)
    );
}

#[test]
fn a_storage_decline_retains_the_existing_preference() {
    let theory = crate::ferraris::fixtures::choice_beside_a_loop();
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, u64::MAX);
    let opened = open(&theory, 0, &mut budget).unwrap();
    let receipt = opened.statistics.generator_order;
    assert_eq!(receipt.status, GeneratorOrderStatus::StorageDeclined);
    assert!(receipt.peak_bytes > 0);
    assert_eq!(receipt.retained_bytes, 0);
    assert!(opened.parts.subject(&theory).generator_order().is_none());
    assert!(opened.parts.subject(&theory).choices().contains(0));
    assert_eq!(opened.statistics.counts.work, budget.statistics.work);
}

#[test]
fn refused_preparation_preserves_the_search_work_prefix() {
    let theory = crate::ferraris::fixtures::choice_beside_a_loop();
    let cancellation = Cancellation::default();
    let mut completed_budget = budget(&cancellation, u64::MAX);
    let completed = open(&theory, u64::MAX, &mut completed_budget).unwrap();
    let preparation = completed.statistics.generator_order.work;
    let extraction = completed.statistics.counts.work - preparation;
    let ceiling = extraction + preparation / 2;
    let mut refused_budget = budget(&cancellation, ceiling);
    assert!(matches!(
        open(&theory, u64::MAX, &mut refused_budget),
        Err(Incomplete::WorkLimit)
    ));
    assert_eq!(refused_budget.statistics.work, ceiling);
}
