use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{Parser, Subcommand};
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
    /// Auto retains CPU execution. Explicit GPU requests fail if unavailable;
    /// device failure does not silently retry on CPU. GPU support is enabled by default.
    #[arg(long, value_parser = backend_parser(), default_value = "auto")]
    pub backend: Backend,
    /// Grounding mode, independent of execution backend.
    ///
    /// Lazy uses source joins for the relational profile on CPU or GPU.
    /// General formulas require eager grounding, bounded by atom, substitution
    /// and ground-rule ceilings.
    #[arg(long, value_parser = grounder_parser(), default_value = "auto")]
    pub grounder: Grounder,
    /// Positive joins during eager formula grounding.
    ///
    /// Table reuses support masks for eligible flat patterns. Relational source
    /// grounding and support-growth rounds retain indexed joins. Preparation
    /// consumes the existing support-storage and grounding-work limits.
    #[arg(long, value_parser = formula_joins_parser(), default_value = "indexed", hide_short_help = true)]
    pub formula_joins: zetesis_themelios::JoinStrategy,
    /// Advanced relational CPU source batching. Union/worlds require lazy or
    /// auto grounding and CPU execution. A stopped shared
    /// batch publishes no candidate checks; independent remains the default.
    #[arg(long, value_parser = source_batching_parser(), default_value = "independent", hide_short_help = true)]
    pub source_batching: SourceBatching,
    /// Advanced oracle selection. Auto preserves answer-set semantics while
    /// selecting an applicable reduct procedure. Explicit hardware and grounder
    /// requests are always honored or refused.
    #[arg(long, value_parser = oracle_parser(), default_value = "auto", hide_short_help = true)]
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
    /// Cooperative process deadline after input loading, in whole seconds.
    ///
    /// Zero requests an immediate stop. No deadline is imposed when omitted.
    /// Checked work boundaries observe the deadline; blocking source I/O,
    /// frontend operations and a running GPU kernel cannot be preempted.
    /// Library callers supply their own Control instead of this process option.
    #[arg(long, value_name = "SECONDS")]
    pub time_limit: Option<u64>,
    /// Maximum JSON bytes per model record or terminal outcome; not an all-model buffer.
    #[arg(long, default_value_t = 8_388_608, hide_short_help = true)]
    pub max_json_record_bytes: usize,
    /// Cumulative candidate restriction preparation/traversal or formula search work.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_search_work, hide_short_help = true)]
    pub max_search_work: u64,
    /// Cumulative branch decisions for the countermodel oracle.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_search_decisions, hide_short_help = true)]
    pub max_search_decisions: u64,
    /// Maximum distinct projections retained during formula candidate enumeration.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_projection_entries, hide_short_help = true)]
    pub max_projection_entries: u64,
    /// Maximum logical nodes in retained formula projection history.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_projection_nodes, hide_short_help = true)]
    pub max_projection_nodes: usize,
    /// Maximum named projection-history capacity, including growth overlap.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_projection_bytes, hide_short_help = true)]
    pub max_projection_bytes: usize,
    /// Override source-expansion and eager formula-grounding work ceilings.
    ///
    /// Omission preserves each library default: 1,048,576 source-term operations
    /// and 10,000,000 formula-grounding operations. An explicit value applies
    /// independently to both counters. Formula work includes checked typed
    /// lookup, index construction, copying and commit work.
    #[arg(long, hide_short_help = true)]
    pub max_expansion_work: Option<usize>,
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
    /// Override distinct source-domain values in each selected admission profile.
    /// Omission preserves the relational and formula library defaults.
    #[arg(long, hide_short_help = true)]
    pub max_domain_values: Option<usize>,
    /// Candidate values in one eager formula assignment, range or presence subset.
    #[arg(long, default_value_t = zetesis_themelios::FormulaLimits::default().max_assignment_values, hide_short_help = true)]
    pub max_assignment_values: usize,
    /// Distinct generated binding values across eager formula grounding.
    #[arg(long, default_value_t = zetesis_themelios::FormulaLimits::default().max_generated_values, hide_short_help = true)]
    pub max_generated_values: usize,
    /// Complete eager possible-support rounds, including the final no-change round.
    #[arg(long, default_value_t = zetesis_themelios::FormulaLimits::default().max_support_rounds, hide_short_help = true)]
    pub max_support_rounds: u64,
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
    /// Maximum retained canonical bytes, counting the whole atom catalog per model.
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
    /// Maximum query and transient/result scratch bytes, excluding the
    /// scalar cursor, allocator/table overhead, thread stacks and GPU storage.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_completion_scratch_bytes, hide_short_help = true)]
    pub max_completion_scratch_bytes: u64,
    /// Maximum candidate seeds; reaching a limit leaves search incomplete.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_candidates, hide_short_help = true)]
    pub max_candidates: u64,
    /// Maximum copied payload for necessary candidate restrictions, including
    /// temporary templates. Allocator and index overhead are excluded.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_candidate_bytes, hide_short_help = true)]
    pub max_candidate_bytes: usize,
    /// Maximum gate tuples retained by the incremental candidate cursor.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_carrier_atoms, hide_short_help = true)]
    pub max_carrier_atoms: usize,
    /// Maximum charged oracle operations per CPU candidate, or shared source
    /// operations per lazy GPU batch. Shared CPU worlds count record visits
    /// plus antecedent tests. Join/copy and eager scan units differ.
    /// Also bounds each independent formula verification or certified candidate
    /// check; formula search and GPU propagation have separate limits.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_work, hide_short_help = true)]
    pub max_work: u64,
    /// Maximum named storage bytes per independent lazy CPU closure.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_closure_bytes, hide_short_help = true)]
    pub max_closure_bytes: usize,
    /// Collective independent CPU preparation, idle cache and assigned closure
    /// storage allowance. Also bounds immutable query preparation bytes.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_closure_batch_bytes, hide_short_help = true)]
    pub max_closure_batch_bytes: usize,
    /// Device propagation work per formula candidate, independent of CPU work.
    /// A budget below mandatory setup work refuses before device submission.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.gpu_formula_work, hide_short_help = true)]
    pub gpu_formula_work: u32,
    /// Device propagation sweeps per formula candidate. Zero keeps original-truth
    /// setup and sends undecided candidates to exact CPU residual search.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.gpu_formula_rounds, hide_short_help = true)]
    pub gpu_formula_rounds: u32,
    /// Shared CPU source work per batch or independent CPU query preparation
    /// work. Candidate/world execution has the separate max-work allowance.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_source_work, hide_short_help = true)]
    pub max_source_work: u64,
    /// Maximum derived independent CPU atoms, shared CPU/GPU catalog atoms or
    /// eager atoms. A shared catalog cap is collective across the batch.
    /// Also sets the eager formula atom ceiling.
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
    /// Maximum substitutions inspected in eager lowering or formula admission.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_substitutions, hide_short_help = true)]
    pub max_substitutions: usize,
    /// Maximum rules retained during eager CPU/GPU lowering.
    /// Also sets the eager formula theory-root ceiling.
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
            max_projection_entries: options.max_projection_entries,
            max_projection_nodes: options.max_projection_nodes,
            max_projection_bytes: options.max_projection_bytes,
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
            max_candidate_bytes: options.max_candidate_bytes,
            max_carrier_atoms: options.max_carrier_atoms,
            max_work: options.max_work,
            max_closure_bytes: options.max_closure_bytes,
            max_closure_batch_bytes: options.max_closure_batch_bytes,
            gpu_formula_work: options.gpu_formula_work,
            gpu_formula_rounds: options.gpu_formula_rounds,
            max_source_work: options.max_source_work,
            max_atoms: options.max_atoms,
            max_substitutions: options.max_substitutions,
            max_ground_rules: options.max_ground_rules,
            max_batch_bytes: options.max_batch_bytes,
        }
    }
}

// The CLI owns each spelling/help mapping, while the parsed values remain the
// library's policies. Clap validates against the same table before mapping, so
// every accepted value has a typed entry. Case-insensitive lookup is safe here:
// PossibleValuesParser already enforces the argument's case-sensitivity policy.
fn policy_parser<T: Clone + Send + Sync + 'static, const N: usize>(
    choices: [(T, PossibleValue); N],
) -> impl TypedValueParser<Value = T> {
    PossibleValuesParser::new(choices.iter().map(|(_, value)| value.clone())).map(move |value| {
        choices
            .iter()
            .find(|(_, possible)| possible.matches(&value, true))
            .expect("clap validated this value against the same choice table")
            .0
            .clone()
    })
}

fn backend_parser() -> impl TypedValueParser<Value = Backend> {
    policy_parser([
        (
            Backend::Auto,
            PossibleValue::new(Backend::Auto.label())
                .help("CPU execution until a measured GPU crossover supports automatic selection."),
        ),
        (
            Backend::Cpu,
            PossibleValue::new(Backend::Cpu.label())
                .help("Source joins or static closure scans on an owned Rayon pool."),
        ),
        (
            Backend::Gpu,
            PossibleValue::new(Backend::Gpu.label())
                .help("Exact integer GPU batches, including explicit lazy relational execution."),
        ),
        (
            Backend::Metal,
            PossibleValue::new(Backend::Metal.label()).help("Require a physical GPU using Metal."),
        ),
        (
            Backend::Vulkan,
            PossibleValue::new(Backend::Vulkan.label())
                .help("Require a physical GPU using Vulkan."),
        ),
        (
            Backend::Dx12,
            PossibleValue::new(Backend::Dx12.label())
                .help("Require a physical GPU using DirectX 12."),
        ),
        (
            Backend::Gl,
            PossibleValue::new(Backend::Gl.label())
                .help("Require a physical GPU using OpenGL or OpenGL ES."),
        ),
        (
            Backend::Nvidia,
            PossibleValue::new(Backend::Nvidia.label())
                .help("Require an NVIDIA GPU through a compiled graphics API; this is not CUDA."),
        ),
    ])
}

fn grounder_parser() -> impl TypedValueParser<Value = Grounder> {
    policy_parser([
        (Grounder::Auto, PossibleValue::new(Grounder::Auto.label()).help("Prefer lazy source grounding where admitted, independently of hardware.")),
        (Grounder::Lazy, PossibleValue::new(Grounder::Lazy.label()).help("Require source joins without materializing a complete ground rule store. Explicit GPU requests use immutable relational rounds; Auto hardware selection retains CPU.")),
        (Grounder::Eager, PossibleValue::new(Grounder::Eager.label()).help("Materialize a bounded static program before checking on CPU or GPU.")),
    ])
}

fn formula_joins_parser() -> impl TypedValueParser<Value = zetesis_themelios::JoinStrategy> {
    use zetesis_themelios::JoinStrategy;
    policy_parser([
        (
            JoinStrategy::Indexed,
            PossibleValue::new("indexed").help("Probe the shortest matching value posting."),
        ),
        (
            JoinStrategy::Table,
            PossibleValue::new("table")
                .help("Reuse finite-table masks over completed possible support."),
        ),
    ])
}

fn source_batching_parser() -> impl TypedValueParser<Value = SourceBatching> {
    policy_parser([
        (
            SourceBatching::Independent,
            PossibleValue::new(SourceBatching::Independent.label())
                .help("Each candidate owns an independent relational join traversal."),
        ),
        (
            SourceBatching::Union,
            PossibleValue::new(SourceBatching::Union.label())
                .help("Share the union carrier; evaluate each frozen candidate on Rayon."),
        ),
        (
            SourceBatching::Worlds,
            PossibleValue::new(SourceBatching::Worlds.label())
                .help("Prune source prefixes with per-world membership; evaluate on Rayon."),
        ),
    ])
}

fn oracle_parser() -> impl TypedValueParser<Value = Oracle> {
    policy_parser([
        (
            Oracle::Auto,
            PossibleValue::new(Oracle::Auto.label())
                .help("Select reduct closure, checked tight support, or general reduct checking."),
        ),
        (
            Oracle::Closure,
            PossibleValue::new(Oracle::Closure.label()).help(
                "Require reduct closure with sparse gate candidates on CPU or static GPU batches.",
            ),
        ),
        (
            Oracle::Countermodel,
            PossibleValue::new(Oracle::Countermodel.label()).help(
                "Require eager Ferraris search: CPU, or GPU propagation with exact CPU residuals.",
            ),
        ),
    ])
}
