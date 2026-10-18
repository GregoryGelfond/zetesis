use super::*;

mod failure_tests;
mod footer_admission_tests;
mod frontier_tests;
mod terminal_tests;
mod source_tests;
mod objective_work_tests;
mod model_policy_tests;

#[test]
fn frozen_positive_json_preserves_failed_attempt_work() {
    let statistics = zetesis_sat::Statistics {
        reduct: zetesis_sat::ReductStatistics {
            frozen_positive_attempts: 3,
            frozen_positive_completed: 2,
            frozen_positive_preparation_work: 5,
            frozen_positive_work: 17,
            ..zetesis_sat::ReductStatistics::default()
        },
        ..zetesis_sat::Statistics::default()
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
