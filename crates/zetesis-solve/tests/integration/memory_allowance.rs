//! The library's byte ceilings are the shares of a reference allowance; a
//! session's allowance scales them, and its worker count shares the
//! collective closure ceiling among the workers.

use std::num::NonZeroUsize;

use zetesis_solve::SolveConfig;

fn workers(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}

#[test]
fn the_reference_allowance_with_the_default_workers_is_the_default() {
    let scaled = SolveConfig::for_allowance(SolveConfig::REFERENCE_MEMORY, workers(4));
    assert_eq!(scaled, SolveConfig::DEFAULT);
}

#[test]
fn a_doubled_allowance_doubles_the_byte_ceilings_and_no_other_ceiling() {
    let default = SolveConfig::DEFAULT;
    let doubled = SolveConfig::for_allowance(2 * SolveConfig::REFERENCE_MEMORY, workers(4));
    assert_eq!(
        doubled.max_projection_bytes,
        2 * default.max_projection_bytes
    );
    assert_eq!(
        doubled.max_objective_key_bytes,
        2 * default.max_objective_key_bytes
    );
    assert_eq!(doubled.max_optimal_bytes, 2 * default.max_optimal_bytes);
    assert_eq!(doubled.max_model_bytes, 2 * default.max_model_bytes);
    assert_eq!(doubled.max_reduct_bytes, 2 * default.max_reduct_bytes);
    assert_eq!(
        doubled.max_completion_scratch_bytes,
        2 * default.max_completion_scratch_bytes
    );
    assert_eq!(doubled.max_candidate_bytes, 2 * default.max_candidate_bytes);
    assert_eq!(
        doubled.max_closure_batch_bytes,
        2 * default.max_closure_batch_bytes
    );
    assert_eq!(doubled.max_closure_bytes, 2 * default.max_closure_bytes);
    assert_eq!(doubled.max_batch_bytes, 2 * default.max_batch_bytes);
    let same = SolveConfig {
        max_projection_bytes: default.max_projection_bytes,
        max_objective_key_bytes: default.max_objective_key_bytes,
        max_optimal_bytes: default.max_optimal_bytes,
        max_model_bytes: default.max_model_bytes,
        max_reduct_bytes: default.max_reduct_bytes,
        max_completion_scratch_bytes: default.max_completion_scratch_bytes,
        max_candidate_bytes: default.max_candidate_bytes,
        max_closure_batch_bytes: default.max_closure_batch_bytes,
        max_closure_bytes: default.max_closure_bytes,
        max_batch_bytes: default.max_batch_bytes,
        ..doubled
    };
    assert_eq!(same, default);
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
fn the_largest_allowance_scales_every_ceiling_without_overflow() {
    // Scaling in the ceiling's own width would overflow here; the wide
    // arithmetic scales each ceiling up and saturates at its type's maximum.
    let default = SolveConfig::DEFAULT;
    let vast = SolveConfig::for_allowance(u64::MAX, workers(1));
    assert!(vast.max_reduct_bytes > default.max_reduct_bytes);
    assert!(vast.max_closure_batch_bytes > default.max_closure_batch_bytes);
    assert_eq!(vast.max_closure_bytes, vast.max_closure_batch_bytes);
    assert!(vast.max_batch_bytes > default.max_batch_bytes);
}
