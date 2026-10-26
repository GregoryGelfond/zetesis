use super::*;

mod test_harness;
mod certificate_order_tests;
mod certificate_limits_tests;
mod batch_orchestration_tests;
mod partial_batch_tests;

#[test]
fn search_limits_follow_their_named_owner_allowances() {
    let limits = search_limits(&SolveConfig {
        max_candidate_bytes: 1024,
        max_reduct_bytes: 2048,
        ..SolveConfig::default()
    });
    assert_eq!(limits.max_native_narrowing_bytes, 1024);
    assert_eq!(limits.max_reduct_bytes, 2048);
}
