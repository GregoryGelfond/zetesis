use clap::{Parser, Subcommand, ValueEnum};
use std::num::NonZeroUsize;
use std::path::PathBuf;

use crate::{Backend, Grounder, Oracle, SourceBatching};

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
    disable_help_flag = true,
    arg(clap::Arg::new("help").short('h').long("help").global(true).action(clap::ArgAction::HelpShort).help("Show everyday solving options")),
    arg(clap::Arg::new("help-all").long("help-all").global(true).action(clap::ArgAction::HelpLong).help("Show all oracle, worker, batch and resource options")),
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
    /// Additional original input files, in argument order.
    ///
    /// Standard input
    /// cannot be combined with file roots in this source-bundle profile.
    #[arg(value_name = "FILE")]
    pub additional_inputs: Vec<PathBuf>,
    /// Execution backend.
    ///
    /// Explicit GPU requests fail if unavailable; auto may
    /// fall back to CPU with a reason on stderr. GPU support is enabled by default.
    #[arg(long, value_enum, default_value_t)]
    pub backend: Backend,
    /// Grounding mode, independent of execution backend.
    ///
    /// Lazy uses source joins for the relational profile on CPU or GPU,
    /// including automatic hardware selection. General formulas require eager grounding, bounded by atom,
    /// substitution and ground-rule ceilings.
    #[arg(long, value_enum, default_value_t)]
    pub grounder: Grounder,
    /// Advanced relational CPU source batching. Union/worlds require lazy or
    /// auto grounding and select CPU when backend is auto. A stopped shared
    /// batch publishes no candidate checks; independent remains the default.
    #[arg(long, value_enum, default_value_t, hide_short_help = true)]
    pub source_batching: SourceBatching,
    /// Advanced oracle selection. Auto preserves stable-model semantics while
    /// selecting an applicable reduct procedure. Explicit hardware and grounder
    /// requests are always honored or refused.
    #[arg(long, value_enum, default_value_t, hide_short_help = true)]
    pub oracle: Oracle,
    /// Print grounding, solving and execution statistics on stderr.
    ///
    /// Report version, settings, completion, available counters and total driver
    /// elapsed time on stderr. Answer-set output on stdout is unchanged.
    #[arg(long)]
    pub stats: bool,
    /// Stream a versioned JSON document with full models, shown channels and coverage.
    #[arg(long)]
    pub json: bool,
    /// Style human headings and solve metadata.
    ///
    /// Auto follows each stream's terminal capability, `NO_COLOR` and `TERM`.
    /// JSON is always plain.
    #[arg(long, value_enum, default_value_t)]
    pub color: crate::ColorMode,
    /// Maximum JSON bytes per model record or terminal outcome; not an all-model buffer.
    #[arg(long, default_value_t = 8_388_608, hide_short_help = true)]
    pub max_json_record_bytes: usize,
    /// Cumulative encoding, certificate and search operations for the formula oracle.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_search_work, hide_short_help = true)]
    pub max_search_work: u64,
    /// Cumulative branch decisions for the countermodel oracle.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_search_decisions, hide_short_help = true)]
    pub max_search_decisions: u64,
    /// Maximum source-expansion and eager formula-grounding work.
    ///
    /// Formula grounding includes typed column construction and lookup.
    #[arg(long, default_value_t = 1_048_576, hide_short_help = true)]
    pub max_expansion_work: usize,
    /// Maximum authored bytes for eager formula support views, indexes and queries.
    ///
    /// Source atoms, allocator/tree overhead and other grounding state are excluded.
    #[arg(long, default_value_t = zetesis_themelios::FormulaLimits::default().max_support_bytes, hide_short_help = true)]
    pub max_support_bytes: usize,
    /// Maximum output templates from source expansion.
    #[arg(long, default_value_t = 100_000, hide_short_help = true)]
    pub max_expanded_templates: usize,
    /// Maximum scalar alternatives/emitted arguments in source expansion.
    #[arg(long, default_value_t = 1_000_000, hide_short_help = true)]
    pub max_expansion_values: usize,
    /// Maximum models to display; 0 requests all.
    ///
    /// Without objectives this stops
    /// search early. Optimization seeks exhaustion and then displays this many
    /// tied optima; interrupted runs may display incumbents without proving an optimum.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.models, display_order = 0)]
    pub models: usize,
    /// Cumulative objective evaluation work across all verified stable models.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_objective_work, hide_short_help = true)]
    pub max_objective_work: u64,
    /// Observation evaluation/rendering work per displayed full model.
    #[arg(long, default_value_t = 1_000_000, hide_short_help = true)]
    pub max_observation_work: u64,
    /// Complete positive bindings per observation operation.
    #[arg(long, default_value_t = 100_000, hide_short_help = true)]
    pub max_observation_bindings: u64,
    /// Distinct enabled terms per displayed model.
    #[arg(long, default_value_t = 65_536, hide_short_help = true)]
    pub max_observation_terms: usize,
    /// Retained observation payload and complete Answer record bytes.
    #[arg(long, default_value_t = 8_388_608, hide_short_help = true)]
    pub max_observation_bytes: usize,
    /// Cumulative work for optional incumbent candidate bounds. Zero disables
    /// pruning; a refused bound preserves ordinary exact answer-set search.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_objective_bound_work, hide_short_help = true)]
    pub max_objective_bound_work: u64,
    /// Maximum complete objective bindings evaluated per stable model.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_objective_bindings, hide_short_help = true)]
    pub max_objective_bindings: u64,
    /// Maximum distinct objective contribution keys retained per stable model.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_objective_keys, hide_short_help = true)]
    pub max_objective_keys: usize,
    /// Maximum encoded objective contribution bytes per stable model.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_objective_key_bytes, hide_short_help = true)]
    pub max_objective_key_bytes: usize,
    /// Maximum tied incumbent models retained while proving an optimum.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_optimal_models, hide_short_help = true)]
    pub max_optimal_models: usize,
    /// Maximum atoms across retained incumbent models, before display selection.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_optimal_atoms, hide_short_help = true)]
    pub max_optimal_atoms: usize,
    /// Maximum retained model payload bytes, excluding allocator overhead.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_optimal_bytes, hide_short_help = true)]
    pub max_optimal_bytes: usize,
    /// Maximum batched formula candidates; closure batches follow its first seed.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.batch_size, hide_short_help = true)]
    pub batch_size: NonZeroUsize,
    /// Closure CPU worker count. Formula completion has a separate worker setting.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.workers, hide_short_help = true)]
    pub workers: NonZeroUsize,
    /// Exact formula completion workers; one retains the scalar CPU cursor.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.completion_workers, hide_short_help = true)]
    pub completion_workers: NonZeroUsize,
    /// Maximum logical scratch bytes for completion batches, excluding the scalar
    /// cursor, allocator overhead, thread stacks and GPU storage.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_completion_scratch_bytes, hide_short_help = true)]
    pub max_completion_scratch_bytes: u64,
    /// Maximum candidate seeds; reaching a limit leaves search incomplete.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_candidates, hide_short_help = true)]
    pub max_candidates: u64,
    /// Maximum gate tuples retained by the incremental candidate cursor.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_carrier_atoms, hide_short_help = true)]
    pub max_carrier_atoms: usize,
    /// Maximum charged oracle operations per CPU candidate, or shared source
    /// operations per lazy GPU batch. Shared CPU worlds count record visits
    /// plus antecedent tests. Join/copy and eager scan units differ.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_work, hide_short_help = true)]
    pub max_work: u64,
    /// Collective source work per shared CPU batch. Separate from max-work,
    /// which bounds record visits plus antecedent tests per shared CPU world.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_source_work, hide_short_help = true)]
    pub max_source_work: u64,
    /// Maximum derived independent CPU atoms, shared CPU/GPU catalog atoms or
    /// eager atoms. A shared catalog cap is collective across the batch.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_atoms, hide_short_help = true)]
    pub max_atoms: usize,
    /// Maximum bytes in each original file or standard input before parsing.
    #[arg(long, default_value_t = 1_048_576, hide_short_help = true)]
    pub max_source_bytes: usize,
    /// Maximum explicit input root occurrences, including repeated filenames.
    #[arg(long, default_value_t = 256, hide_short_help = true)]
    pub max_source_roots: usize,
    /// Maximum distinct original files across every input/include graph.
    #[arg(long, default_value_t = 256, hide_short_help = true)]
    pub max_source_files: usize,
    /// Maximum original bytes retained across distinct input/include files.
    #[arg(long, default_value_t = 8_388_608, hide_short_help = true)]
    pub max_total_source_bytes: usize,
    /// Maximum include edges from any explicit input root.
    #[arg(long, default_value_t = 32, hide_short_help = true)]
    pub max_include_depth: usize,
    /// Maximum source substitutions inspected during eager CPU/GPU lowering.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_substitutions, hide_short_help = true)]
    pub max_substitutions: usize,
    /// Maximum rules retained during eager CPU/GPU lowering.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_ground_rules, hide_short_help = true)]
    pub max_ground_rules: usize,
    /// Maximum accounted batch bytes, excluding allocator/driver overhead.
    /// Lazy GPU reserves half for source state and half for transient transport.
    /// Shared CPU rounds use the full allowance for source/world state.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_batch_bytes, hide_short_help = true)]
    pub max_batch_bytes: u64,
}

impl From<&Options> for crate::SolveConfig {
    fn from(options: &Options) -> Self {
        Self {
            backend: options.backend,
            grounder: options.grounder,
            source_batching: options.source_batching,
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
            max_source_work: options.max_source_work,
            max_atoms: options.max_atoms,
            max_substitutions: options.max_substitutions,
            max_ground_rules: options.max_ground_rules,
            max_batch_bytes: options.max_batch_bytes,
        }
    }
}

impl ValueEnum for Backend {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self::Auto,
            Self::Cpu,
            Self::Gpu,
            Self::Metal,
            Self::Vulkan,
            Self::Dx12,
            Self::Gl,
            Self::Nvidia,
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Auto => clap::builder::PossibleValue::new("auto").help("CPU formula search; closure may use GPU batches of 32 or more after its first seed."),
            Self::Cpu => clap::builder::PossibleValue::new("cpu").help("Source joins or static closure scans on an owned Rayon pool."),
            Self::Gpu => clap::builder::PossibleValue::new("gpu").help("Exact integer GPU batches, including explicit lazy relational execution."),
            Self::Metal => clap::builder::PossibleValue::new("metal").help("Require a physical GPU using Metal."),
            Self::Vulkan => clap::builder::PossibleValue::new("vulkan").help("Require a physical GPU using Vulkan."),
            Self::Dx12 => clap::builder::PossibleValue::new("dx12").help("Require a physical GPU using DirectX 12."),
            Self::Gl => clap::builder::PossibleValue::new("gl").help("Require a physical GPU using OpenGL or OpenGL ES."),
            Self::Nvidia => clap::builder::PossibleValue::new("nvidia").help("Require an NVIDIA GPU through a compiled graphics API; this is not CUDA."),
        })
    }
}

impl ValueEnum for Grounder {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Auto, Self::Lazy, Self::Eager]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Auto => clap::builder::PossibleValue::new("auto").help("Prefer lazy source grounding where admitted, independently of hardware."),
            Self::Lazy => clap::builder::PossibleValue::new("lazy").help("Require source joins without materializing a complete ground rule store. Explicit GPU requests use immutable relational rounds; Auto may discover a device after the first seed."),
            Self::Eager => clap::builder::PossibleValue::new("eager").help("Materialize a bounded static program before checking on CPU or GPU."),
        })
    }
}

impl ValueEnum for SourceBatching {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Independent, Self::Union, Self::Worlds]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Independent => clap::builder::PossibleValue::new("independent")
                .help("Each candidate owns an independent relational join traversal."),
            Self::Union => clap::builder::PossibleValue::new("union")
                .help("Share the union carrier; evaluate each frozen candidate on Rayon."),
            Self::Worlds => clap::builder::PossibleValue::new("worlds")
                .help("Prune source prefixes with per-world membership; evaluate on Rayon."),
        })
    }
}

impl ValueEnum for Oracle {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Auto, Self::Closure, Self::Countermodel]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Auto => clap::builder::PossibleValue::new("auto")
                .help("Select reduct closure, checked tight support, or general reduct checking."),
            Self::Closure => clap::builder::PossibleValue::new("closure").help(
                "Require reduct closure with sparse gate candidates on CPU or static GPU batches.",
            ),
            Self::Countermodel => clap::builder::PossibleValue::new("countermodel").help(
                "Require eager Ferraris search: CPU, or GPU propagation with exact CPU residuals.",
            ),
        })
    }
}
