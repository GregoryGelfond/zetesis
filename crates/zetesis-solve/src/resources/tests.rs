//! Ordinary control is memory and cancellation, not arbitrary traversal fuel.

use super::Resources;
use std::num::NonZeroUsize;

#[test]
fn mandatory_traversal_uses_counter_representation() {
    let resources = Resources::new(64 * 1024 * 1024, NonZeroUsize::MIN);
    let solve = resources.solve_config();
    for ceiling in [
        solve.max_search_work,
        solve.max_search_decisions,
        solve.max_model_work,
        solve.max_objective_work,
        solve.max_objective_bindings,
        solve.max_candidates,
        solve.max_work,
        solve.max_source_work,
        solve.constraints.max_work,
        solve.constraints.max_substitutions,
    ] {
        assert_eq!(ceiling, u64::MAX);
    }
    let formula = resources.formula_limits();
    for ceiling in [
        formula.max_work,
        formula.max_substitutions,
        formula.max_support_rounds,
        formula.aggregate.max_work,
        formula.aggregate.max_subsets,
    ] {
        assert_eq!(ceiling, u64::MAX);
    }
    let expansion = resources.expansion_limits();
    assert_eq!(expansion.max_term_work, usize::MAX);
    assert_eq!(expansion.max_values, usize::MAX);
    assert_eq!(expansion.max_scalar_bytes, usize::MAX);
    assert_eq!(solve.constraints.max_scalar_bytes, usize::MAX);
    assert_eq!(resources.observation_limits().max_work, u64::MAX);
    assert_eq!(resources.observation_limits().max_bindings, u64::MAX);
}

#[test]
fn memory_controls_retained_populations() {
    let small = Resources::new(64 * 1024 * 1024, NonZeroUsize::MIN);
    let large = Resources::new(128 * 1024 * 1024, NonZeroUsize::MIN);
    let small_formula = small.formula_limits();
    let large_formula = large.formula_limits();
    assert!(small_formula.max_support_bytes < large_formula.max_support_bytes);
    assert!(small_formula.theory.max_nodes < large_formula.theory.max_nodes);
    assert!(small_formula.theory.max_operands < large_formula.theory.max_operands);
    assert!(small.expansion_limits().max_templates < large.expansion_limits().max_templates);
    assert!(
        small.admission_options().max_source_bytes < large.admission_options().max_source_bytes
    );
    assert!(
        small.program_admission_options().max_nodes < large.program_admission_options().max_nodes
    );
    assert!(small.bundle_limits().max_total_bytes < large.bundle_limits().max_total_bytes);
    assert!(
        small.observation_limits().max_output_bytes < large.observation_limits().max_output_bytes
    );
    assert!(small.json_record_bytes() < large.json_record_bytes());
}

#[test]
fn zero_memory_does_not_gain_population_headroom() {
    let resources = Resources::new(0, NonZeroUsize::MIN);
    let solve = resources.solve_config();
    assert_eq!(solve.max_closure_bytes, 0);
    assert_eq!(solve.max_atoms, 0);
    assert_eq!(solve.max_projection_nodes, 0);
    assert_eq!(resources.formula_limits().theory.max_nodes, 0);
    assert_eq!(resources.formula_limits().max_support_bytes, 0);
    assert_eq!(resources.expansion_limits().max_templates, 0);
    assert_eq!(resources.admission_options().max_syntax_nodes, 0);
    assert_eq!(resources.program_admission_options().max_nodes, 0);
    assert_eq!(resources.bundle_limits().max_files, 0);
    assert_eq!(resources.observation_limits().max_terms, 0);
    assert_eq!(resources.json_record_bytes(), 0);
}

#[test]
fn closure_workers_share_the_collective_capacity() {
    for memory in [0, 1, 63, 1024, Resources::REFERENCE_MEMORY, u64::MAX] {
        for workers in [1, 3, 7, 64] {
            let workers = NonZeroUsize::new(workers).unwrap();
            let config = Resources::new(memory, workers).solve_config();
            assert_eq!(config.workers, workers);
            assert!(
                config.max_closure_bytes.checked_mul(workers.get()).unwrap()
                    <= config.max_closure_batch_bytes
            );
        }
    }
}

#[test]
fn optional_and_device_effort_remains_bounded() {
    let resources = Resources::default();
    let solve = resources.solve_config();
    assert!(solve.max_objective_bound_work > 0 && solve.max_objective_bound_work < u64::MAX);
    assert!(solve.gpu_formula_work > 0 && solve.gpu_formula_work < u32::MAX);
    assert!(solve.gpu_formula_rounds > 0 && solve.gpu_formula_rounds < u32::MAX);
    let formula = resources.formula_limits();
    assert!(formula.max_key_work > 0 && formula.max_key_work < u64::MAX);
}

#[test]
fn ordinary_solve_defaults_share_the_factory() {
    assert_eq!(
        crate::SolveConfig::default(),
        Resources::default().solve_config()
    );
    let workers = NonZeroUsize::new(3).unwrap();
    assert_eq!(
        crate::SolveConfig::for_allowance(123_456_789, workers),
        Resources::new(123_456_789, workers).solve_config(),
    );
}

#[test]
fn maximal_memory_retains_checked_coordinate_extents() {
    let resources = Resources::new(u64::MAX, NonZeroUsize::MIN);
    let config = resources.solve_config();
    assert!(u32::try_from(config.max_projection_nodes).is_ok());
    let formula = resources.formula_limits();
    assert!(isize::try_from(formula.max_support_bytes).is_ok());
    assert!(
        formula
            .theory
            .max_nodes
            .checked_mul(size_of::<zetesis_ferraris::Node>())
            .is_some()
    );
    assert!(
        formula
            .theory
            .max_operands
            .checked_mul(size_of::<usize>())
            .is_some()
    );
}

#[test]
fn answer_projection_uses_memory_and_checked_work() {
    let small = Resources::new(64 * 1024 * 1024, NonZeroUsize::MIN).projection_limits();
    let large = Resources::new(128 * 1024 * 1024, NonZeroUsize::MIN).projection_limits();
    assert_eq!(large.max_keys, 2 * small.max_keys);
    assert_eq!(large.max_bytes, 2 * small.max_bytes);
    assert_eq!(large.max_work, u64::MAX);
}

#[test]
fn simultaneously_materialized_source_families_use_memory() {
    let small = Resources::new(64 * 1024 * 1024, NonZeroUsize::MIN).expansion_limits();
    let large = Resources::new(128 * 1024 * 1024, NonZeroUsize::MIN).expansion_limits();
    assert_eq!(small.max_family_bytes, 8 * 1024 * 1024);
    assert_eq!(large.max_family_bytes, 2 * small.max_family_bytes);
    assert_eq!(
        Resources::new(0, NonZeroUsize::MIN)
            .expansion_limits()
            .max_family_bytes,
        0
    );
}
