use super::*;

mod failure_tests;
mod footer_admission_tests;
mod frontier_tests;
mod residual_sample_tests;
mod terminal_tests;
mod source_tests;
mod objective_work_tests;
mod model_policy_tests;

#[test]
fn region_json_preserves_support_work_components() {
    let mut statistics = zetesis_stable::RegionSearchStatistics::default();
    statistics.counts.work = 23;
    statistics.counts.support_work = 5;
    statistics.counts.founded_work = 11;
    let mut output = Buffer::new(4096);
    region_statistics(&mut output, Some(&statistics)).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&output.bytes).unwrap();
    assert_eq!(value["reading_work"], 23);
    assert_eq!(value["support_work"], 5);
    assert_eq!(value["founded_work"], 11);
}

#[test]
fn frozen_positive_json_preserves_failed_attempt_work() {
    let mut reduct = zetesis_stable::ReductStatistics::default();
    reduct.frozen_positive_attempts = 3;
    reduct.frozen_positive_completed = 2;
    reduct.frozen_positive_preparation_work = 5;
    reduct.frozen_positive_work = 17;
    let statistics = zetesis_stable::Statistics {
        reduct,
        ..zetesis_stable::Statistics::default()
    };
    let mut output = Buffer::new(4096);
    search_statistics(&mut output, Some(&statistics), "test").unwrap();
    let value: serde_json::Value = serde_json::from_slice(&output.bytes).unwrap();
    assert_eq!(
        value["frozen_positive"],
        serde_json::json!({
            "attempts": 3,
            "completed": 2,
            "preparation_work": 5,
            "work": 17,
        })
    );
}

#[test]
fn inner_preparation_json_lists_every_receipt() {
    let mut reduct = zetesis_stable::ReductStatistics::default();
    reduct.regions.regions = 7;
    reduct.inner_preparation.attempts = 4;
    reduct.inner_preparation.prepared = 3;
    reduct.inner_preparation.declined = 1;
    reduct.inner_preparation.saturation_units = 3;
    reduct.inner_preparation.groups = 30;
    reduct.inner_preparation.probes = 243;
    reduct.inner_preparation.refuted_patterns = 120;
    reduct.inner_preparation.rows = 29;
    reduct.inner_preparation.inexact_hulls = 2;
    reduct.inner_preparation.root_refuted = 1;
    reduct.inner_preparation.work = 5_000;
    let statistics = zetesis_stable::Statistics {
        reduct,
        ..zetesis_stable::Statistics::default()
    };
    let mut output = Buffer::new(4096);
    search_statistics(&mut output, Some(&statistics), "test").unwrap();
    let value: serde_json::Value = serde_json::from_slice(&output.bytes).unwrap();
    assert_eq!(value["reduct_query_regions"]["visited"], 7);
    assert_eq!(
        value["reduct_query_regions"]["preparation"],
        serde_json::json!({
            "attempts": 4,
            "prepared": 3,
            "declined": 1,
            "saturation_units": 3,
            "groups": 30,
            "probes": 243,
            "refuted_patterns": 120,
            "rows": 29,
            "inexact_hulls": 2,
            "root_refuted": 1,
            "work": 5000,
        })
    );
}

#[test]
fn region_json_preserves_support_explanation_receipts() {
    let mut statistics = zetesis_stable::RegionSearchStatistics::default();
    statistics.counts.work = 31;
    statistics.counts.support_explanations = zetesis_ferraris::SupportExplanationStatistics {
        attempts: 2,
        certified: 1,
        matched: 3,
        cut: 4,
        forced: 6,
        refuted: 1,
        declined: 1,
        work: 19,
        peak_bytes: 43,
    };
    let mut output = Buffer::new(4096);
    region_statistics(&mut output, Some(&statistics)).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&output.bytes).unwrap();
    assert_eq!(value["reading_work"], 31);
    assert_eq!(
        value["support_explanations"],
        serde_json::json!({
            "attempts": 2,
            "certified": 1,
            "matched": 3,
            "cut": 4,
            "forced": 6,
            "refuted": 1,
            "declined": 1,
            "work": 19,
            "peak_bytes": 43,
        })
    );
}
