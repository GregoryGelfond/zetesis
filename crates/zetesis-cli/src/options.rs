use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{Parser, Subcommand};
use std::num::NonZeroUsize;
use std::path::PathBuf;

use crate::{Backend, Grounder, Oracle, SearchMethod, SourceBatching};

mod values;

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
    about = "An answer-set solver"
)]
pub struct Options {
    /// Statistics presentation selected by the invocation adapter.
    /// Legacy library/parser callers retain the line-oriented record view.
    #[arg(skip)]
    pub statistics_view: crate::StatisticsView,
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
    /// Execution backend: cpu (the default), gpu, metal or vulkan.
    ///
    /// `gpu` uses the platform's native API: Metal on macOS, Vulkan elsewhere.
    /// An unavailable GPU fails; device failure does not silently retry on CPU.
    #[arg(long, value_parser = zetesis_backend::BackendParser, default_value = "cpu")]
    pub backend: Backend,
    /// Grounding mode, independent of execution backend.
    ///
    /// Relational lazy grounding uses source joins on CPU or GPU. For formula
    /// inputs, lazy CPU grounding retains an eager producer core and streams
    /// eligible constraints; objectives and table joins are refused. Formula
    /// GPU execution requires eager materialization. Grounding limits still apply.
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
    /// Advanced formula search method, for proposing candidates and for the
    /// reduct's proper-subset query alike. Regions, the default, narrow the
    /// candidate space and the reduct's subsets by the theory's readings;
    /// clauses is the classical search over a clause form, with its own
    /// batched completion, `--completion-workers` and scratch ceiling. The
    /// reduct decides membership either way.
    #[arg(long, value_parser = search_parser(), default_value = "regions", hide_short_help = true)]
    pub search: SearchMethod,
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
    /// Cooperative process deadline after input loading: seconds, or a whole number with s, m or h.
    ///
    /// Zero requests an immediate stop. No deadline is imposed when omitted.
    /// A timer thread marks the deadline and checked work boundaries observe
    /// the mark as they observe cancellation, without reading the clock, so
    /// an unreached deadline costs nothing measurable. Blocking source I/O,
    /// frontend operations and a running GPU kernel cannot be preempted.
    /// Library callers supply their own Cancellation instead of this process option.
    #[arg(long, value_name = "DURATION", value_parser = values::seconds)]
    pub time_limit: Option<u64>,
    /// Memory allowance in bytes, or a whole number with KiB, MiB, GiB or TiB.
    ///
    /// The session's byte ceilings
    /// are the shares of a two-gibibyte allowance; each one not given (the
    /// projection, objective key, optimal, reduct, completion scratch,
    /// candidate, closure, closure batch and batch bytes) is its library
    /// default scaled by this allowance over two gibibytes, so a larger
    /// host admits larger problems before one refuses. The admission and
    /// output ceilings (the source, expansion, support, JSON record and
    /// observation bytes) keep their fixed defaults. The default allowance
    /// is half of the host's physical memory and at least two gibibytes, or
    /// two gibibytes when the host does not report its memory. Work, count
    /// and structural ceilings are not memory and do not scale. The
    /// ceilings bound named storage, not resident memory.
    #[arg(long, visible_alias = "memory-budget", value_name = "SIZE", value_parser = values::bytes, default_value_t = host_memory_allowance(), hide_short_help = true)]
    pub memory: u64,
    /// Maximum encoded JSON bytes per model record or terminal outcome; not an all-model buffer.
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
    /// Maximum reserved projection-history capacity, including growth overlap;
    /// capacity is counted before it is written, so this exceeds resident bytes.
    /// Omitted, it is the library default scaled by `--memory`.
    #[arg(long, hide_short_help = true)]
    pub max_projection_bytes: Option<usize>,
    /// Override source-expansion and cumulative formula-work ceilings.
    ///
    /// Omission preserves each library default: 1,048,576 source-term operations
    /// and 10,000,000 formula-grounding operations. An explicit value applies
    /// independently to both counters. Formula work includes admission, checked
    /// lookups, index construction and reconstruction of deferred definitions.
    #[arg(long, hide_short_help = true)]
    pub max_expansion_work: Option<usize>,
    /// Maximum named storage for formula support and reconstruction.
    ///
    /// Includes shared canonical payload, indexes, query buffers and growth
    /// overlap. Other grounding state and allocator overhead remain separate;
    /// this is not a process-memory limit.
    #[arg(long, default_value_t = zetesis_themelios::FormulaLimits::default().max_support_bytes, hide_short_help = true)]
    pub max_support_bytes: usize,
    /// Maximum output templates from source expansion.
    #[arg(long, default_value_t = 100_000, hide_short_help = true)]
    pub max_expanded_templates: usize,
    /// Maximum scalar alternatives/emitted arguments in source expansion.
    #[arg(long, default_value_t = 1_000_000, hide_short_help = true)]
    pub max_expansion_values: usize,
    /// Cumulative source-expansion bytes for copied scalar payload, plans and
    /// checked construction scratch. Structural captures borrow canonical terms
    /// but charge their delta cells. ID-only binding copies add no payload charge.
    /// This is separate from live canonical storage and process memory.
    #[arg(long, default_value_t = zetesis_themelios::ExpansionLimits::default().max_scalar_bytes, hide_short_help = true)]
    pub max_expansion_bytes: usize,
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
    /// Cumulative work preparing atom order and constructing verified formula models.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_model_work, hide_short_help = true)]
    pub max_model_work: u64,
    /// Maximum reserved capacity for prepared atom ranks and active model-construction metadata.
    /// Omitted, it is the library default scaled by `--memory`.
    #[arg(long, hide_short_help = true)]
    pub max_model_bytes: Option<usize>,
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
    /// Retained observation payload and complete Answer record bytes, counted as
    /// canonical text rather than as capacity.
    #[arg(long, default_value_t = 8_388_608, hide_short_help = true)]
    pub max_observation_bytes: usize,
    /// Cumulative work for optional incumbent bounds, excluding shared objective
    /// preparation. Zero disables pruning; prepared scoring remains available.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_objective_bound_work, hide_short_help = true)]
    pub max_objective_bound_work: u64,
    /// Maximum complete objective bindings evaluated per stable model.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_objective_bindings, hide_short_help = true)]
    pub max_objective_bindings: u64,
    /// Maximum distinct objective contribution keys retained per stable model.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_objective_keys, hide_short_help = true)]
    pub max_objective_keys: usize,
    /// Maximum canonical encoded objective contribution bytes per stable model.
    /// Omitted, it is the library default scaled by `--memory`.
    #[arg(long, hide_short_help = true)]
    pub max_objective_key_bytes: Option<usize>,
    /// Maximum tied incumbent models retained while proving an optimum.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_optimal_models, hide_short_help = true)]
    pub max_optimal_models: usize,
    /// Maximum atoms across retained incumbent models, before display selection.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_optimal_atoms, hide_short_help = true)]
    pub max_optimal_atoms: usize,
    /// Maximum canonical incumbent bytes: distinct catalogs, selections and one score.
    /// Omitted, it is the library default scaled by `--memory`.
    #[arg(long, hide_short_help = true)]
    pub max_optimal_bytes: Option<usize>,
    /// Maximum candidates per batch: the clause search's completion batches
    /// and the leaves a device checks; closure batches follow its first seed.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.batch_size, hide_short_help = true)]
    pub batch_size: NonZeroUsize,
    /// Threads for candidate search; auto uses the host's available parallelism.
    ///
    /// The closure route's pool and the region tree's walkers use this count.
    /// Auto falls back to one when the host does not report its parallelism.
    /// Each closure worker is admitted at
    /// the per-closure allowance, so workers × max-closure-bytes must not
    /// exceed max-closure-batch-bytes. Under `--search regions` with more
    /// than one worker, models arrive in the schedule's order, which differs
    /// between runs; the family of answer sets is the same. Formula
    /// completion under `--search clauses` has a separate worker setting.
    #[arg(long, visible_alias = "threads", value_name = "auto|N", value_parser = zetesis_backend::parse_threads, default_value = "auto", hide_short_help = true)]
    pub workers: NonZeroUsize,
    /// Workers for unresolved formula queries: under `--search clauses`, and
    /// under regions with one CPU walker or general device propagation.
    /// Complete device certificates use scalar validation with no residual
    /// pool. Multiple CPU region workers decide their own leaves in `--workers`.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.completion_workers, hide_short_help = true)]
    pub completion_workers: NonZeroUsize,
    /// Maximum reserved capacity for cold reduct preparation and for each query's
    /// retained workspace. Shared theory payload and allocator metadata are excluded.
    /// Omitted, it is the library default scaled by `--memory`.
    #[arg(long, hide_short_help = true)]
    pub max_reduct_bytes: Option<u64>,
    /// Maximum reserved capacity, under `--search clauses`, for the shared
    /// prepared reduct, worker queries and
    /// transient/result slots, excluding the scalar cursor, allocator/table overhead,
    /// thread stacks and GPU storage.
    /// Optional class preparation/checking uses the same ceiling independently.
    /// Omitted, it is the library default scaled by `--memory`.
    #[arg(long, hide_short_help = true)]
    pub max_completion_scratch_bytes: Option<u64>,
    /// Maximum candidate seeds; reaching a limit leaves search incomplete.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.max_candidates, hide_short_help = true)]
    pub max_candidates: u64,
    /// Maximum reserved capacity for necessary candidate restrictions, including
    /// indexes, join scratch and replacement overlap. Shared program storage and
    /// allocator overhead are excluded.
    /// Omitted, it is the library default scaled by `--memory`.
    #[arg(long, hide_short_help = true)]
    pub max_candidate_bytes: Option<usize>,
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
    /// Maximum reserved named capacity per independent lazy CPU closure, including
    /// spare capacity and replacement overlap; not resident bytes. Omitted, it is
    /// max-closure-batch-bytes divided by the worker count; given, workers × this
    /// value must not exceed max-closure-batch-bytes.
    #[arg(long, hide_short_help = true)]
    pub max_closure_bytes: Option<usize>,
    /// Collective reserved capacity for independent CPU preparation, the idle cache
    /// and assigned closure allowances; every worker's per-closure allowance is
    /// admitted against it. Also bounds immutable query preparation bytes.
    /// Omitted, it is the library default scaled by `--memory`.
    #[arg(long, hide_short_help = true)]
    pub max_closure_batch_bytes: Option<usize>,
    /// Device work per formula candidate, independent of CPU work. Bounds
    /// propagation setup/sweeps or the complete tight-support scan.
    /// Insufficient mandatory work refuses before device submission.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.gpu_formula_work, hide_short_help = true)]
    pub gpu_formula_work: u32,
    /// Device propagation sweeps per formula candidate. Zero keeps original-truth
    /// setup and sends undecided candidates to exact CPU residual search.
    /// Does not apply when complete tight support is selected.
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
    /// Maximum original bytes in each file or standard input before parsing.
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
    /// Maximum reserved batch capacity, excluding allocator/driver overhead.
    /// Lazy GPU reserves half for source state and half for transient transport.
    /// Shared CPU rounds use the full allowance for source/world state.
    /// Omitted, it is the library default scaled by `--memory`.
    #[arg(long, hide_short_help = true)]
    pub max_batch_bytes: Option<u64>,
}

/// The host's physical memory in bytes, read once per process the first
/// time it is asked for, when the default allowance is taken or the
/// statistics header prints it: the header prints the reading the allowance
/// came from.
pub(crate) fn host_memory() -> Option<u64> {
    static HOST_MEMORY: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *HOST_MEMORY.get_or_init(read_host_memory)
}

/// The host's physical memory in bytes, from `/proc/meminfo`.
#[cfg(target_os = "linux")]
fn read_host_memory() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kibibytes: u64 = text
        .lines()
        .find_map(|line| line.strip_prefix("MemTotal:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    kibibytes.checked_mul(1024)
}

/// The host's physical memory in bytes, from the system's `sysctl`: the
/// crate forbids foreign calls, and the system command is the reading
/// without one.
#[cfg(target_os = "macos")]
fn read_host_memory() -> Option<u64> {
    let output = std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;
    std::str::from_utf8(&output.stdout)
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// The host does not report its physical memory on this platform.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn read_host_memory() -> Option<u64> {
    None
}

/// The default allowance: half of the host's memory, at least the reference.
fn host_memory_allowance() -> u64 {
    host_memory().map_or(crate::SolveConfig::REFERENCE_MEMORY, |memory| {
        (memory / 2).max(crate::SolveConfig::REFERENCE_MEMORY)
    })
}

impl Options {
    /// The library's defaults for the allowance and the workers: what each
    /// byte ceiling not given on the command line is.
    fn allowed(&self) -> crate::SolveConfig {
        crate::SolveConfig::for_allowance(self.memory, self.workers)
    }

    /// The per-closure allowance: the given value, or each worker's share of
    /// the collective ceiling.
    #[must_use]
    pub fn closure_allowance(&self) -> usize {
        self.max_closure_bytes.unwrap_or_else(|| {
            self.max_closure_batch_bytes.map_or_else(
                || self.allowed().max_closure_bytes,
                |collective| collective / self.workers.get(),
            )
        })
    }

    /// The collective closure ceiling: the given value, or the library
    /// default scaled by the allowance.
    #[must_use]
    pub fn closure_collective(&self) -> usize {
        self.max_closure_batch_bytes
            .unwrap_or_else(|| self.allowed().max_closure_batch_bytes)
    }
}

impl From<&Options> for crate::SolveConfig {
    fn from(options: &Options) -> Self {
        let allowed = options.allowed();
        let formula = crate::admission::formula_limits(options);
        Self {
            backend: options.backend,
            grounder: options.grounder,
            source_batching: options.source_batching,
            oracle: options.oracle,
            search: options.search,
            stats: options.stats,
            constraints: zetesis_themelios::ConstraintCheckLimits {
                max_work: formula.max_work,
                max_substitutions: formula.max_substitutions,
                max_scalar_bytes: options.max_expansion_bytes,
            },
            models: options.models,
            max_model_work: options.max_model_work,
            max_model_bytes: options.max_model_bytes.unwrap_or(allowed.max_model_bytes),
            max_search_work: options.max_search_work,
            max_search_decisions: options.max_search_decisions,
            max_projection_entries: options.max_projection_entries,
            max_projection_nodes: options.max_projection_nodes,
            max_projection_bytes: options
                .max_projection_bytes
                .unwrap_or(allowed.max_projection_bytes),
            max_objective_work: options.max_objective_work,
            max_objective_bound_work: options.max_objective_bound_work,
            max_objective_bindings: options.max_objective_bindings,
            max_objective_keys: options.max_objective_keys,
            max_objective_key_bytes: options
                .max_objective_key_bytes
                .unwrap_or(allowed.max_objective_key_bytes),
            max_optimal_models: options.max_optimal_models,
            max_optimal_atoms: options.max_optimal_atoms,
            max_optimal_bytes: options
                .max_optimal_bytes
                .unwrap_or(allowed.max_optimal_bytes),
            batch_size: options.batch_size,
            workers: options.workers,
            completion_workers: options.completion_workers,
            max_reduct_bytes: options.max_reduct_bytes.unwrap_or(allowed.max_reduct_bytes),
            max_completion_scratch_bytes: options
                .max_completion_scratch_bytes
                .unwrap_or(allowed.max_completion_scratch_bytes),
            max_candidates: options.max_candidates,
            max_candidate_bytes: options
                .max_candidate_bytes
                .unwrap_or(allowed.max_candidate_bytes),
            max_carrier_atoms: options.max_carrier_atoms,
            max_work: options.max_work,
            max_closure_bytes: options.closure_allowance(),
            max_closure_batch_bytes: options.closure_collective(),
            gpu_formula_work: options.gpu_formula_work,
            gpu_formula_rounds: options.gpu_formula_rounds,
            max_source_work: options.max_source_work,
            max_atoms: options.max_atoms,
            max_substitutions: options.max_substitutions,
            max_ground_rules: options.max_ground_rules,
            max_batch_bytes: options.max_batch_bytes.unwrap_or(allowed.max_batch_bytes),
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

fn grounder_parser() -> impl TypedValueParser<Value = Grounder> {
    policy_parser([
        (Grounder::Auto, PossibleValue::new(Grounder::Auto.label()).help("Prefer lazy source grounding where admitted, independently of hardware.")),
        (Grounder::Lazy, PossibleValue::new(Grounder::Lazy.label()).help("Require relational source joins, or CPU hybrid formula grounding with an eager producer core and streamed eligible constraints.")),
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

fn search_parser() -> impl TypedValueParser<Value = SearchMethod> {
    policy_parser([
        (
            SearchMethod::Regions,
            PossibleValue::new(SearchMethod::Regions.label()).help(
                "Regions narrowed by the theory's readings, for candidates and for the reduct query; no clause form.",
            ),
        ),
        (
            SearchMethod::Clauses,
            PossibleValue::new(SearchMethod::Clauses.label())
                .help("The classical search over a clause form for candidates, and the clause query for the reduct."),
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
