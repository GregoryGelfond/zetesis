//! The `perf` command: bounded baseline campaigns and the instrumented
//! profile matrix.

use crate::{Completion, Error};
use clap::{Args, ValueEnum};
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use zetesis_validation::performance::{self, Limits, Request, Schedule, Suite};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum SuiteArgument {
    Baseline,
    Queens,
    Corpus,
    Series,
}

/// A clean corpus campaign: the ordinary CPU comparison, or the instrumented
/// profile matrix with its retained parity, timing and execution evidence.
#[derive(Debug, Args)]
pub struct PerfOptions {
    /// Select a manifest-relative clean corpus case; repeat for an ordinary CPU campaign.
    #[arg(long = "case", conflicts_with_all = ["profile", "workers", "completion_workers", "clingo_workers", "batch_size", "native_report_bytes"])]
    cases: Vec<String>,
    /// Separate child RSS rounds per solver/case, zero through 41, through a
    /// fresh helper; excluded from the timed population.
    #[arg(long, default_value_t = 0)]
    memory_runs: usize,
    /// Explicit native eager-formula join strategy; omission preserves its default.
    #[arg(long, value_enum)]
    formula_joins: Option<JoinArgument>,
    /// Explicit native formula search method; omission preserves its default.
    #[arg(long, value_enum)]
    search: Option<SearchArgument>,
    /// Explicit instrumented profile; repeat for a matrix. Corpus defaults to all four.
    #[arg(long, value_enum)]
    profile: Vec<ProfileArgument>,
    /// Matrix closure workers (default 4; requires --profile or --suite corpus).
    #[arg(long)]
    workers: Option<std::num::NonZeroUsize>,
    /// Matrix formula completion workers (default 4).
    #[arg(long)]
    completion_workers: Option<std::num::NonZeroUsize>,
    /// Reference workers in the instrumented matrix (default 1).
    #[arg(long)]
    clingo_workers: Option<std::num::NonZeroUsize>,
    /// Candidate batch ceiling for matrix native profiles (default 64).
    #[arg(long)]
    batch_size: Option<std::num::NonZeroUsize>,
    /// Cooperative native deadline in whole seconds for every matrix profile;
    /// omission imposes none.
    #[arg(long)]
    time_limit: Option<std::num::NonZeroU64>,
    /// Requested native reduct procedure for every matrix profile (default auto).
    #[arg(long, value_enum)]
    oracle: Option<OracleArgument>,
    /// Per-invocation stdout/stderr ceiling (default retains the established protocol bound).
    #[arg(long)]
    sample_bytes: Option<usize>,
    /// Matrix native JSON decoder byte ceiling (default 8 MiB; separate from capture).
    #[arg(long)]
    native_report_bytes: Option<usize>,
    /// Combined retained stdout/stderr bytes for the campaign.
    #[arg(long, default_value_t = 134_217_728)]
    capture_bytes: usize,
    /// Serialized evidence ceiling, including all raw captures.
    #[arg(long, default_value_t = 536_870_912)]
    report_bytes: usize,
    /// Self-contained clean examples/correctness directory.
    root: PathBuf,
    /// Established CPU baseline, all six curated queens encodings, the
    /// instrumented full corpus, or the fixed instrumented series cells.
    #[arg(long, value_enum, default_value = "baseline")]
    suite: SuiteArgument,
    /// Native zetesis executable path.
    #[arg(long)]
    zetesis: PathBuf,
    /// Independent clingo executable path.
    #[arg(long)]
    clingo: PathBuf,
    /// New evidence path; existing files are never replaced.
    #[arg(long)]
    report: PathBuf,
    /// Timed rounds per case (legacy default 21 pairs; instrumented matrix default 20).
    #[arg(long)]
    repetitions: Option<usize>,
    /// Untimed preparation pairs per case, zero through five.
    #[arg(long, default_value_t = 3)]
    warmups: usize,
    /// Per-invocation timeout in seconds.
    #[arg(long, default_value_t = 30)]
    timeout_seconds: u64,
    /// Total campaign scheduling deadline in seconds.
    #[arg(long, default_value_t = 180)]
    campaign_seconds: u64,
}
/// Run the campaign and write its one-line outcome to `output`.
///
/// Memory rounds re-execute this process's executable as their measurement
/// helper, so it must answer the helper protocol, as `zetesis-bench` does.
pub(crate) fn execute(
    options: &PerfOptions,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
) -> Result<Completion, Error> {
    if matches!(options.suite, SuiteArgument::Corpus | SuiteArgument::Series)
        || !options.profile.is_empty()
    {
        if !options.cases.is_empty() {
            return Err(Error::Usage("--case requires the ordinary CPU campaign"));
        }
        return matrix(options, output, diagnostics);
    }
    if options.workers.is_some()
        || options.completion_workers.is_some()
        || options.clingo_workers.is_some()
        || options.batch_size.is_some()
        || options.native_report_bytes.is_some()
        || options.time_limit.is_some()
        || options.oracle.is_some()
    {
        return Err(Error::Usage(
            "matrix controls require --profile, --suite corpus or --suite series",
        ));
    }
    let native = std::path::absolute(&options.zetesis).map_err(Error::Io)?;
    let reference = std::path::absolute(&options.clingo).map_err(Error::Io)?;
    let mut limits = Limits::default();
    limits.process.timeout = Duration::from_secs(options.timeout_seconds);
    limits.campaign_timeout = Duration::from_secs(options.campaign_seconds);
    if let Some(bytes) = options.sample_bytes {
        limits.process.max_output_bytes = bytes;
    }
    limits.max_total_capture_bytes = options.capture_bytes;
    limits.max_report_bytes = options.report_bytes;
    let schedule = if options.cases.is_empty() {
        Schedule::for_suite(
            match options.suite {
                SuiteArgument::Baseline => Suite::Baseline,
                SuiteArgument::Queens => Suite::Queens,
                SuiteArgument::Corpus | SuiteArgument::Series => {
                    return Err(Error::Usage(
                        "corpus and series suites require matrix dispatch",
                    ));
                }
            },
            options.warmups,
            options.repetitions.unwrap_or(21),
        )
    } else {
        if !matches!(options.suite, SuiteArgument::Baseline) {
            return Err(Error::Usage(
                "explicit --case cannot be combined with a named nondefault suite",
            ));
        }
        Schedule::for_cases(
            options.cases.clone(),
            options.warmups,
            options.repetitions.unwrap_or(21),
        )
    }
    .and_then(|schedule| schedule.with_memory(options.memory_runs))
    .map_err(Error::Campaign)?;
    let request = Request {
        corpus: &options.root,
        native: &native,
        reference: &reference,
        report: &options.report,
        schedule,
        formula_joins: options.formula_joins.map(Into::into),
        search: options.search.map(Into::into),
        limits,
    };
    let report = if options.memory_runs > 0 || request.schedule.suite().is_none() {
        let helper = std::env::current_exe().map_err(Error::Io)?;
        performance::run_with_runner(&request, &helper)
    } else {
        performance::run(&request)
    }
    .map_err(Error::Campaign)?;
    report.publish().map_err(Error::Campaign)?;
    writeln!(
        output,
        "{}: {} retained observations; evidence {}",
        if report.passed() { "pass" } else { "fail" },
        report.samples().len(),
        options.report.display()
    )
    .map_err(Error::Io)?;
    Ok(if report.passed() {
        Completion::Passed
    } else {
        Completion::NonPass
    })
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ProfileArgument {
    /// The shipped defaults: automatic grounding and oracle on the CPU.
    CpuAuto,
    CpuEager,
    CpuLazy,
    MetalEager,
    MetalLazy,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum JoinArgument {
    Indexed,
    Table,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OracleArgument {
    Auto,
    Closure,
    Countermodel,
}

impl From<OracleArgument> for zetesis_validation::selected::Oracle {
    fn from(value: OracleArgument) -> Self {
        match value {
            OracleArgument::Auto => Self::Auto,
            OracleArgument::Closure => Self::Closure,
            OracleArgument::Countermodel => Self::Countermodel,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum SearchArgument {
    Regions,
    Clauses,
}

impl From<SearchArgument> for zetesis_validation::selected::SearchMethod {
    fn from(value: SearchArgument) -> Self {
        match value {
            SearchArgument::Regions => Self::Regions,
            SearchArgument::Clauses => Self::Clauses,
        }
    }
}

impl From<JoinArgument> for zetesis_validation::selected::FormulaJoins {
    fn from(value: JoinArgument) -> Self {
        match value {
            JoinArgument::Indexed => Self::Indexed,
            JoinArgument::Table => Self::Table,
        }
    }
}

fn execution_profiles(options: &PerfOptions) -> Vec<zetesis_validation::selected::NativeExecution> {
    use zetesis_validation::selected::{Backend, Grounder, NativeExecution, Oracle};
    const METAL: Backend = Backend::Gpu(Some(zetesis_backend::GpuApi::Metal));
    let defaults = [
        ProfileArgument::CpuEager,
        ProfileArgument::CpuLazy,
        ProfileArgument::MetalEager,
        ProfileArgument::MetalLazy,
    ];
    let profiles = if options.profile.is_empty() {
        defaults.as_slice()
    } else {
        options.profile.as_slice()
    };
    profiles
        .iter()
        .map(|profile| {
            let (backend, grounder) = match profile {
                ProfileArgument::CpuAuto => (Backend::Cpu, Grounder::Auto),
                ProfileArgument::CpuEager => (Backend::Cpu, Grounder::Eager),
                ProfileArgument::CpuLazy => (Backend::Cpu, Grounder::Lazy),
                ProfileArgument::MetalEager => (METAL, Grounder::Eager),
                ProfileArgument::MetalLazy => (METAL, Grounder::Lazy),
            };
            NativeExecution {
                backend,
                grounder,
                formula_joins: options.formula_joins.map(Into::into),
                search: options.search.map(Into::into),
                workers: options
                    .workers
                    .unwrap_or(std::num::NonZeroUsize::new(4).expect("four is nonzero")),
                completion_workers: options
                    .completion_workers
                    .unwrap_or(std::num::NonZeroUsize::new(4).expect("four is nonzero")),
                batch_size: options
                    .batch_size
                    .unwrap_or(std::num::NonZeroUsize::new(64).expect("64 is nonzero")),
                time_limit_seconds: options.time_limit,
                oracle: options.oracle.map_or(Oracle::Auto, Into::into),
                ..NativeExecution::default()
            }
        })
        .collect()
}

fn matrix(
    options: &PerfOptions,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
) -> Result<Completion, Error> {
    use zetesis_validation::performance::matrix;
    let profiles = execution_profiles(options);
    let suite = match options.suite {
        SuiteArgument::Baseline => matrix::Suite::Baseline,
        SuiteArgument::Queens => matrix::Suite::Queens,
        SuiteArgument::Corpus => matrix::Suite::Corpus,
        SuiteArgument::Series => matrix::Suite::Series,
    };
    let native = std::path::absolute(&options.zetesis).map_err(Error::Io)?;
    let reference = std::path::absolute(&options.clingo).map_err(Error::Io)?;
    let mut limits = Limits::default();
    limits.process.timeout = Duration::from_secs(options.timeout_seconds);
    limits.campaign_timeout = Duration::from_secs(options.campaign_seconds);
    if let Some(bytes) = options.sample_bytes {
        limits.process.max_output_bytes = bytes;
    }
    limits.max_total_capture_bytes = options.capture_bytes;
    limits.max_report_bytes = options.report_bytes;
    let mut native_answers = zetesis_validation::answers::native_json::Limits::default();
    if let Some(bytes) = options.native_report_bytes {
        native_answers.report.max_input_bytes = bytes;
    }
    // The series knows its own record sizes; ceilings below them are raised.
    if matches!(options.suite, SuiteArgument::Series) {
        limits = zetesis_validation::performance::series::limits(limits);
        native_answers = zetesis_validation::performance::series::native_answers(native_answers);
    }
    let plan = matrix::Plan::new(
        suite,
        profiles,
        options
            .clingo_workers
            .unwrap_or(std::num::NonZeroUsize::new(1).expect("one is nonzero")),
        options.warmups,
        options.repetitions.unwrap_or(20),
    )
    .and_then(|plan| plan.with_memory(options.memory_runs))
    .map_err(Error::Campaign)?;
    writeln!(
        diagnostics,
        "Recording instrumented solver matrix; evidence will be written to {}",
        options.report.display()
    )
    .map_err(Error::Io)?;
    // The memory rounds run each solver as this executable's child.
    let helper = (options.memory_runs > 0)
        .then(std::env::current_exe)
        .transpose()
        .map_err(Error::Io)?;
    let request = matrix::Request {
        corpus: &options.root,
        native: &native,
        reference: Some(matrix::Reference {
            executable: &reference,
            policy: matrix::ReferencePolicy::AllPhases,
        }),
        report: &options.report,
        plan,
        limits,
        native_answers,
        max_spelling_bytes: limits.answers.max_input_bytes,
        helper: helper.as_deref(),
    };
    let report =
        zetesis_validation::performance::command::run(&request, matrix::NativeInvocation::Legacy)
            .map_err(Error::Campaign)?;
    report.publish().map_err(Error::Campaign)?;
    writeln!(
        output,
        "{}: {} matrix positions retained; accounted={}; evidence {}",
        if report.passed() {
            "pass"
        } else {
            "non-pass cells retained"
        },
        report.samples().len(),
        report.accounted(),
        options.report.display()
    )
    .map_err(Error::Io)?;
    Ok(if report.passed() {
        Completion::Passed
    } else {
        Completion::NonPass
    })
}
