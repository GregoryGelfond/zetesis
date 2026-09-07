//! Ordinary solve policy, independent of command arguments and presentation.

use std::num::NonZeroUsize;

use crate::{Backend, Grounder, Options, Oracle};

/// Policy for one semantic session. Budgets retain their existing ownership:
/// formula search/objective work is cumulative; exact closure limits are per candidate.
/// No field selects an output format, file path, source loader or writer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SolveConfig {
    /// Execution backend policy.
    pub backend: Backend,
    /// Materialization policy for relational programs.
    pub grounder: Grounder,
    /// Exact membership policy; prepared formula inputs retain their original theory.
    pub oracle: Oracle,
    /// Enable optional host timing; semantic/resource counters remain independent.
    pub stats: bool,
    /// Maximum yielded models or retained optimum ties; zero requests all.
    pub models: usize,
    /// Cumulative formula encoding and search work.
    pub max_search_work: u64,
    /// Cumulative formula branch decisions.
    pub max_search_decisions: u64,
    /// Cumulative objective evaluation work.
    pub max_objective_work: u64,
    /// Cumulative optional objective-bound work; zero disables pruning.
    pub max_objective_bound_work: u64,
    /// Complete objective bindings per verified model.
    pub max_objective_bindings: u64,
    /// Distinct objective keys per verified model.
    pub max_objective_keys: usize,
    /// Objective key payload bytes per verified model.
    pub max_objective_key_bytes: usize,
    /// Maximum retained incumbent models.
    pub max_optimal_models: usize,
    /// Maximum atoms across retained incumbents.
    pub max_optimal_atoms: usize,
    /// Maximum retained incumbent payload bytes, excluding allocator overhead.
    pub max_optimal_bytes: usize,
    /// Maximum candidates per owned batch.
    pub batch_size: NonZeroUsize,
    /// Closure worker count.
    pub workers: NonZeroUsize,
    /// Formula exact-completion worker count.
    pub completion_workers: NonZeroUsize,
    /// Logical completion scratch bytes, excluding thread stacks and allocator overhead.
    pub max_completion_scratch_bytes: u64,
    /// Maximum candidate seeds or formula candidates.
    pub max_candidates: u64,
    /// Maximum incrementally retained gate atoms.
    pub max_carrier_atoms: usize,
    /// Maximum charged exact-oracle work per candidate.
    pub max_work: u64,
    /// Maximum derived or eagerly materialized atoms.
    pub max_atoms: usize,
    /// Maximum substitutions in explicit static lowering.
    pub max_substitutions: usize,
    /// Maximum rules in explicit static lowering.
    pub max_ground_rules: usize,
    /// Maximum accounted pending/GPU batch payload bytes.
    pub max_batch_bytes: u64,
}

impl SolveConfig {
    /// Shared ordinary-solve defaults used by both library sessions and CLI flags.
    pub const DEFAULT: Self = Self {
        backend: Backend::Auto,
        grounder: Grounder::Auto,
        oracle: Oracle::Auto,
        stats: false,
        models: 1,
        max_search_work: 100_000_000,
        max_search_decisions: 1_000_000,
        max_objective_work: 100_000_000,
        max_objective_bound_work: 10_000_000,
        max_objective_bindings: 1_000_000,
        max_objective_keys: 1_000_000,
        max_objective_key_bytes: 67_108_864,
        max_optimal_models: 100_000,
        max_optimal_atoms: 1_000_000,
        max_optimal_bytes: 67_108_864,
        batch_size: NonZeroUsize::new(64).unwrap(),
        workers: NonZeroUsize::new(4).unwrap(),
        completion_workers: NonZeroUsize::new(1).unwrap(),
        max_completion_scratch_bytes: 268_435_456,
        max_candidates: 1_000_000,
        max_carrier_atoms: 4_096,
        max_work: 10_000_000,
        max_atoms: 1_000_000,
        max_substitutions: 10_000_000,
        max_ground_rules: 1_000_000,
        max_batch_bytes: 67_108_864,
    };
}

impl Default for SolveConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl From<&Options> for SolveConfig {
    fn from(options: &Options) -> Self {
        Self {
            backend: options.backend,
            grounder: options.grounder,
            oracle: options.oracle,
            stats: options.stats,
            models: options.models,
            max_search_work: options.max_search_work,
            max_search_decisions: options.max_search_decisions,
            max_objective_work: options.max_objective_work,
            max_objective_bound_work: options.max_objective_bound_work,
            max_objective_bindings: options.max_objective_bindings,
            max_objective_keys: options.max_objective_keys,
            max_objective_key_bytes: options.max_objective_key_bytes,
            max_optimal_models: options.max_optimal_models,
            max_optimal_atoms: options.max_optimal_atoms,
            max_optimal_bytes: options.max_optimal_bytes,
            batch_size: options.batch_size,
            workers: options.workers,
            completion_workers: options.completion_workers,
            max_completion_scratch_bytes: options.max_completion_scratch_bytes,
            max_candidates: options.max_candidates,
            max_carrier_atoms: options.max_carrier_atoms,
            max_work: options.max_work,
            max_atoms: options.max_atoms,
            max_substitutions: options.max_substitutions,
            max_ground_rules: options.max_ground_rules,
            max_batch_bytes: options.max_batch_bytes,
        }
    }
}
