//! Ordinary execution resources shared by source, session and output consumers.
//!
//! Work, visited substitutions, rounds and copied-byte traffic are receipts,
//! bounded only by their counter representation. Cancellation and an optional
//! deadline remain the control boundary. Explicit low-level limit types still
//! support finite operation allowances. Optional analysis and device dispatch
//! retain bounded effort. Optional analyses fail open; mandatory device setup
//! can still refuse a dispatch that exceeds its internal capacity.
//!
//! Memory supplies conservative, independent named capacity shares: support and
//! formula storage use quarters, canonical/source owners eighths, and ancillary
//! metadata/output owners sixteenths or smaller. Population limits use logical
//! cell envelopes within those shares. These owners can overlap; their sum is
//! not the allowance, and neither the allowance nor these counts guarantee RSS.
//! Allocator overhead, source-library allocations and thread stacks remain outside
//! the named accounting. Existing checked depth limits protect recursive ingress.

use std::num::NonZeroUsize;

use crate::{Backend, Grounder, Oracle, SearchMethod, SolveConfig, SourceBatching};
use zetesis_themelios::{
    AdmissionOptions, BundleLimits, ExpansionLimits, FormulaLimits, ProgramAdmissionOptions,
    observation,
};

const DEFAULT_BATCH_SIZE: NonZeroUsize = NonZeroUsize::new(64).unwrap();

/// Ordinary named-memory allowance and CPU worker capacity.
///
/// No selected work ceiling is stored here. Pass the same cancellation token
/// through preparation and the session to impose an optional deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resources {
    memory: usize,
    workers: NonZeroUsize,
}

impl Resources {
    /// Reference memory for ordinary library defaults: two gibibytes.
    pub const REFERENCE_MEMORY: u64 = 2 * 1024 * 1024 * 1024;
    /// Reference policy with four CPU workers. Frontends can select the host's
    /// parallelism and construct this same policy with [`Self::new`].
    pub const DEFAULT: Self = Self {
        memory: if usize::BITS > 32 {
            2 * 1024 * 1024 * 1024
        } else {
            isize::MAX as usize
        },
        workers: NonZeroUsize::new(4).unwrap(),
    };

    /// Derive every ordinary capacity from one allowance. Clamp to the maximum
    /// signed allocation extent supported by this host; zero remains zero.
    #[must_use]
    pub fn new(memory: u64, workers: NonZeroUsize) -> Self {
        Self {
            memory: usize::try_from(memory)
                .unwrap_or(usize::MAX)
                .min(isize::MAX as usize),
            workers,
        }
    }

    const fn bytes(self, denominator: usize) -> usize {
        self.memory / denominator
    }

    const fn cells(self, denominator: usize, bytes: usize) -> usize {
        self.bytes(denominator) / bytes
    }

    fn cells32(self, denominator: usize, bytes: usize) -> u32 {
        u32::try_from(self.cells(denominator, bytes)).unwrap_or(u32::MAX)
    }

    /// Ordinary semantic-session policy. Batches and GPU sweep allowances remain
    /// physical scheduling choices; mandatory CPU work has no chosen ceiling.
    #[must_use]
    pub const fn solve_config(self) -> SolveConfig {
        let closure = self.bytes(4);
        SolveConfig {
            backend: Backend::Cpu,
            grounder: Grounder::Auto,
            source_batching: SourceBatching::Independent,
            oracle: Oracle::Auto,
            search: SearchMethod::Regions,
            stats: false,
            constraints: zetesis_themelios::ConstraintCheckLimits {
                max_work: u64::MAX,
                max_substitutions: u64::MAX,
                max_scalar_bytes: usize::MAX,
            },
            models: 1,
            max_search_work: u64::MAX,
            max_search_decisions: u64::MAX,
            max_model_work: u64::MAX,
            max_model_bytes: self.bytes(32),
            max_projection_entries: self.cells(16, 16) as u64,
            // Trie coordinates have an independent checked u32 representation.
            max_projection_nodes: if self.cells(16, 16) > u32::MAX as usize {
                u32::MAX as usize
            } else {
                self.cells(16, 16)
            },
            max_projection_bytes: self.bytes(16),
            max_objective_work: u64::MAX,
            // Optional candidate pruning may stop without ending exact scoring.
            max_objective_bound_work: 10_000_000,
            max_objective_bindings: u64::MAX,
            max_objective_keys: self.cells(32, 32),
            max_objective_key_bytes: self.bytes(32),
            max_optimal_models: self.cells(32, 64),
            max_optimal_atoms: self.cells(32, size_of::<usize>()),
            max_optimal_bytes: self.bytes(32),
            batch_size: DEFAULT_BATCH_SIZE,
            workers: self.workers,
            completion_workers: NonZeroUsize::MIN,
            max_reduct_bytes: self.bytes(32) as u64,
            max_completion_scratch_bytes: self.bytes(8) as u64,
            max_candidates: u64::MAX,
            max_carrier_atoms: self.cells(32, 32),
            max_candidate_bytes: self.bytes(32),
            max_work: u64::MAX,
            max_closure_bytes: closure / self.workers.get(),
            max_closure_batch_bytes: closure,
            // These are dispatch bounds, not cumulative solve budgets.
            gpu_formula_work: 100_000_000,
            gpu_formula_rounds: 64,
            max_source_work: u64::MAX,
            max_atoms: self.cells(8, 32),
            max_substitutions: usize::MAX,
            max_ground_rules: self.cells(8, 64),
            max_batch_bytes: self.bytes(32) as u64,
        }
    }

    /// Source normalization keeps cumulative traffic as representationally
    /// checked counters. Retained templates and declaration metadata use memory.
    #[must_use]
    pub fn expansion_limits(self) -> ExpansionLimits {
        ExpansionLimits {
            max_constants: self.cells(16, 32),
            max_term_work: usize::MAX,
            max_templates: self.cells(8, 64),
            max_values: usize::MAX,
            max_family_bytes: self.bytes(8),
            max_scalar_bytes: usize::MAX,
            max_origin_locations: self.cells(16, 32),
            max_metadata_statements: self.cells(16, 32),
        }
    }

    /// Named formula/support capacities and mandatory traversal accounting.
    /// Optional key analysis keeps its bounded default effort and fail-open route.
    #[must_use]
    pub fn formula_limits(self) -> FormulaLimits {
        let theory = self.theory_limits();
        FormulaLimits {
            max_atom_storage_bytes: self.bytes(8),
            max_project_atoms: self.cells(32, 32),
            max_project_bytes: self.bytes(32),
            max_objective_presence_entries: self.cells(16, 32),
            max_objective_formula_atoms: theory.max_atoms,
            max_objective_formula_nodes: theory.max_nodes,
            max_objective_formula_operands: theory.max_operands,
            max_domain_values: self.cells(8, 32),
            max_assignment_values: self.cells(16, 32),
            max_generated_values: self.cells(8, 32),
            max_disjunction_elements: self.cells(16, 32),
            max_support_index_entries: self.cells(4, size_of::<usize>()),
            max_support_bytes: self.bytes(4),
            max_aggregate_cache_rows: self.cells(32, 64),
            max_aggregate_cache_key_bytes: self.bytes(32),
            max_aggregate_cache_elements: self.cells(32, 32),
            max_aggregate_cache_roots: self.cells(32, size_of::<usize>()),
            max_analysis_nodes: self.cells(8, 32),
            max_analysis_edges: self.cells(8, 2 * size_of::<usize>()),
            max_substitutions: u64::MAX,
            max_work: u64::MAX,
            max_support_rounds: u64::MAX,
            max_origin_locations: self.cells(16, 32),
            max_warnings: self.cells(16, 32),
            theory,
            aggregate: zetesis_ferraris::AggregateLimits {
                max_elements: self.cells(16, 32),
                max_nodes: theory.max_nodes,
                max_operands: theory.max_operands,
                max_work: u64::MAX,
                max_states: self.cells(16, 32),
                max_subsets: u64::MAX,
            },
            objective: self.objective_admission(),
            observation: self.observation_admission(),
            metadata_storage: zetesis_themelios::MetadataStorageLimits {
                max_bytes: self.bytes(32),
            },
            ..FormulaLimits::default()
        }
    }

    fn theory_limits(self) -> zetesis_ferraris::AdmissionLimits {
        zetesis_ferraris::AdmissionLimits {
            max_atoms: self.cells(8, 32),
            max_nodes: self.cells(4, size_of::<zetesis_ferraris::Node>()),
            max_roots: self.cells(16, size_of::<usize>()),
            max_operands: self.cells(4, size_of::<usize>()),
        }
    }

    fn core_limits(self) -> zetesis_core::AdmissionLimits {
        zetesis_core::AdmissionLimits {
            max_templates: self.cells(8, 64),
            max_predicate_arity: self.cells(16, size_of::<usize>()),
            max_variables_per_template: self.cells(16, 32),
            max_positive_body: self.cells(16, 32),
            max_domain_values: self.cells(8, 32),
            max_bytes: self.bytes(8),
        }
    }

    /// Parsed-source preflight. Syntax depth retains the existing guarded
    /// recursive-ingress bound; retained populations derive from memory.
    #[must_use]
    pub fn admission_options(self) -> AdmissionOptions {
        AdmissionOptions {
            max_source_bytes: self.bytes(8),
            max_syntax_nodes: self.cells(8, 16),
            max_body_elements: self.cells(16, 32),
            core_limits: self.core_limits(),
            ..AdmissionOptions::default()
        }
    }

    /// Canonical-program preflight uses the same source/core capacity shares.
    #[must_use]
    pub fn program_admission_options(self) -> ProgramAdmissionOptions {
        ProgramAdmissionOptions {
            max_nodes: self.cells(8, 16),
            max_body_elements: self.cells(16, 32),
            max_text_bytes: self.bytes(8),
            core_limits: self.core_limits(),
            ..ProgramAdmissionOptions::default()
        }
    }

    /// Original-file retention. Include depth retains the guarded traversal
    /// bound; file/root populations and bytes derive from the source share.
    #[must_use]
    pub fn bundle_limits(self) -> BundleLimits {
        BundleLimits {
            max_roots: self.cells(8, 64),
            max_files: self.cells(8, 64),
            max_file_bytes: self.bytes(8),
            max_total_bytes: self.bytes(8),
            ..BundleLimits::default()
        }
    }

    fn objective_admission(self) -> zetesis_objective::AdmissionLimits {
        zetesis_objective::AdmissionLimits {
            max_condition_node_bytes: self.bytes(16),
            max_bytes: self.bytes(16),
            max_templates: self.cells(16, 64),
            max_tuple_width: self.cells(16, size_of::<usize>()),
            max_variables_per_template: self.cells(16, 32),
            max_positive_body: self.cells(16, 32),
            max_predicate_arity: self.cells(16, size_of::<usize>()),
            max_filters: self.cells(16, 32),
            max_condition_nodes: self.cells(16, 32),
        }
    }

    fn observation_admission(self) -> observation::AdmissionLimits {
        observation::AdmissionLimits {
            max_directives: self.cells32(16, 64),
            max_nodes: self.cells32(16, 16),
            max_bytes: u32::try_from(self.bytes(16)).unwrap_or(u32::MAX),
            max_variables: self.cells32(16, 32),
            max_body_elements: self.cells32(16, 32),
            max_arity: self.cells32(16, size_of::<usize>()),
            max_origins: self.cells32(16, 32),
            ..observation::AdmissionLimits::default()
        }
    }

    /// Output evaluation with cumulative work/binding counters unrestricted by
    /// policy; retained values, construction scratch and record bytes use memory.
    #[must_use]
    pub fn observation_limits(self) -> observation::Limits {
        observation::Limits {
            max_work: u64::MAX,
            max_bindings: u64::MAX,
            max_terms: self.cells(32, 32),
            max_symbol_nodes: self.cells(32, 16),
            max_symbol_bytes: self.bytes(32),
            max_output_bytes: self.bytes(32),
            max_local_bytes: self.bytes(32),
            max_term_storage_bytes: self.bytes(32),
            ..observation::Limits::default()
        }
    }

    /// Explicit answer projection retains memory-bounded identity history and
    /// checked cumulative work without a selected operation ceiling.
    #[must_use]
    pub fn projection_limits(self) -> crate::ProjectionLimits {
        crate::ProjectionLimits {
            max_keys: self.cells(16, 32),
            max_bytes: self.bytes(16),
            max_work: u64::MAX,
        }
    }

    /// Maximum named capacity for one complete JSON record or output atom.
    #[must_use]
    pub const fn json_record_bytes(self) -> usize {
        self.bytes(32)
    }
}

impl Default for Resources {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests;
