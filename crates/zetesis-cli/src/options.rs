use clap::{Parser, Subcommand, ValueEnum};
use std::num::NonZeroUsize;
use std::path::PathBuf;

/// Execution policy. Explicit GPU requests require a real selected device.
/// General formulas use GPU propagation with exact native CPU residual search.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Backend {
    /// CPU formula search; closure may use GPU batches of 32 or more after its first seed.
    #[default]
    Auto,
    /// Source joins or static closure scans on an owned Rayon pool.
    Cpu,
    /// Exact integer GPU batches; currently requires eager static lowering.
    Gpu,
    /// Require a physical GPU using Metal.
    Metal,
    /// Require a physical GPU using Vulkan.
    Vulkan,
    /// Require a physical GPU using DirectX 12.
    Dx12,
    /// Require a physical GPU using OpenGL or OpenGL ES.
    Gl,
    /// Require an NVIDIA GPU through a compiled graphics API; this is not CUDA.
    Nvidia,
}

/// Materialization policy, independent of execution hardware.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Grounder {
    /// Preserve adaptive behavior: CPU starts lazy; GPU uses static lowering.
    #[default]
    Auto,
    /// Require source joins without materializing a complete ground rule store.
    /// Currently supported on CPU only; automatic hardware stays on CPU.
    Lazy,
    /// Materialize a bounded static program before checking on CPU or GPU.
    Eager,
}

impl Grounder {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Lazy => "lazy",
            Self::Eager => "eager",
        }
    }
}

/// Exact stable-model oracle selection, independent of language support.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Oracle {
    /// Select reduct closure, checked tight support, or general reduct checking.
    #[default]
    Auto,
    /// Require reduct closure with sparse gate candidates on CPU or static GPU batches.
    Closure,
    /// Require eager Ferraris search: CPU, or GPU propagation with exact CPU residuals.
    Countermodel,
}

/// Commands that do not read an answer-set program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// List compiled GPU APIs and detected adapters with their capabilities.
    Devices,
}

/// CLI configuration and the library driver's explicit resource settings.
#[derive(Clone, Debug, Parser)]
#[command(
    name = "zetesis",
    version,
    args_conflicts_with_subcommands = true,
    about = "Candidate-directed answer-set solving through the reduct"
)]
pub struct Options {
    /// Optional device inventory command; no source input is read for it.
    #[command(subcommand)]
    pub command: Option<Command>,
    /// First original input file, or `-` alone for standard input.
    #[arg(default_value = "-", value_name = "FILE")]
    pub input: PathBuf,
    /// Additional original input files, in argument order. Standard input
    /// cannot be combined with file roots in this source-bundle profile.
    #[arg(value_name = "FILE")]
    pub additional_inputs: Vec<PathBuf>,
    /// Execution policy. Explicit GPU requests fail if unavailable; auto may
    /// fall back to CPU with a reason on stderr. GPU support is enabled by default.
    #[arg(long, value_enum, default_value_t)]
    pub backend: Backend,
    /// Materialization policy, independent of --backend. Lazy currently requires
    /// CPU; eager is bounded by atom, substitution and ground-rule ceilings.
    #[arg(long, value_enum, default_value_t)]
    pub grounder: Grounder,
    /// Advanced oracle selection. Auto preserves stable-model semantics while
    /// selecting an applicable reduct procedure. Explicit hardware and grounder
    /// requests are always honored or refused.
    #[arg(long, value_enum, default_value_t)]
    pub oracle: Oracle,
    /// Report version, settings, completion, available counters and total driver
    /// elapsed time on stderr. Answer-set output on stdout is unchanged.
    #[arg(long)]
    pub stats: bool,
    /// Stream a versioned JSON document with full models, shown channels and coverage.
    #[arg(long)]
    pub json: bool,
    /// Color human headings and optimization metadata. Auto follows stdout
    /// terminal detection, `NO_COLOR` and `TERM`; JSON is always plain.
    #[arg(long, value_enum, default_value_t)]
    pub color: crate::ColorMode,
    /// Maximum JSON bytes per model record or terminal outcome; not an all-model buffer.
    #[arg(long, default_value_t = 8_388_608)]
    pub max_json_record_bytes: usize,
    /// Cumulative encoding, certificate and search operations for the formula oracle.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_search_work)]
    pub max_search_work: u64,
    /// Cumulative branch decisions for the countermodel oracle.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_search_decisions)]
    pub max_search_decisions: u64,
    /// Maximum term-evaluation/dependency steps in source expansion.
    #[arg(long, default_value_t = 1_048_576)]
    pub max_expansion_work: usize,
    /// Maximum output templates from source expansion.
    #[arg(long, default_value_t = 100_000)]
    pub max_expanded_templates: usize,
    /// Maximum scalar alternatives/emitted arguments in source expansion.
    #[arg(long, default_value_t = 1_000_000)]
    pub max_expansion_values: usize,
    /// Maximum models to display; 0 requests all. Without objectives this stops
    /// search early. Optimization seeks exhaustion and then displays this many
    /// tied optima; interrupted runs may display incumbents without proving an optimum.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.models)]
    pub models: usize,
    /// Cumulative objective evaluation work across all verified stable models.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_objective_work)]
    pub max_objective_work: u64,
    /// Observation evaluation/rendering work per displayed full model.
    #[arg(long, default_value_t = 1_000_000)]
    pub max_observation_work: u64,
    /// Complete positive bindings per observation operation.
    #[arg(long, default_value_t = 100_000)]
    pub max_observation_bindings: u64,
    /// Distinct enabled terms per displayed model.
    #[arg(long, default_value_t = 65_536)]
    pub max_observation_terms: usize,
    /// Retained observation payload and complete Answer record bytes.
    #[arg(long, default_value_t = 8_388_608)]
    pub max_observation_bytes: usize,
    /// Cumulative work for optional incumbent candidate bounds. Zero disables
    /// pruning; a refused bound preserves ordinary exact answer-set search.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_objective_bound_work)]
    pub max_objective_bound_work: u64,
    /// Maximum complete objective bindings evaluated per stable model.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_objective_bindings)]
    pub max_objective_bindings: u64,
    /// Maximum distinct objective contribution keys retained per stable model.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_objective_keys)]
    pub max_objective_keys: usize,
    /// Maximum encoded objective contribution bytes per stable model.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_objective_key_bytes)]
    pub max_objective_key_bytes: usize,
    /// Maximum tied incumbent models retained while proving an optimum.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_optimal_models)]
    pub max_optimal_models: usize,
    /// Maximum atoms across retained incumbent models, before display selection.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_optimal_atoms)]
    pub max_optimal_atoms: usize,
    /// Maximum retained model payload bytes, excluding allocator overhead.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_optimal_bytes)]
    pub max_optimal_bytes: usize,
    /// Maximum batched formula candidates; closure batches follow its first seed.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.batch_size)]
    pub batch_size: NonZeroUsize,
    /// Closure CPU worker count. Formula completion has a separate worker setting.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.workers)]
    pub workers: NonZeroUsize,
    /// Exact formula completion workers; one retains the scalar CPU cursor.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.completion_workers)]
    pub completion_workers: NonZeroUsize,
    /// Maximum logical scratch bytes for completion batches, excluding the scalar
    /// cursor, allocator overhead, thread stacks and GPU storage.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_completion_scratch_bytes)]
    pub max_completion_scratch_bytes: u64,
    /// Maximum candidate seeds; reaching a limit leaves search incomplete.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_candidates)]
    pub max_candidates: u64,
    /// Maximum gate tuples retained by the incremental candidate cursor.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_carrier_atoms)]
    pub max_carrier_atoms: usize,
    /// Maximum charged oracle operations per CPU candidate; lazy joins and eager
    /// scans charge different operations.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_work)]
    pub max_work: u64,
    /// Maximum derived CPU atoms and materialized eager atoms.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_atoms)]
    pub max_atoms: usize,
    /// Maximum bytes in each original file or standard input before parsing.
    #[arg(long, default_value_t = 1_048_576)]
    pub max_source_bytes: usize,
    /// Maximum explicit input root occurrences, including repeated filenames.
    #[arg(long, default_value_t = 256)]
    pub max_source_roots: usize,
    /// Maximum distinct original files across every input/include graph.
    #[arg(long, default_value_t = 256)]
    pub max_source_files: usize,
    /// Maximum original bytes retained across distinct input/include files.
    #[arg(long, default_value_t = 8_388_608)]
    pub max_total_source_bytes: usize,
    /// Maximum include edges from any explicit input root.
    #[arg(long, default_value_t = 32)]
    pub max_include_depth: usize,
    /// Maximum source substitutions inspected during eager CPU/GPU lowering.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_substitutions)]
    pub max_substitutions: usize,
    /// Maximum rules retained during eager CPU/GPU lowering.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_ground_rules)]
    pub max_ground_rules: usize,
    /// Maximum accounted GPU batch transport bytes, excluding driver overhead.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_batch_bytes)]
    pub max_batch_bytes: u64,
}
