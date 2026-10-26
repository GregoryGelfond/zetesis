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

#[test]
fn learned_tiers_grow_with_the_candidate_allowance() {
    let limits = |bytes| {
        search_limits(&SolveConfig {
            max_candidate_bytes: bytes,
            ..SolveConfig::default()
        })
        .learning
    };
    assert_eq!(limits(1024), zetesis_ferraris::LearningLimits::DEFAULT);
    assert_eq!(limits(512 << 20).tier_rows, 512);
}
