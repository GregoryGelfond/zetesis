//! Ordinary memory controls named byte capacities and retained populations.
//! Work counters and scheduling stay independent of the allowance; workers
//! share the collective closure capacity.

use zetesis_solve::SolveConfig;
use zetesis_test_support::counts::nonzero as workers;

#[test]
fn the_reference_allowance_with_the_default_workers_is_the_default() {
    let scaled = SolveConfig::for_allowance(SolveConfig::REFERENCE_MEMORY, workers(4));
    assert_eq!(scaled, SolveConfig::DEFAULT);
}

#[test]
fn doubling_memory_scales_only_named_capacities() {
    // Both allowances are below the signed host extent even on 32-bit hosts,
    // and every capacity share is integral before and after doubling.
    let memory = 64 * 1024 * 1024;
    let base = SolveConfig::for_allowance(memory, workers(4));
    let doubled = SolveConfig::for_allowance(2 * memory, workers(4));
    let expected = SolveConfig {
        max_projection_entries: 2 * base.max_projection_entries,
        max_projection_nodes: 2 * base.max_projection_nodes,
        max_objective_keys: 2 * base.max_objective_keys,
        max_optimal_models: 2 * base.max_optimal_models,
        max_optimal_atoms: 2 * base.max_optimal_atoms,
        max_carrier_atoms: 2 * base.max_carrier_atoms,
        max_atoms: 2 * base.max_atoms,
        max_ground_rules: 2 * base.max_ground_rules,
        max_projection_bytes: 2 * base.max_projection_bytes,
        max_objective_key_bytes: 2 * base.max_objective_key_bytes,
        max_optimal_bytes: 2 * base.max_optimal_bytes,
        max_model_bytes: 2 * base.max_model_bytes,
        max_reduct_bytes: 2 * base.max_reduct_bytes,
        max_completion_scratch_bytes: 2 * base.max_completion_scratch_bytes,
        max_candidate_bytes: 2 * base.max_candidate_bytes,
        max_closure_batch_bytes: 2 * base.max_closure_batch_bytes,
        max_closure_bytes: 2 * base.max_closure_bytes,
        max_batch_bytes: 2 * base.max_batch_bytes,
        ..base
    };
    // The full comparison also preserves mandatory/optional work, device
    // dispatch controls, batch size, workers and semantic execution policies.
    assert_eq!(doubled, expected);
}

#[test]
fn the_closure_share_is_the_collective_ceiling_over_the_workers() {
    let eight = SolveConfig::for_allowance(SolveConfig::REFERENCE_MEMORY, workers(8));
    assert_eq!(eight.workers, workers(8));
    assert_eq!(
        eight.max_closure_bytes,
        SolveConfig::DEFAULT.max_closure_batch_bytes / 8
    );
    assert!(eight.validate().is_ok());
}

#[test]
fn maximal_memory_respects_host_and_coordinate_extents() {
    let default = SolveConfig::DEFAULT;
    let vast = SolveConfig::for_allowance(u64::MAX, workers(1));
    let host_extent = u64::try_from(isize::MAX).unwrap();
    assert_eq!(vast, SolveConfig::for_allowance(host_extent, workers(1)));
    assert!(vast.max_reduct_bytes >= default.max_reduct_bytes);
    assert!(vast.max_closure_batch_bytes >= default.max_closure_batch_bytes);
    assert_eq!(vast.max_closure_bytes, vast.max_closure_batch_bytes);
    assert!(vast.max_batch_bytes >= default.max_batch_bytes);
    // Projection entries follow memory; trie coordinates additionally saturate
    // at their u32 representation. The host extent is tighter on 32-bit hosts.
    assert_eq!(
        vast.max_projection_nodes,
        usize::try_from(vast.max_projection_entries)
            .unwrap()
            .min(u32::MAX as usize)
    );
    if usize::BITS > 32 {
        assert_eq!(vast.max_projection_nodes, u32::MAX as usize);
        assert!(vast.max_projection_entries > u64::from(u32::MAX));
    }
}
