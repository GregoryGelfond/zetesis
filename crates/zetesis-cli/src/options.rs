use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{Parser, Subcommand};
use std::num::NonZeroUsize;
use std::path::PathBuf;

use crate::{Backend, Grounder, Oracle, SearchMethod, SourceBatching};

#[cfg(any(target_os = "linux", test))]
mod cgroup;
mod values;

#[cfg(test)]
mod tests;

/// Commands that do not read an answer-set program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// List compiled GPU APIs and detected adapters with their capabilities.
    Devices,
}

/// CLI configuration using the shared ordinary resource policy.
#[derive(Clone, Debug, Parser)]
#[command(
    name = "zetesis",
    version,
    disable_help_flag = true,
    arg(clap::Arg::new("help").short('h').long("help").global(true).action(clap::ArgAction::HelpShort).help("Show everyday solving options")),
    arg(clap::Arg::new("help-all").long("help-all").global(true).action(clap::ArgAction::HelpLong).help("Show all execution and output options")),
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
    /// Execution backend: cpu (the default) or gpu.
    ///
    /// The everyday help names these two; the full help lists every value with
    /// its meaning from the backend vocabulary, the GPU APIs included, in place
    /// of the parser's own list of values.
    #[arg(long, value_parser = zetesis_backend::BackendParser, default_value = "cpu", hide_possible_values = true, long_help = backend_long_help())]
    pub backend: Backend,
    /// Grounding mode, independent of execution backend.
    ///
    /// `eager` instantiates every rule before solving. `lazy` instantiates on
    /// demand: relational source joins on CPU or GPU, or, for formula inputs,
    /// a producer core checked by the selected backend with eligible constraints
    /// streamed on the host. Terminal definitions (derived predicates nothing
    /// reads) can be reconstructed per answer.
    /// Objectives require complete constraint checks before scoring and disable
    /// terminal deferral. Table joins are refused. `auto` admits relational source
    /// joins where it can, else instantiates an eager base and defers terminal
    /// definitions. Resource allowances still apply.
    #[arg(long, value_parser = grounder_parser(), default_value = "auto")]
    pub grounder: Grounder,
    /// Positive joins during eager formula grounding.
    ///
    /// Table reuses support masks for eligible flat patterns. Relational source
    /// grounding and support-growth rounds retain indexed joins. Preparation
    /// consumes the configured support storage and records grounding work.
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
    /// batched completion under the shared thread and memory allowance. The
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
    /// the mark as they observe cancellation, without reading the clock. Blocking source I/O,
    /// frontend operations and a running GPU kernel cannot be preempted.
    /// Library callers supply their own Cancellation instead of this process option.
    #[arg(long, value_name = "DURATION", value_parser = values::seconds)]
    pub time_limit: Option<u64>,
    /// Memory allowance in bytes, or a whole number with KiB, MiB, GiB or TiB.
    ///
    /// Bounds named capacities during input, grounding, solving and publication;
    /// it is not a resident-memory limit. The default is half of the memory
    /// available to the process: physical memory, or on Linux the memory limit
    /// of its control group when that is lower; two gibibytes when the host
    /// does not report it.
    #[arg(long, visible_alias = "memory-budget", value_name = "SIZE", value_parser = zetesis_backend::parse_memory, default_value_t = host_memory_allowance(), hide_default_value = true, hide_short_help = true)]
    pub memory: u64,
    /// Maximum models to display; 0 requests all.
    ///
    /// Without objectives this stops
    /// search early. Optimization seeks exhaustion and then displays this many
    /// tied optima; interrupted runs may display incumbents without proving an optimum.
    #[arg(long, default_value_t = crate::SolveConfig::DEFAULT.models, display_order = 0)]
    pub models: usize,
    /// Host threads; auto uses available parallelism, falling back to one.
    ///
    /// The shared policy assigns worker and scratch capacity within the memory
    /// allowance. Parallel answer order can vary; the answer-set family does not.
    #[arg(long = "threads", visible_alias = "workers", value_name = "auto|N", value_parser = zetesis_backend::parse_threads, default_value = "auto")]
    pub workers: NonZeroUsize,
}

/// The full help's account of `--backend`: every value with its meaning, in the
/// vocabulary's order, so that each API's scope and limits are stated where the
/// expert values are offered.
fn backend_long_help() -> String {
    let values: Vec<String> = Backend::ALL
        .into_iter()
        .map(|backend| format!("  {:<7} {}", backend.label(), backend.help()))
        .collect();
    format!(
        "Execution backend: cpu (the default) or gpu.\n\nValues:\n{}\n\n\
         An unavailable GPU fails; device failure does not silently retry on CPU.",
        values.join("\n")
    )
}

/// The memory available to the process in bytes, read once per process: when
/// the command is built, since its default allowance derives from the reading,
/// so on every invocation, an explicit `--memory` included; or, for a caller
/// that builds `Options` itself, when the statistics header first prints it.
/// The header prints the reading the allowance came from.
pub(crate) fn host_memory() -> Option<u64> {
    static HOST_MEMORY: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *HOST_MEMORY.get_or_init(read_host_memory)
}

/// The memory available to the process in bytes, from the files Linux keeps.
#[cfg(target_os = "linux")]
fn read_host_memory() -> Option<u64> {
    available_memory(|path| std::fs::read_to_string(path).ok())
}

/// The memory available to the process in bytes, where `read` returns a file's
/// text: the host's physical memory from `/proc/meminfo`, lowered to the memory
/// limit of the process's control group when one is set, since inside a
/// container the host's figure is not the memory the process may use. Without
/// `/proc/meminfo` there is no reading, whatever a group states: a group's value
/// alone can be version 1's "unlimited", which only the comparison with
/// physical memory renders harmless.
#[cfg(any(target_os = "linux", test))]
fn available_memory(read: impl Fn(&std::path::Path) -> Option<String>) -> Option<u64> {
    let physical = memory_total(&read(std::path::Path::new("/proc/meminfo"))?)?;
    let limit = read(std::path::Path::new("/proc/self/cgroup"))
        .and_then(|groups| cgroup::memory_limit(&groups, &read));
    Some(limit.map_or(physical, |limit| limit.min(physical)))
}

/// Physical memory in bytes from the text of `/proc/meminfo`.
#[cfg(any(target_os = "linux", test))]
fn memory_total(meminfo: &str) -> Option<u64> {
    let kibibytes: u64 = meminfo
        .lines()
        .find_map(|line| line.strip_prefix("MemTotal:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    kibibytes.checked_mul(1024)
}

/// The host's physical memory in bytes, from the kernel's `hw.memsize`
/// through one `sysctlbyname` call. macOS reports physical memory only through
/// system interfaces (`sysctl`, or Mach's `host_info`), each a foreign call;
/// this is the one in zetesis's own source, and it starts no process. The
/// kernel writes at most the eight bytes it is given and stores how many it
/// wrote; anything but a complete eight-byte reading is no reading.
#[cfg(target_os = "macos")]
#[expect(
    unsafe_code,
    reason = "macOS reports physical memory only through system interfaces: one sysctlbyname call into a fixed eight-byte buffer"
)]
fn read_host_memory() -> Option<u64> {
    use std::ffi::{c_char, c_int, c_void};
    unsafe extern "C" {
        fn sysctlbyname(
            name: *const c_char,
            oldp: *mut c_void,
            oldlenp: *mut usize,
            newp: *mut c_void,
            newlen: usize,
        ) -> c_int;
    }
    let mut memory = 0_u64;
    let mut length = size_of::<u64>();
    // SAFETY: the declaration matches sysctlbyname(3), `int sysctlbyname(const
    // char *name, void *oldp, size_t *oldlenp, void *newp, size_t newlen)`, with
    // `usize` for `size_t`. The name is a NUL-terminated string; `oldp` points
    // to `memory`, whose size `length` states, so the kernel writes at most
    // those eight bytes and stores the count it wrote in `length`; with a null
    // `newp` and a zero `newlen` nothing is set.
    let status = unsafe {
        sysctlbyname(
            c"hw.memsize".as_ptr(),
            (&raw mut memory).cast(),
            &raw mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    (status == 0 && length == size_of::<u64>()).then_some(memory)
}

/// The host does not report its physical memory on this platform.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn read_host_memory() -> Option<u64> {
    None
}

/// The default allowance: half of the memory available to the process, or
/// the reference when the host does not report it.
fn host_memory_allowance() -> u64 {
    allowance(host_memory())
}

/// Half of `available` memory whenever there is a reading, so that the
/// allowance stays within what the process may use, a container's limit
/// included; the reference without one.
fn allowance(available: Option<u64>) -> u64 {
    available.map_or(crate::SolveConfig::REFERENCE_MEMORY, |memory| memory / 2)
}

impl Options {
    /// The same ordinary resource policy used by the library engine.
    #[must_use]
    pub fn resources(&self) -> zetesis_solve::Resources {
        zetesis_solve::Resources::new(self.memory, self.workers)
    }
}

impl From<&Options> for crate::SolveConfig {
    fn from(options: &Options) -> Self {
        Self {
            backend: options.backend,
            grounder: options.grounder,
            source_batching: options.source_batching,
            oracle: options.oracle,
            search: options.search,
            stats: options.stats,
            models: options.models,
            ..options.resources().solve_config()
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
        (Grounder::Lazy, PossibleValue::new(Grounder::Lazy.label()).help("Require relational source joins, or CPU formula grounding with an eager producer core, streamed eligible constraints and terminal definitions reconstructed per answer.")),
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
