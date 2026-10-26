//! Ordinary solve policy, independent of command arguments and presentation.

use std::num::NonZeroUsize;

use crate::{Backend, GpuBatching, Grounder, Oracle, QueryDivision};

/// Policy for one semantic session. Budgets retain their existing ownership:
/// formula search and objective work are cumulative; verification work is per
/// call. No field selects an output format, file path, source loader or writer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SolveConfig {
    /// Execution backend: the CPU (the default) walks regions on an owned Rayon
    /// pool; a GPU backend requires a physical device of its API and runs exact
    /// integer batches there, with general formulas propagated on the GPU and
    /// their residual search exact on the CPU.
    pub backend: Backend,
    /// Materialization policy within the admitted input's supported profiles.
    pub grounder: Grounder,
    /// Exact membership policy; prepared formula inputs retain their original theory.
    pub oracle: Oracle,
    /// Enable optional host timing; semantic/resource counters remain independent.
    pub stats: bool,
    /// Per-check streamed-constraint limits for a hybrid formula session.
    /// Independent of core candidate/reduct work; eager sessions do not use them.
    /// Ordinary sessions retain only representational work/substitution maxima.
    pub constraints: zetesis_themelios::ConstraintCheckLimits,
    /// Maximum yielded models or retained optimum ties; zero requests all.
    pub models: usize,
    /// Explicit cumulative candidate restriction, encoding, certificate preparation
    /// and search allowance.
    /// Ordinary policy sets the counter representation maximum.
    pub max_search_work: u64,
    /// Cumulative formula branch decisions.
    pub max_search_decisions: u64,
    /// Work per verified formula model's construction. One-time preparation of
    /// the catalog's semantic order has a separate allowance of this same size.
    /// Failed attempts retain their accepted work in cumulative statistics.
    /// Independent of membership, reconstruction and scoring.
    pub max_model_work: u64,
    /// Peak live model-construction metadata: the prepared order and private
    /// selection/publication buffers, including actual growth overlap. Excludes
    /// immutable catalog payload, previously published models and allocator/Arc
    /// bookkeeping. This is a named capacity allowance, not process RSS.
    pub max_model_bytes: usize,
    /// Distinct projected answers an explicit `#project` session retains to
    /// exclude repeated projections.
    pub max_projection_entries: u64,
    /// Named capacity of that projection history and its conservative growth
    /// overlap. This excludes allocator overhead.
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
    /// How an optimizing search proves its optimum and finds its ties:
    /// [`crate::OptimumSearch::StrictForOneAnswer`] by default. The other
    /// schedules are kept for measurement; every one returns the same optima.
    pub optimum_search: crate::OptimumSearch,
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
    /// Maximum candidates per owned batch. On a GPU formula route under
    /// [`GpuBatching::Adaptive`], the size of the first candidate slate and of
    /// the slate bound feedback returns to; no device prefix shrinks a slate
    /// below it.
    pub batch_size: NonZeroUsize,
    /// Proposal scheduling for general and tight GPU formula membership.
    /// Adaptive by default ([`GpuBatching::DEFAULT`]); adaptive growth remains
    /// memory bounded and preserves the same ordered, all-or-nothing batch
    /// commit. CPU execution retains `batch_size` independently of this GPU
    /// formula policy.
    pub gpu_batching: GpuBatching,
    /// Worker count: the walkers of the region tree, one being the scalar walk.
    /// The library default is four; the command uses the host's available
    /// parallelism, or one when availability is unknown.
    pub workers: NonZeroUsize,
    /// When several CPU region walkers divide one candidate's proper-subset
    /// query among themselves: by default once it has run for the cost of
    /// engaging a worker while another waits idle. Answer sets are the same
    /// under every division. [`QueryDivision::Never`] keeps each query on the
    /// walker that reached its candidate, for measurement; the command sets no
    /// flag for it.
    pub query_division: QueryDivision,
    /// Worker count for unresolved formula queries: with one CPU region walker,
    /// and under general device propagation.
    /// Complete device certificates use scalar batch commit with no residual
    /// pool. Multiple CPU region workers decide their own leaves in `workers`.
    pub completion_workers: NonZeroUsize,
    /// Each proper-subset query's retained workspace capacity. Shared theory
    /// payload and allocator metadata are excluded.
    pub max_reduct_bytes: u64,
    /// Completion's ordered results and its workers' query workspaces, as
    /// created, excluding thread stacks, allocator/table overhead, scalar
    /// cursor and GPU storage.
    /// Optional class preparation/checking uses this ceiling independently;
    /// it is not a combined cap on class storage plus completion storage.
    pub max_completion_scratch_bytes: u64,
    /// Explicit candidate-count ceiling; ordinary policy uses its representation maximum.
    pub max_candidates: u64,
    /// Logical candidate restriction payload during source preparation.
    /// This excludes allocator slack and source relation/tree metadata.
    pub max_candidate_bytes: usize,
    /// Maximum charged work per independent original or frozen-reduct
    /// verification call, and per certified candidate check. It does not
    /// replace cumulative `max_search_work` or device propagation limits.
    pub max_work: u64,
    /// Maximum device work per formula candidate, independent of CPU work.
    /// Propagation charges setup and sweeps; tight support charges its complete
    /// scan. Insufficient mandatory work is refused before submission.
    pub gpu_formula_work: u32,
    /// Maximum device propagation sweeps per formula candidate. Zero performs
    /// original-truth setup and leaves undecided candidates to exact CPU search.
    /// This ceiling does not apply to the tight-support scan.
    pub gpu_formula_rounds: u32,
    /// Maximum atoms of one published model.
    pub max_atoms: usize,
    /// Maximum accounted pending/GPU batch payload bytes, for every device batch
    /// except the tight support check's, which has its own ceiling
    /// ([`Self::max_tight_batch_bytes`]). These conservative capacities
    /// exclude the overhead named by each owner and are not RSS.
    pub max_batch_bytes: u64,
    /// Maximum accounted payload bytes of one GPU tight support batch: the
    /// resident graph, transport, packing, results and, once admitted, the
    /// update state. It is separate from [`Self::max_batch_bytes`] so that the
    /// update state can still admit the batch size under smaller allowances;
    /// the state is admitted only when that many candidates fit beside it. Like
    /// the other capacities, it is conservative accounting, not RSS.
    pub max_tight_batch_bytes: u64,
    /// Share learned rows among a solve's candidate walks through one store,
    /// the default. Off, each walk keeps its rows to itself. A measurement
    /// switch with no command-line flag: answer sets are the same either way.
    pub share_learned_rows: bool,
}

impl SolveConfig {
    /// Shared ordinary defaults, derived from [`crate::Resources::DEFAULT`].
    /// Mandatory work and traversal counters have only representation maxima;
    /// memory-derived capacities and bounded optional/device operations remain.
    pub const DEFAULT: Self = crate::Resources::DEFAULT.solve_config();
}

impl SolveConfig {
    /// The workers that walk the region tree together and decide its
    /// leaves themselves: several, on a CPU backend. A device backend uses
    /// separate bounded candidate producers; those workers do not decide
    /// membership. One worker uses the scalar proposer, so both
    /// alternatives return `None`.
    #[must_use]
    pub fn region_workers(&self) -> Option<NonZeroUsize> {
        (self.workers.get() > 1 && self.backend == Backend::Cpu).then_some(self.workers)
    }
}

impl Default for SolveConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl SolveConfig {
    /// Reference memory used by the shared ordinary resource policy.
    pub const REFERENCE_MEMORY: u64 = crate::Resources::REFERENCE_MEMORY;

    /// Ordinary session policy for this memory allowance and worker capacity.
    /// Source and output consumers use the same [`crate::Resources`] owner to
    /// derive their capacities. These independent named shares are not RSS.
    #[must_use]
    pub fn for_allowance(memory: u64, workers: NonZeroUsize) -> Self {
        crate::Resources::new(memory, workers).solve_config()
    }
}

impl SolveConfig {
    /// Validate policies for finite formula admission before constructing it.
    ///
    /// This does not establish that any source is admissible or a device exists.
    ///
    /// # Errors
    /// Refuses lazy grounding, which needs the hybrid preparation.
    pub fn validate_formula(&self) -> Result<(), crate::SolveError> {
        if self.grounder == Grounder::Lazy {
            return Err(crate::SolveError::UnsupportedOracle {
                backend: self.backend,
                grounder: self.grounder,
            });
        }
        Ok(())
    }

    /// Validate the execution policy for a producer core with streamed source
    /// constraints. This does not admit source or construct an executor.
    ///
    /// # Errors
    /// Refuses eager materialization. The retained formula core uses the
    /// selected backend; streamed constraints are checked on the host.
    pub fn validate_hybrid(&self) -> Result<(), crate::SolveError> {
        if self.grounder == Grounder::Eager {
            return Err(crate::SolveError::PreparedInput {
                profile: crate::PreparedProfile::Hybrid,
                oracle: self.oracle,
                grounder: self.grounder,
            });
        }
        Ok(())
    }
}
