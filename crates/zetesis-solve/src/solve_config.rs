//! Ordinary solve policy, independent of command arguments and presentation.

use std::num::NonZeroUsize;

use crate::{Backend, Grounder, Oracle, SearchMethod, SourceBatching};

const DEFAULT_MAX_SUBSTITUTIONS: usize = 10_000_000;

/// Policy for one semantic session. Budgets retain their existing ownership:
/// Formula search/objective work is cumulative. Independent closure work is per
/// candidate; shared CPU rounds separate collective source and per-world work.
/// No field selects an output format, file path, source loader or writer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SolveConfig {
    /// Execution backend: the CPU (the default) runs source joins or static
    /// closure scans on an owned Rayon pool; a GPU backend requires a physical
    /// device of its API and runs exact integer batches there, with general
    /// formulas propagated on the GPU and their residual search exact on the CPU.
    pub backend: Backend,
    /// Materialization policy within the admitted input's supported profiles.
    pub grounder: Grounder,
    /// Relational source traversal; the sharing strategies require CPU execution.
    pub source_batching: SourceBatching,
    /// Exact membership policy; prepared formula inputs retain their original theory.
    pub oracle: Oracle,
    /// How the formula route proposes candidates and queries the reduct.
    pub search: SearchMethod,
    /// Enable optional host timing; semantic/resource counters remain independent.
    pub stats: bool,
    /// Per-check streamed-constraint limits for a hybrid formula session.
    /// Independent of core candidate/reduct work; eager sessions do not use them.
    /// The default substitution ceiling matches the ordinary session allowance.
    pub constraints: zetesis_themelios::ConstraintCheckLimits,
    /// Maximum yielded models or retained optimum ties; zero requests all.
    pub models: usize,
    /// Cumulative candidate restriction or formula encoding and search work.
    pub max_search_work: u64,
    /// Cumulative formula branch decisions.
    pub max_search_decisions: u64,
    /// Cumulative prepared-order and selected-model construction work per formula
    /// session, including failed attempts. Independent of membership and scoring.
    pub max_model_work: u64,
    /// Peak live model-construction metadata: the prepared order and private
    /// selection/publication buffers, including actual growth overlap. Excludes
    /// immutable catalog payload, previously published models and allocator/Arc
    /// bookkeeping. This is a named capacity allowance, not process RSS.
    pub max_model_bytes: usize,
    /// Distinct candidate projections retained to exclude previously visited keys.
    pub max_projection_entries: u64,
    /// Logical nodes in the retained candidate-projection trie.
    pub max_projection_nodes: usize,
    /// Named projection-history capacity and conservative growth overlap.
    /// This excludes allocator overhead and the separate authored CNF encoding.
    pub max_projection_bytes: usize,
    /// Cumulative objective preparation and evaluation work, including refusals.
    pub max_objective_work: u64,
    /// Cumulative optional bound-generation work, excluding shared preparation;
    /// zero disables pruning while prepared scoring remains available.
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
    /// Maximum portable incumbent payload bytes: each distinct occurrence
    /// catalog's encoding once, selected positions per retained model, and one
    /// shared best-score record. Equal-content separate catalogs count separately.
    /// Excludes canonical identities outside occurrence maps, spare vector/hash
    /// capacity, owner-index entries, allocator/Arc overhead, subjects, execution
    /// state and transient old/new replacement overlap; this is not RSS.
    pub max_optimal_bytes: usize,
    /// Maximum candidates per owned batch.
    pub batch_size: NonZeroUsize,
    /// Worker count: the closure route's pool, and the walkers of the
    /// region tree under the regions method, one being the scalar walk.
    /// CPU closure setup conservatively reserves `max_closure_bytes` per worker;
    /// [`Self::for_allowance`] keeps that product within the collective ceiling.
    /// The library default is four; the command uses the host's available
    /// parallelism, or one when availability is unknown.
    pub workers: NonZeroUsize,
    /// Worker count for unresolved formula queries: under the clauses search,
    /// and under regions with one CPU walker or general device propagation.
    /// Complete device certificates use scalar validation with no residual
    /// pool. Multiple CPU region workers decide their own leaves in `workers`.
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
    /// CPU closure setup conservatively admits every assigned worker at this
    /// allowance, including eager and shared execution, so
    /// `workers * max_closure_bytes` must not exceed `max_closure_batch_bytes`;
    /// that route refuses an excessive or overflowing product before allocating
    /// its pool, compiling static rules or initializing candidates. Formula and
    /// device execution do not use this reservation.
    pub max_closure_bytes: usize,
    /// Collective independent CPU preparation, cached workspace and active
    /// closure allowance. Also bounds immutable query preparation bytes.
    /// Shared-round and GPU batch storage remain separate.
    pub max_closure_batch_bytes: usize,
    /// Maximum device work per formula candidate, independent of CPU work.
    /// Propagation charges setup and sweeps; tight support charges its complete
    /// scan. Insufficient mandatory work is refused before submission.
    pub gpu_formula_work: u32,
    /// Maximum device propagation sweeps per formula candidate. Zero performs
    /// original-truth setup and leaves undecided candidates to exact CPU search.
    /// This ceiling does not apply to the tight-support scan.
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
    /// The retained-storage byte ceilings are shares of [`Self::REFERENCE_MEMORY`];
    /// [`Self::for_allowance`] scales them by a session's allowance and
    /// shares the closure ceiling by its workers, which is how the command
    /// takes the host's memory and parallelism. Cumulative scalar-copy limits
    /// count work across source checks and are not scaled with retained storage.
    pub const DEFAULT: Self = Self {
        backend: Backend::Cpu,
        grounder: Grounder::Auto,
        source_batching: SourceBatching::Independent,
        oracle: Oracle::Auto,
        search: SearchMethod::Regions,
        stats: false,
        constraints: zetesis_themelios::ConstraintCheckLimits {
            max_substitutions: DEFAULT_MAX_SUBSTITUTIONS as u64,
            ..zetesis_themelios::ConstraintCheckLimits::DEFAULT
        },
        models: 1,
        max_search_work: 10_000_000_000,
        max_search_decisions: 10_000_000,
        max_model_work: 1_000_000_000,
        max_model_bytes: 67_108_864,
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
        max_substitutions: DEFAULT_MAX_SUBSTITUTIONS,
        max_ground_rules: 1_000_000,
        max_batch_bytes: 67_108_864,
    };
}

impl SolveConfig {
    /// The workers that walk the region tree together and decide its
    /// leaves themselves: several, under the regions method on a CPU
    /// backend. A device backend uses separate bounded candidate producers;
    /// those workers do not decide membership. One worker or clause search
    /// uses the scalar proposer, so all these alternatives return `None`.
    #[must_use]
    pub fn region_workers(&self) -> Option<NonZeroUsize> {
        (self.search == SearchMethod::Regions
            && self.workers.get() > 1
            && self.backend == Backend::Cpu)
            .then_some(self.workers)
    }
}

impl Default for SolveConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl SolveConfig {
    /// Two gibibytes: the memory allowance the byte ceilings of
    /// [`Self::DEFAULT`] are the shares of.
    pub const REFERENCE_MEMORY: u64 = 2 * 1024 * 1024 * 1024;

    /// The defaults for a session allowed `memory` bytes over `workers`:
    /// [`Self::DEFAULT`] with the ten byte ceilings that bound retained
    /// storage scaled by `memory` over [`Self::REFERENCE_MEMORY`], each
    /// saturating at its type's maximum, and the per-closure allowance each
    /// worker's share of the scaled collective closure ceiling, so that the
    /// product checked by CPU closure setup holds. The projection, objective
    /// key, incumbent, model construction, reduct, completion scratch, candidate, collective
    /// closure and batch ceilings scale; work, count and structural
    /// ceilings, and the ceilings of source admission, do not. Constant
    /// time.
    #[must_use]
    pub fn for_allowance(memory: u64, workers: NonZeroUsize) -> Self {
        let scale_u64 = |default: u64| {
            let scaled =
                u128::from(default) * u128::from(memory) / u128::from(Self::REFERENCE_MEMORY);
            u64::try_from(scaled).unwrap_or(u64::MAX)
        };
        let scale_usize = |default: usize| {
            usize::try_from(scale_u64(u64::try_from(default).unwrap_or(u64::MAX)))
                .unwrap_or(usize::MAX)
        };
        let max_closure_batch_bytes = scale_usize(Self::DEFAULT.max_closure_batch_bytes);
        Self {
            workers,
            max_projection_bytes: scale_usize(Self::DEFAULT.max_projection_bytes),
            max_objective_key_bytes: scale_usize(Self::DEFAULT.max_objective_key_bytes),
            max_optimal_bytes: scale_usize(Self::DEFAULT.max_optimal_bytes),
            max_model_bytes: scale_usize(Self::DEFAULT.max_model_bytes),
            max_reduct_bytes: scale_u64(Self::DEFAULT.max_reduct_bytes),
            max_completion_scratch_bytes: scale_u64(Self::DEFAULT.max_completion_scratch_bytes),
            max_candidate_bytes: scale_usize(Self::DEFAULT.max_candidate_bytes),
            max_closure_bytes: max_closure_batch_bytes / workers.get(),
            max_closure_batch_bytes,
            max_batch_bytes: scale_u64(Self::DEFAULT.max_batch_bytes),
            ..Self::DEFAULT
        }
    }
}

impl SolveConfig {
    /// Validate execution-policy combinations before admitting a source.
    ///
    /// This performs no admission, execution, allocation or device discovery.
    /// A later prepared input can impose additional representation constraints.
    /// Execution-specific resource checks happen only when their route is set
    /// up. CPU closure setup checks the worker reservation before allocating or
    /// initializing execution state. After policy validation, a pre-cancelled
    /// session stops before those resource checks and allocations.
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
        if self.grounder == Grounder::Lazy {
            return Err(crate::SolveError::UnsupportedOracle {
                backend: self.backend,
                grounder: self.grounder,
            });
        }
        crate::engine::validate_countermodel(self)
    }

    /// Validate the execution policy for a producer core with streamed source
    /// constraints. This does not admit source or construct an executor.
    ///
    /// # Errors
    /// Refuses eager materialization, closure membership, device execution and
    /// shared relational source rounds for this initial CPU formula profile.
    pub fn validate_hybrid(&self) -> Result<(), crate::SolveError> {
        if self.source_batching != SourceBatching::Independent {
            return Err(crate::SolveError::UnsupportedSourceBatching);
        }
        if self.backend.is_gpu() {
            return Err(crate::SolveError::HybridBackend {
                backend: self.backend,
            });
        }
        if self.oracle == Oracle::Closure || self.grounder == Grounder::Eager {
            return Err(crate::SolveError::PreparedInput {
                profile: crate::PreparedProfile::Hybrid,
                oracle: self.oracle,
                grounder: self.grounder,
            });
        }
        Ok(())
    }
}
