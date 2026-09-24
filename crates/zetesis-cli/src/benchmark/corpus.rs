use super::{Completion, Error, ViewOptions};
use clap::{Args, ValueEnum};
use std::{
    io,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    time::Duration,
};
use zetesis_presentation::Layout;
use zetesis_validation::{
    performance::{self, matrix},
    selected,
};

/// Maintained source selections; every cell retains complete selected families.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
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
}
impl From<Suite> for matrix::Suite {
    fn from(value: Suite) -> Self {
        match value {
            Suite::Corpus => Self::Corpus,
            Suite::Baseline => Self::Baseline,
            Suite::Queens => Self::Queens,
            Suite::Series => Self::Series,
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

/// Physical requests supported by the maintained corpus telemetry decoder.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Device {
    /// CPU execution.
    #[default]
    Cpu,
    /// Required physical Metal; absence is retained as a non-pass.
    Metal,
}
impl From<Device> for selected::Backend {
    fn from(value: Device) -> Self {
        match value {
            Device::Cpu => Self::Cpu,
            Device::Metal => Self::Metal,
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
    /// Native executable; omitted uses this installed executable and `solve`.
    #[arg(long)]
    pub zetesis: Option<PathBuf>,
    /// Explicit executable defaults to legacy; the installed executable uses solve.
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
    /// CPU or required Metal execution (the corpus decoder does not yet qualify Vulkan).
    #[arg(long, alias = "backend", value_enum, default_value_t)]
    pub device: Device,
    /// Requested materialization policy.
    #[arg(long, value_enum, default_value_t)]
    pub grounder: Grounder,
    /// Compare eager and lazy native profiles; clingo only qualifies complete answers.
    #[arg(long, conflicts_with = "grounder")]
    pub compare_grounders: bool,
    /// Native candidate/closure workers; auto uses at most four available threads.
    #[arg(long, alias = "workers", value_name = "auto|N", value_parser = crate::options::values::workers, default_value = "auto")]
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
        let profile = selected::NativeExecution {
            backend: self.device.into(),
            grounder: self.grounder.into(),
            workers: self.threads,
            completion_workers: self.completion_workers,
            batch_size: self.batch_size,
            ..selected::NativeExecution::default()
        };
        let profiles = if self.compare_grounders {
            [selected::Grounder::Eager, selected::Grounder::Lazy]
                .map(|grounder| selected::NativeExecution {
                    grounder,
                    ..profile
                })
                .to_vec()
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
        Ok(if self.compare_grounders {
            plan.with_reference(matrix::ReferencePolicy::QualificationOnly)
        } else {
            plan
        })
    }

    /// Actual interface selection. Omission always exercises the explicit solve
    /// command of this installed executable; an explicit legacy binary is opt-in.
    #[must_use]
    pub fn invocation(&self) -> matrix::NativeInvocation {
        self.native_interface.map_or_else(
            || {
                if self.zetesis.is_some() {
                    matrix::NativeInvocation::Legacy
                } else {
                    matrix::NativeInvocation::Solve
                }
            },
            Into::into,
        )
    }
}

pub(super) fn execute(
    options: &CorpusOptions,
    layout: Layout,
    output: &mut impl io::Write,
    diagnostics: &mut impl io::Write,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Completion, Error> {
    let current = std::env::current_exe().map_err(Error::Io)?;
    let native = executable(options.zetesis.as_deref().unwrap_or(&current))?;
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
    let report = performance::command::run_with_cancellation(
        &matrix::Request {
            corpus: &options.root,
            native: &native,
            reference: &reference,
            report: &options.report,
            plan: options.plan().map_err(Error::Campaign)?,
            limits,
            native_answers,
            max_spelling_bytes: limits.answers.max_input_bytes,
            helper: (options.memory_runs > 0).then_some(current.as_path()),
        },
        options.invocation(),
        cancelled,
    )
    .map_err(Error::Campaign)?;
    report.publish().map_err(Error::Campaign)?;
    super::view::corpus(&report.summary(), options.view.json, layout, output)?;
    Ok(if report.passed() {
        Completion::Passed
    } else {
        Completion::NonPass
    })
}

fn executable(path: &Path) -> Result<PathBuf, Error> {
    let search_path = std::env::var_os("PATH");
    zetesis_validation::process::resolve_executable(path, search_path.as_deref()).map_err(Error::Io)
}
