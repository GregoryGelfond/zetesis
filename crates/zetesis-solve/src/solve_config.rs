//! Ordinary solve policy, independent of command arguments and presentation.

use std::num::NonZeroUsize;

use crate::{Backend, Grounder, Oracle, SourceBatching};

/// Policy for one semantic session. Budgets retain their existing ownership:
/// Formula search/objective work is cumulative. Independent closure work is per
/// candidate; shared CPU rounds separate collective source and per-world work.
/// No field selects an output format, file path, source loader or writer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SolveConfig {
    /// Execution backend policy.
    pub backend: Backend,
    /// Materialization policy for relational programs.
    pub grounder: Grounder,
    /// Relational source traversal; the sharing strategies require CPU execution.
    pub source_batching: SourceBatching,
    /// Exact membership policy; prepared formula inputs retain their original theory.
    pub oracle: Oracle,
    /// Enable optional host timing; semantic/resource counters remain independent.
    pub stats: bool,
    /// Maximum yielded models or retained optimum ties; zero requests all.
    pub models: usize,
    /// Cumulative candidate restriction or formula encoding and search work.
    pub max_search_work: u64,
    /// Cumulative formula branch decisions.
    pub max_search_decisions: u64,
    /// Distinct candidate projections retained to exclude previously visited keys.
    pub max_projection_entries: u64,
    /// Logical nodes in the retained candidate-projection trie.
    pub max_projection_nodes: usize,
    /// Named projection-history capacity and conservative growth overlap.
    /// This excludes allocator overhead and the separate authored CNF encoding.
    pub max_projection_bytes: usize,
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
    /// Maximum canonical incumbent payload bytes: each distinct catalog owner
    /// once, selected positions per retained model, and one shared best-score
    /// record. Equal-content separate catalogs count separately. Excludes spare
    /// vector/hash capacity, owner-index entries, allocator/Arc overhead, subjects,
    /// execution state and transient old/new replacement overlap; this is not RSS.
    pub max_optimal_bytes: usize,
    /// Maximum candidates per owned batch.
    pub batch_size: NonZeroUsize,
    /// Closure worker count.
    pub workers: NonZeroUsize,
    /// Formula exact-completion worker count.
    pub completion_workers: NonZeroUsize,
    /// Named cold reduct preparation and each query's retained capacity.
    /// The immutable reduct is prepared once per original theory. Parallel
    /// completion also admits its shared owner against the collective ceiling.
    /// Shared theory payload and allocator metadata are excluded.
    pub max_reduct_bytes: u64,
    /// Shared prepared reduct, worker query capacities and transient/result bytes, excluding
    /// thread stacks, allocator/table overhead, scalar cursor and GPU storage.
    /// Optional class preparation/checking uses this ceiling independently;
    /// it is not a combined cap on class storage plus completion storage.
    pub max_completion_scratch_bytes: u64,
    /// Maximum candidate seeds or formula candidates.
    pub max_candidates: u64,
    /// Maximum incrementally retained gate atoms.
    pub max_carrier_atoms: usize,
    /// Logical candidate restriction payload during source preparation.
    /// This excludes allocator slack and source relation/tree metadata.
    pub max_candidate_bytes: usize,
    /// Maximum charged CPU oracle work per candidate, or shared host source
    /// work per lazy GPU batch. Shared CPU rounds count record visits plus
    /// antecedent tests per world; independent joins and device units differ.
    /// Formula execution also applies this ceiling per independent original or
    /// frozen-reduct verification call, and per certified candidate check. It
    /// does not replace cumulative `max_search_work` or device propagation limits.
    pub max_work: u64,
    /// Named storage for one independent lazy CPU closure construction.
    /// Input seeds and completed model retention belong to separate owners.
    pub max_closure_bytes: usize,
    /// Collective independent CPU preparation, cached workspace and active
    /// closure allowance. Also bounds immutable query preparation bytes.
    /// Shared-round and GPU batch storage remain separate.
    pub max_closure_batch_bytes: usize,
    /// Maximum device propagation work per formula candidate, independent of
    /// CPU work. Below mandatory setup work the device refuses before submission.
    pub gpu_formula_work: u32,
    /// Maximum device propagation sweeps per formula candidate. Zero performs
    /// original-truth setup and leaves undecided candidates to exact CPU search.
    pub gpu_formula_rounds: u32,
    /// Collective source work per shared CPU batch, or immutable query
    /// preparation work for independent CPU closure. Candidate work is separate.
    pub max_source_work: u64,
    /// Maximum derived independent CPU atoms, demanded shared CPU/GPU batch
    /// catalog atoms, eager atoms, or copied candidate restriction atom occurrences.
    /// Shared catalogs and candidate preparation are collective, not per world.
    pub max_atoms: usize,
    /// Maximum substitutions in explicit static lowering.
    pub max_substitutions: usize,
    /// Maximum rules in explicit static lowering.
    pub max_ground_rules: usize,
    /// Maximum accounted pending/GPU batch payload bytes. Lazy GPU splits this
    /// allowance equally between source coordination and transient transport.
    /// Shared CPU rounds use the full allowance for source and world state;
    /// allocator, tree and driver overhead are outside the payload bound.
    pub max_batch_bytes: u64,
}

impl SolveConfig {
    /// Shared ordinary-solve defaults used by both library sessions and CLI flags.
    /// These finite session allowances differ from standalone primitive defaults.
    /// Logical work ceilings do not impose a wall-clock deadline or remove the
    /// independently configured source, storage and materialization limits.
    pub const DEFAULT: Self = Self {
        backend: Backend::Auto,
        grounder: Grounder::Auto,
        source_batching: SourceBatching::Independent,
        oracle: Oracle::Auto,
        stats: false,
        models: 1,
        max_search_work: 10_000_000_000,
        max_search_decisions: 10_000_000,
        max_projection_entries: zetesis_sat::ProjectionLimits::DEFAULT.max_entries,
        max_projection_nodes: zetesis_sat::ProjectionLimits::DEFAULT.max_nodes,
        max_projection_bytes: zetesis_sat::ProjectionLimits::DEFAULT.max_bytes,
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
        max_reduct_bytes: zetesis_sat::ReductPreparationLimits::DEFAULT_BYTES,
        max_completion_scratch_bytes: 268_435_456,
        max_candidates: 10_000_000,
        max_carrier_atoms: 4_096,
        max_candidate_bytes: 67_108_864,
        max_work: 100_000_000,
        max_closure_bytes: 134_217_728,
        max_closure_batch_bytes: zetesis_cpu::BatchOracle::DEFAULT_CLOSURE_BYTES,
        gpu_formula_work: 100_000_000,
        gpu_formula_rounds: 64,
        max_source_work: 10_000_000,
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

impl SolveConfig {
    /// Validate execution-policy combinations before admitting a source.
    ///
    /// This performs no admission, execution, allocation or device discovery.
    /// A later prepared input can impose additional representation constraints.
    ///
    /// # Errors
    /// Refuses incompatible oracle, grounding and shared-source policies.
    pub fn validate(&self) -> Result<(), crate::SolveError> {
        crate::engine::validate_combination(self)
    }

    /// Validate policies for finite formula admission before constructing it.
    ///
    /// This does not establish that any source is admissible or a device exists.
    ///
    /// # Errors
    /// Refuses lazy grounding and relational shared-source policies.
    pub fn validate_formula(&self) -> Result<(), crate::SolveError> {
        crate::engine::validate_countermodel(self)
    }
}
