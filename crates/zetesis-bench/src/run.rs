//! The `run` command: the benchmark suite measured through the given `zetesis`,
//! beside clingo when there is one and alone when there is not.

use crate::{Completion, Error, ViewOptions};
use clap::{Args, ValueEnum};
use std::{
    io,
    num::{NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use zetesis_presentation::Layout;
use zetesis_validation::{
    performance::{self, matrix, scalability, series},
    selected,
};

#[cfg(test)]
mod tests;

/// Maintained source selections; every cell retains complete selected families.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Suite {
    /// All 94 verified corpus entries.
    #[default]
    Corpus,
    /// SEND, queens variant 02 and task allocation.
    Baseline,
    /// The six unchanged eight-queens encodings.
    Queens,
    /// The maintained 22-cell generated/constant workload series.
    Series,
    /// Ten maintained authored/corpus workloads for thread comparisons.
    Scalability,
}
impl Suite {
    /// The suite's name on the command line and in the default evidence name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Corpus => "corpus",
            Self::Baseline => "baseline",
            Self::Queens => "queens",
            Self::Series => "series",
            Self::Scalability => "scalability",
        }
    }
}
impl From<Suite> for matrix::Suite {
    fn from(value: Suite) -> Self {
        match value {
            Suite::Corpus => Self::Corpus,
            Suite::Baseline => Self::Baseline,
            Suite::Queens => Self::Queens,
            Suite::Series => Self::Series,
            Suite::Scalability => Self::Scalability,
        }
    }
}

/// Explicit executable-interface compatibility, independent of execution policy.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum NativeInterface {
    /// Historical flat solver arguments.
    Legacy,
    /// Canonical explicit solve command.
    Solve,
}
impl From<NativeInterface> for matrix::NativeInvocation {
    fn from(value: NativeInterface) -> Self {
        match value {
            NativeInterface::Legacy => Self::Legacy,
            NativeInterface::Solve => Self::Solve,
        }
    }
}

/// Requested materialization policy for the benchmark profile.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Grounder {
    /// Solver's automatic policy.
    #[default]
    Auto,
    /// Complete eager source materialization.
    Eager,
    /// Lazy source joins, with unsupported sources retained as refusals.
    Lazy,
}
impl From<Grounder> for selected::Grounder {
    fn from(value: Grounder) -> Self {
        match value {
            Grounder::Auto => Self::Auto,
            Grounder::Eager => Self::Eager,
            Grounder::Lazy => Self::Lazy,
        }
    }
}

/// Requested reduct procedure.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Oracle {
    /// Solver's automatic choice.
    #[default]
    Auto,
    /// Least-consequence closure.
    Closure,
    /// Countermodel search.
    Countermodel,
}
impl From<Oracle> for selected::Oracle {
    fn from(value: Oracle) -> Self {
        match value {
            Oracle::Auto => Self::Auto,
            Oracle::Closure => Self::Closure,
            Oracle::Countermodel => Self::Countermodel,
        }
    }
}

/// Requested formula search method.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Search {
    /// Region search.
    Regions,
    /// Clause search.
    Clauses,
}
impl From<Search> for selected::SearchMethod {
    fn from(value: Search) -> Self {
        match value {
            Search::Regions => Self::Regions,
            Search::Clauses => Self::Clauses,
        }
    }
}

/// Requested eager-formula join strategy.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum FormulaJoins {
    /// Indexed joins.
    Indexed,
    /// Table joins.
    Table,
}
impl From<FormulaJoins> for selected::FormulaJoins {
    fn from(value: FormulaJoins) -> Self {
        match value {
            FormulaJoins::Indexed => Self::Indexed,
            FormulaJoins::Table => Self::Table,
        }
    }
}

/// One benchmark campaign; statistics are always part of its protocol.
#[derive(Debug, Args)]
pub struct RunOptions {
    /// Existing clean corpus directory from the repository; never downloaded.
    #[arg(default_value = "examples/correctness")]
    pub root: PathBuf,
    /// Maintained workload selection.
    #[arg(long, value_enum, default_value_t)]
    pub suite: Suite,
    /// A case of the suite to measure; repeat for several, in order. Series
    /// paths name workload entries, including generated paths, and select every
    /// amended cell of that entry. Corpus case paths are corpus-relative.
    /// Omitted, the whole suite is measured.
    #[arg(long = "case", value_name = "PATH")]
    pub cases: Vec<String>,
    /// Authored examples root for --suite scalability; defaults to examples.
    #[arg(long)]
    pub examples: Option<PathBuf>,
    /// Include the unchanged Einstein riddle in --suite scalability.
    #[arg(long)]
    pub include_einstein: bool,
    /// Native executable; omitted uses the installed `zetesis` (the one beside
    /// this tool, else the first on PATH).
    #[arg(long)]
    pub zetesis: Option<PathBuf>,
    /// Native command interface: `solve`, the default, or `legacy` for an older
    /// binary's flat arguments.
    #[arg(
        long,
        value_enum,
        hide_short_help = true,
        help_heading = "Advanced measurement controls"
    )]
    pub native_interface: Option<NativeInterface>,
    /// The clingo to compare with, which must run. Omitted, the first `clingo`
    /// on PATH is used, and without one the run measures zetesis alone.
    #[arg(long, conflicts_with = "without_clingo")]
    pub clingo: Option<PathBuf>,
    /// Measure zetesis alone: qualify each workload against its recorded
    /// contract and launch no clingo.
    #[arg(long)]
    pub without_clingo: bool,
    /// New complete JSON evidence file, including failures and unlaunched
    /// positions; omitted, `zetesis-bench-<suite>-<UTC time>.json` in the
    /// current directory. An existing file is never replaced.
    #[arg(long)]
    pub report: Option<PathBuf>,
    /// Execution backend: cpu (the default), gpu, metal or vulkan. A GPU backend
    /// is required: its absence is retained as a non-pass.
    #[arg(long, value_parser = zetesis_backend::BackendParser, default_value = "cpu")]
    pub backend: selected::Backend,
    /// Requested materialization policy.
    #[arg(long, value_enum, default_value_t)]
    pub grounder: Grounder,
    /// Native candidate/closure workers; auto uses the host's available parallelism.
    #[arg(long, alias = "workers", value_name = "auto|N", value_parser = zetesis_backend::parse_threads, default_value = "auto")]
    pub threads: NonZeroUsize,
    /// Compare native thread counts, for example 1,2,4; clingo only qualifies.
    #[arg(long, value_delimiter = ',', num_args = 1.., conflicts_with = "threads")]
    pub compare_threads: Vec<NonZeroUsize>,
    /// Compare eager and lazy native grounding; clingo only qualifies.
    #[arg(long, conflicts_with = "grounder")]
    pub compare_grounders: bool,
    /// Compare native backends, for example cpu,gpu; clingo only qualifies.
    #[arg(long, value_delimiter = ',', num_args = 1.., value_parser = zetesis_backend::BackendParser, conflicts_with = "backend")]
    pub compare_backends: Vec<selected::Backend>,
    /// Requested reduct procedure for every profile.
    #[arg(
        long,
        value_enum,
        default_value_t,
        hide_short_help = true,
        help_heading = "Advanced measurement controls"
    )]
    pub oracle: Oracle,
    /// Formula search method for every profile; omitted, the suite's default.
    #[arg(
        long,
        value_enum,
        hide_short_help = true,
        help_heading = "Advanced measurement controls"
    )]
    pub search: Option<Search>,
    /// Eager-formula join strategy for every profile; omitted, the suite's default.
    #[arg(
        long,
        value_enum,
        hide_short_help = true,
        help_heading = "Advanced measurement controls"
    )]
    pub formula_joins: Option<FormulaJoins>,
    /// Native exact residual completion workers.
    #[arg(
        long,
        default_value = "1",
        hide_short_help = true,
        help_heading = "Advanced measurement controls"
    )]
    pub completion_workers: NonZeroUsize,
    /// Candidate batch ceiling.
    #[arg(
        long,
        default_value = "64",
        hide_short_help = true,
        help_heading = "Advanced measurement controls"
    )]
    pub batch_size: NonZeroUsize,
    /// Explicit native grounding expansion ceiling, retained in every profile.
    #[arg(
        long,
        hide_short_help = true,
        help_heading = "Advanced measurement controls"
    )]
    pub max_expansion_work: Option<usize>,
    /// Cooperative native deadline in whole seconds for every profile; omitted,
    /// none.
    #[arg(
        long,
        value_name = "SECONDS",
        hide_short_help = true,
        help_heading = "Advanced measurement controls"
    )]
    pub time_limit: Option<NonZeroU64>,
    /// Unmodified clingo worker setting; no search-heuristic tuning.
    #[arg(
        long,
        alias = "clingo-workers",
        default_value = "1",
        hide_short_help = true,
        help_heading = "Advanced measurement controls"
    )]
    pub clingo_threads: NonZeroUsize,
    /// Untimed warmup rounds, separate from the mandatory qualification.
    #[arg(long, default_value_t = 1)]
    pub warmups: usize,
    /// Timed rounds per solver/case, one through 41.
    #[arg(long, default_value_t = 3)]
    pub repetitions: usize,
    /// Separate fresh-child RSS rounds, zero through 41.
    #[arg(
        long,
        default_value_t = 1,
        hide_short_help = true,
        help_heading = "Advanced measurement controls"
    )]
    pub memory_runs: usize,
    /// Timeout for each solver child, in seconds.
    #[arg(
        long,
        default_value_t = 10,
        hide_short_help = true,
        help_heading = "Advanced measurement controls"
    )]
    pub timeout_seconds: u64,
    /// Total campaign scheduling deadline, in seconds; no replacement launches.
    #[arg(
        long,
        default_value_t = 180,
        hide_short_help = true,
        help_heading = "Advanced measurement controls"
    )]
    pub campaign_seconds: u64,
    /// Combined stdout/stderr ceiling per invocation.
    #[arg(long, default_value_t = 16 * 1024 * 1024, hide_short_help = true, help_heading = "Advanced measurement controls")]
    pub sample_bytes: usize,
    /// Native JSON decoder source-byte ceiling, separate from capture.
    #[arg(long, default_value_t = 16 * 1024 * 1024, hide_short_help = true, help_heading = "Advanced measurement controls")]
    pub native_report_bytes: usize,
    /// Combined retained captures across the entire campaign.
    #[arg(long, default_value_t = 512 * 1024 * 1024, hide_short_help = true, help_heading = "Advanced measurement controls")]
    pub capture_bytes: usize,
    /// Serialized complete evidence ceiling.
    #[arg(long, default_value_t = 1024 * 1024 * 1024, hide_short_help = true, help_heading = "Advanced measurement controls")]
    pub report_bytes: usize,
    /// Human or structured presentation.
    #[command(flatten)]
    pub view: ViewOptions,
}

impl RunOptions {
    /// Map command arguments to the reusable bounded plan; no I/O is performed.
    ///
    /// The profiles are the product of the compared backends, grounders and
    /// thread counts, backends outermost; each axis not compared contributes
    /// its single requested value.
    ///
    /// # Errors
    /// Refuses options of another suite, case selections, worker counts,
    /// profile counts, dimensions or schedules outside maintained bounds.
    pub fn plan(&self) -> Result<matrix::Plan, performance::Error> {
        if self.suite != Suite::Scalability && (self.examples.is_some() || self.include_einstein) {
            return Err(performance::Error::Configuration(
                "--examples and --include-einstein require --suite scalability",
            ));
        }
        let scalability = self.suite == Suite::Scalability;
        let base = selected::NativeExecution {
            backend: self.backend,
            grounder: self.grounder.into(),
            oracle: self.oracle.into(),
            workers: self.threads,
            completion_workers: self.completion_workers,
            batch_size: self.batch_size,
            formula_joins: self
                .formula_joins
                .map(Into::into)
                .or(scalability.then_some(selected::FormulaJoins::Indexed)),
            search: self
                .search
                .map(Into::into)
                .or(scalability.then_some(selected::SearchMethod::Regions)),
            time_limit_seconds: self.time_limit,
            max_expansion_work: self.max_expansion_work,
            ..selected::NativeExecution::default()
        };
        let backends = if self.compare_backends.is_empty() {
            vec![self.backend]
        } else {
            self.compare_backends.clone()
        };
        let grounders = if self.compare_grounders {
            vec![selected::Grounder::Eager, selected::Grounder::Lazy]
        } else {
            vec![base.grounder]
        };
        let threads = if self.compare_threads.is_empty() {
            vec![self.threads]
        } else {
            self.compare_threads.clone()
        };
        let mut profiles = Vec::new();
        for &backend in &backends {
            for &grounder in &grounders {
                for &workers in &threads {
                    profiles.push(selected::NativeExecution {
                        backend,
                        grounder,
                        workers,
                        ..base
                    });
                }
            }
        }
        let plan = matrix::Plan::new(
            self.suite.into(),
            profiles,
            self.clingo_threads,
            self.warmups,
            self.repetitions,
        )?
        .with_memory(self.memory_runs)?;
        if self.cases.is_empty() {
            Ok(plan)
        } else {
            plan.with_cases(self.cases.clone())
        }
    }

    /// clingo is timed beside one profile, and only qualifies when profiles
    /// are compared.
    #[must_use]
    pub fn reference_policy(plan: &matrix::Plan) -> matrix::ReferencePolicy {
        if plan.profiles().len() == 1 {
            matrix::ReferencePolicy::AllPhases
        } else {
            matrix::ReferencePolicy::QualificationOnly
        }
    }

    /// Actual interface selection: the explicit `solve` command of whichever
    /// executable is measured, unless `--native-interface legacy` requests an
    /// older binary's flat arguments.
    #[must_use]
    pub fn invocation(&self) -> matrix::NativeInvocation {
        self.native_interface
            .map_or(matrix::NativeInvocation::Solve, Into::into)
    }

    /// The evidence file: the requested one, or a new name for this suite
    /// and the given time.
    ///
    /// # Errors
    /// Refuses a clock that precedes the Unix epoch.
    pub(crate) fn report(&self, now: SystemTime) -> Result<PathBuf, Error> {
        if let Some(path) = &self.report {
            return Ok(path.clone());
        }
        let since = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|_| Error::Usage("the system clock precedes the Unix epoch"))?;
        Ok(PathBuf::from(format!(
            "zetesis-bench-{}-{}.json",
            self.suite.name(),
            utc(since.as_secs())
        )))
    }
}

/// Where the run found clingo, or why it measures zetesis alone.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Clingo {
    /// The clingo the run compares with.
    Found(PathBuf),
    /// Measuring zetesis alone, as `--without-clingo` requested.
    Declined,
    /// Measuring zetesis alone: no `clingo` is on PATH.
    Absent,
}

/// The clingo the run compares with: the named one, which must be a runnable
/// file, else the first `clingo` on `search_path`, else none.
///
/// # Errors
/// Refuses a named clingo that is not a runnable file, and path failures other
/// than an absent `clingo`.
pub(crate) fn clingo(
    options: &RunOptions,
    search_path: Option<&std::ffi::OsStr>,
) -> Result<Clingo, Error> {
    use zetesis_validation::process::{is_executable_file, resolve_executable};
    if options.without_clingo {
        return Ok(Clingo::Declined);
    }
    match &options.clingo {
        Some(named) => {
            let path = resolve_executable(named, search_path).map_err(Error::Io)?;
            if is_executable_file(&path) {
                Ok(Clingo::Found(path))
            } else {
                Err(Error::Io(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("--clingo does not name a runnable file: {}", path.display()),
                )))
            }
        }
        None => match resolve_executable(Path::new("clingo"), search_path) {
            Ok(path) => Ok(Clingo::Found(path)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Clingo::Absent),
            Err(error) => Err(Error::Io(error)),
        },
    }
}

/// Run the campaign, publish its evidence and write its view to `output`.
///
/// Memory rounds re-execute this process's executable as their measurement
/// helper, so it must answer the helper protocol, as `zetesis-bench` does.
pub(crate) fn execute(
    options: &RunOptions,
    layout: Layout,
    output: &mut impl io::Write,
    diagnostics: &mut impl io::Write,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Completion, Error> {
    let current = std::env::current_exe().map_err(Error::Io)?;
    let search_path = std::env::var_os("PATH");
    let native = match &options.zetesis {
        Some(path) => executable(path, search_path.as_deref())?,
        None => installed_zetesis(&current, search_path.as_deref())?,
    };
    let plan = options.plan().map_err(Error::Campaign)?;
    let clingo = clingo(options, search_path.as_deref())?;
    match &clingo {
        Clingo::Found(_) => {}
        Clingo::Declined => writeln!(
            diagnostics,
            "Measuring zetesis alone, without clingo, as requested"
        )
        .map_err(Error::Io)?,
        Clingo::Absent => writeln!(
            diagnostics,
            "No clingo on PATH: measuring zetesis alone, each workload qualified by its recorded contract"
        )
        .map_err(Error::Io)?,
    }
    let evidence = options.report(SystemTime::now())?;
    let mut limits = performance::Limits::default();
    limits.process.timeout = Duration::from_secs(options.timeout_seconds);
    limits.process.max_output_bytes = options.sample_bytes;
    limits.campaign_timeout = Duration::from_secs(options.campaign_seconds);
    limits.max_total_capture_bytes = options.capture_bytes;
    limits.max_report_bytes = options.report_bytes;
    let mut native_answers = zetesis_validation::answers::native_json::Limits::default();
    native_answers.report.max_input_bytes = options.native_report_bytes;
    // The series knows its own record sizes; ceilings below them are raised.
    if options.suite == Suite::Series {
        limits = series::limits(limits);
        native_answers = series::native_answers(native_answers);
    }
    writeln!(
        diagnostics,
        "Recording benchmark evidence in {}",
        evidence.display()
    )
    .map_err(Error::Io)?;
    let policy = RunOptions::reference_policy(&plan);
    let request = matrix::Request {
        tool: crate::tool(),
        corpus: &options.root,
        native: &native,
        reference: match &clingo {
            Clingo::Found(path) => Some(matrix::Reference {
                executable: path,
                policy,
            }),
            Clingo::Declined | Clingo::Absent => None,
        },
        report: &evidence,
        plan,
        limits,
        native_answers,
        max_spelling_bytes: limits.answers.max_input_bytes,
        helper: (options.memory_runs > 0).then_some(current.as_path()),
    };
    let report = if options.suite == Suite::Scalability {
        scalability::run_with_cancellation(
            &request,
            options
                .examples
                .as_deref()
                .unwrap_or_else(|| Path::new("examples")),
            options.include_einstein,
            options.invocation(),
            cancelled,
        )
    } else {
        performance::command::run_with_cancellation(&request, options.invocation(), cancelled)
    }
    .map_err(Error::Campaign)?;
    report.publish().map_err(Error::Campaign)?;
    crate::view::run(&report.summary(), options.view.json, layout, output)?;
    Ok(if report.passed() {
        Completion::Passed
    } else {
        Completion::NonPass
    })
}

/// The installed `zetesis`: the one in the directory of `this` executable,
/// else the first on `search_path`.
fn installed_zetesis(this: &Path, search_path: Option<&std::ffi::OsStr>) -> Result<PathBuf, Error> {
    let beside = this.with_file_name(format!("zetesis{}", std::env::consts::EXE_SUFFIX));
    if beside.is_file() {
        Ok(beside)
    } else {
        executable(Path::new("zetesis"), search_path)
    }
}

fn executable(path: &Path, search_path: Option<&std::ffi::OsStr>) -> Result<PathBuf, Error> {
    zetesis_validation::process::resolve_executable(path, search_path).map_err(Error::Io)
}

/// Whole seconds since the Unix epoch as a UTC time in ISO 8601's basic
/// format, `YYYYMMDDTHHMMSSZ`, which every file system accepts in a name.
fn utc(seconds: u64) -> String {
    let (days, time) = (seconds / 86_400, seconds % 86_400);
    let (year, month, day) = civil(days);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        time / 3_600,
        time % 3_600 / 60,
        time % 60
    )
}

/// The proleptic Gregorian date `days` after 1970-01-01, by Howard Hinnant's
/// `civil_from_days`, shifted to count from 0000-03-01 so that every quantity
/// is non-negative: eras of 400 years (146,097 days) and years that begin in
/// March, so the leap day ends each year.
fn civil(days: u64) -> (u64, u64, u64) {
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = era * 400 + year_of_era + u64::from(month <= 2);
    (year, month, day)
}
