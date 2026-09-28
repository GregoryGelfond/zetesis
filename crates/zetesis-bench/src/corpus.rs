//! The `corpus` command: complete answer families, timings and memory of the
//! measured `zetesis` beside clingo's.

use crate::{Completion, Error, ViewOptions};
use clap::{Args, ValueEnum};
use std::{
    io,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    time::Duration,
};
use zetesis_presentation::Layout;
use zetesis_validation::{
    performance::{self, matrix, scalability},
    selected,
};

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
    /// Nine maintained authored/corpus workloads for thread comparisons.
    Scalability,
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

/// Finite corpus campaign; statistics are always part of its protocol.
#[derive(Debug, Args)]
pub struct CorpusOptions {
    /// Existing clean corpus directory from the repository; never downloaded.
    #[arg(default_value = "examples/correctness")]
    pub root: PathBuf,
    /// Maintained workload selection.
    #[arg(long, value_enum, default_value_t)]
    pub suite: Suite,
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
    /// Independent clingo executable path or executable name on PATH.
    #[arg(long, default_value = "clingo")]
    pub clingo: PathBuf,
    /// New complete JSON evidence file, including failures and unlaunched positions.
    #[arg(long)]
    pub report: PathBuf,
    /// Execution backend: cpu (the default), gpu, metal or vulkan. A GPU backend
    /// is required: its absence is retained as a non-pass.
    #[arg(long, value_parser = zetesis_backend::BackendParser, default_value = "cpu")]
    pub backend: selected::Backend,
    /// Requested materialization policy.
    #[arg(long, value_enum, default_value_t)]
    pub grounder: Grounder,
    /// Compare eager and lazy native profiles; clingo only qualifies complete answers.
    #[arg(long, conflicts_with = "grounder")]
    pub compare_grounders: bool,
    /// Compare native thread counts, for example 1,2,4,8,14; clingo only qualifies.
    #[arg(long, value_delimiter = ',', num_args = 1.., conflicts_with_all = ["threads", "compare_grounders"])]
    pub compare_threads: Vec<NonZeroUsize>,
    /// Native candidate/closure workers; auto uses at most four available threads.
    #[arg(long, alias = "workers", value_name = "auto|N", value_parser = zetesis_backend::parse_threads, default_value = "auto")]
    pub threads: NonZeroUsize,
    /// Native exact residual completion workers.
    #[arg(
        long,
        default_value = "1",
        hide_short_help = true,
        help_heading = "Advanced measurement controls"
    )]
    pub completion_workers: NonZeroUsize,
    /// Unmodified clingo worker setting; no search-heuristic tuning.
    #[arg(
        long,
        alias = "clingo-workers",
        default_value = "1",
        hide_short_help = true,
        help_heading = "Advanced measurement controls"
    )]
    pub clingo_threads: NonZeroUsize,
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

impl CorpusOptions {
    /// Map command arguments to the reusable bounded plan; no I/O is performed.
    ///
    /// # Errors
    /// Refuses worker counts, dimensions or schedules outside maintained bounds.
    pub fn plan(&self) -> Result<matrix::Plan, performance::Error> {
        if self.suite != Suite::Scalability && (self.examples.is_some() || self.include_einstein) {
            return Err(performance::Error::Configuration(
                "--examples and --include-einstein require --suite scalability",
            ));
        }
        let profile = selected::NativeExecution {
            backend: self.backend,
            grounder: self.grounder.into(),
            workers: self.threads,
            completion_workers: self.completion_workers,
            batch_size: self.batch_size,
            formula_joins: (self.suite == Suite::Scalability)
                .then_some(selected::FormulaJoins::Indexed),
            search: (self.suite == Suite::Scalability).then_some(selected::SearchMethod::Regions),
            max_expansion_work: self.max_expansion_work,
            ..selected::NativeExecution::default()
        };
        let profiles = if self.compare_grounders {
            [selected::Grounder::Eager, selected::Grounder::Lazy]
                .map(|grounder| selected::NativeExecution {
                    grounder,
                    ..profile
                })
                .to_vec()
        } else if !self.compare_threads.is_empty() {
            self.compare_threads
                .iter()
                .map(|&workers| selected::NativeExecution { workers, ..profile })
                .collect()
        } else {
            vec![profile]
        };
        let plan = matrix::Plan::new(
            self.suite.into(),
            profiles,
            self.clingo_threads,
            self.warmups,
            self.repetitions,
        )?
        .with_memory(self.memory_runs)?;
        Ok(
            if self.compare_grounders || !self.compare_threads.is_empty() {
                plan.with_reference(matrix::ReferencePolicy::QualificationOnly)
            } else {
                plan
            },
        )
    }

    /// Actual interface selection: the explicit `solve` command of whichever
    /// executable is measured, unless `--native-interface legacy` requests an
    /// older binary's flat arguments.
    #[must_use]
    pub fn invocation(&self) -> matrix::NativeInvocation {
        self.native_interface
            .map_or(matrix::NativeInvocation::Solve, Into::into)
    }
}

/// Run the campaign, publish its evidence and write its view to `output`.
///
/// Memory rounds re-execute this process's executable as their measurement
/// helper, so it must answer the helper protocol, as `zetesis-bench` does.
pub(crate) fn execute(
    options: &CorpusOptions,
    layout: Layout,
    output: &mut impl io::Write,
    diagnostics: &mut impl io::Write,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Completion, Error> {
    let current = std::env::current_exe().map_err(Error::Io)?;
    let native = match &options.zetesis {
        Some(path) => executable(path)?,
        None => installed_zetesis(&current)?,
    };
    let reference = executable(&options.clingo)?;
    let mut limits = performance::Limits::default();
    limits.process.timeout = Duration::from_secs(options.timeout_seconds);
    limits.process.max_output_bytes = options.sample_bytes;
    limits.campaign_timeout = Duration::from_secs(options.campaign_seconds);
    limits.max_total_capture_bytes = options.capture_bytes;
    limits.max_report_bytes = options.report_bytes;
    let mut native_answers = zetesis_validation::answers::native_json::Limits::default();
    native_answers.report.max_input_bytes = options.native_report_bytes;
    writeln!(
        diagnostics,
        "Recording benchmark evidence in {}",
        options.report.display()
    )
    .map_err(Error::Io)?;
    let request = matrix::Request {
        corpus: &options.root,
        native: &native,
        reference: &reference,
        report: &options.report,
        plan: options.plan().map_err(Error::Campaign)?,
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
    crate::view::corpus(&report.summary(), options.view.json, layout, output)?;
    Ok(if report.passed() {
        Completion::Passed
    } else {
        Completion::NonPass
    })
}

/// The installed `zetesis`: the one in the directory of `this` executable,
/// else the first on `PATH`.
fn installed_zetesis(this: &Path) -> Result<PathBuf, Error> {
    let beside = this.with_file_name(format!("zetesis{}", std::env::consts::EXE_SUFFIX));
    if beside.is_file() {
        Ok(beside)
    } else {
        executable(Path::new("zetesis"))
    }
}

fn executable(path: &Path) -> Result<PathBuf, Error> {
    let search_path = std::env::var_os("PATH");
    zetesis_validation::process::resolve_executable(path, search_path.as_deref()).map_err(Error::Io)
}
