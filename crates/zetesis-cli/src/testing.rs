//! Command adapters for bounded conformance workflows and typed result views.

mod view;
mod scalability;
pub use scalability::ScalabilityOptions;

use clap::{Args, Subcommand};
use std::{fmt, io, path::PathBuf, sync::atomic::AtomicBool, time::Duration};
use zetesis_presentation::{ColorMode, Layout};
use zetesis_validation::{backend_check, corpus_comparison};

/// Conformance tasks, separate from benchmarking and developer qualification.
#[derive(Debug, Subcommand)]
pub enum TestCommand {
    /// Compare all 94 local corpus cases with external clingo.
    Corpus(CorpusOptions),
    /// Check three fixed full answer families and actual CPU or Metal execution.
    Backend(BackendOptions),
    /// Qualify maintained workloads across CPU thread counts; requires --report.
    Scalability(ScalabilityOptions),
}

/// Test presentation and optional report publication.
#[derive(Debug, Args)]
pub struct ViewOptions {
    /// Emit the workflow's structured report or compact evidence view on stdout.
    #[arg(long)]
    pub json: bool,
    /// Show optional elapsed-time details; off by default.
    /// Backend checks always capture route telemetry internally as check evidence.
    #[arg(long)]
    pub stats: bool,
    /// Human styling; JSON is always plain.
    #[arg(long, value_enum, default_value_t)]
    pub color: ColorMode,
    /// Also retain the structured evidence at a new path; existing files are refused.
    #[arg(long)]
    pub report: Option<PathBuf>,
}

/// Bounded child execution; a process deadline is not a solver work ceiling.
#[derive(Debug, Args)]
pub struct ProcessOptions {
    /// Maximum elapsed seconds for each native or reference child.
    #[arg(long, default_value_t = 30)]
    pub timeout_seconds: u64,
    /// Maximum combined stdout/stderr bytes captured from each child.
    #[arg(long, default_value_t = 8 * 1024 * 1024)]
    pub capture_bytes: usize,
}

/// Local corpus comparison request; sources are verified and never downloaded.
#[derive(Debug, Args)]
pub struct CorpusOptions {
    /// Checkout containing examples/correctness and its pinned manifest.
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    /// External clingo executable, resolved through PATH for a bare name.
    #[arg(long, default_value = "clingo")]
    pub clingo: PathBuf,
    /// Modern zetesis executable; omitted uses the current executable's solve command.
    #[arg(long)]
    pub zetesis: Option<PathBuf>,
    /// Execution backend: cpu (the default), gpu, metal or vulkan. GPU checks
    /// use the general eager formula route.
    #[arg(long, value_parser = zetesis_backend::BackendParser, default_value = "cpu")]
    pub backend: backend_check::Backend,
    /// Per-invocation capture limits.
    #[command(flatten)]
    pub process: ProcessOptions,
    /// Human or structured report view.
    #[command(flatten)]
    pub view: ViewOptions,
}

/// Small installed-command checks, not the 59-test physical qualification suite.
#[derive(Debug, Args)]
pub struct BackendOptions {
    /// Exact CPU or physical GPU route (cpu, gpu, metal or vulkan); no implicit
    /// fallback.
    #[arg(long, value_parser = zetesis_backend::BackendParser, default_value = "cpu")]
    pub backend: backend_check::Backend,
    /// Modern zetesis executable; omitted uses this installed executable.
    #[arg(long)]
    pub zetesis: Option<PathBuf>,
    /// Per-invocation capture limits.
    #[command(flatten)]
    pub process: ProcessOptions,
    /// Human or structured report view.
    #[command(flatten)]
    pub view: ViewOptions,
}

/// Completed test disposition; an explicit nonpass is still a published report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    /// Every requested check passed.
    Passed,
    /// At least one check failed or was not completed.
    NonPass,
}

/// Preparation or presentation failure, separate from reported check nonpasses.
#[derive(Debug)]
pub enum Error {
    /// Source/executable resolution or a supplied writer failed.
    Io(io::Error),
    /// Corpus preparation failed before per-case results existed.
    Corpus(corpus_comparison::Error),
    /// Backend preparation failed before per-case results existed.
    Backend(backend_check::Error),
    /// Scalability workload admission or qualification preparation failed.
    Scalability(zetesis_validation::performance::Error),
    /// Complete scalability evidence could not be published after the campaign.
    ScalabilityPublication(zetesis_validation::performance::Error),
    /// Structured publication failed.
    Json(serde_json::Error),
    /// Human table shape was invalid.
    Table(zetesis_presentation::TableError),
    /// The requested seconds cannot be represented as corpus milliseconds.
    DurationOverflow,
    /// The primary command failure and a later failure-document write error.
    FailurePublication {
        /// Original command error.
        primary: Box<Error>,
        /// Failure while publishing its structured diagnostic.
        publication: Box<Error>,
    },
}
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Corpus(error) => error.fmt(formatter),
            Self::Backend(error) => error.fmt(formatter),
            Self::Scalability(error) | Self::ScalabilityPublication(error) => error.fmt(formatter),
            Self::Json(error) => error.fmt(formatter),
            Self::Table(error) => error.fmt(formatter),
            Self::DurationOverflow => {
                formatter.write_str("test timeout exceeds the supported millisecond range")
            }
            Self::FailurePublication {
                primary,
                publication,
            } => write!(
                formatter,
                "{primary}; failure report publication also failed: {publication}"
            ),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Corpus(error) => Some(error),
            Self::Backend(error) => Some(error),
            Self::Scalability(error) | Self::ScalabilityPublication(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Table(error) => Some(error),
            Self::DurationOverflow => None,
            Self::FailurePublication { primary, .. } => Some(primary.as_ref()),
        }
    }
}

impl TestCommand {
    /// Human styling policy; terminal evidence is supplied by the process owner.
    #[must_use]
    pub const fn color(&self) -> ColorMode {
        self.view().color
    }
    /// Whether standard output is a structured report.
    #[must_use]
    pub const fn json(&self) -> bool {
        self.view().json
    }
    const fn view(&self) -> &ViewOptions {
        match self {
            Self::Corpus(options) => &options.view,
            Self::Backend(options) => &options.view,
            Self::Scalability(options) => &options.view,
        }
    }
}

/// Run the requested typed workflow and publish through injected writers.
///
/// Failures before any stdout write attempt receive a complete JSON failure
/// document when requested. A publication failure can leave a partial document;
/// no second document is appended. Child nonpasses return `NonPass` with a report.
/// No global stdout/stderr or compatibility executable forwarding is used.
///
/// # Errors
/// Returns input/setup or publication failure; completed failed checks are values.
pub fn execute(
    command: &TestCommand,
    layout: Layout,
    output: &mut impl io::Write,
    diagnostics: &mut impl io::Write,
) -> Result<Completion, Error> {
    execute_with_cancellation(
        command,
        layout,
        output,
        diagnostics,
        &AtomicBool::new(false),
    )
}

/// Execute tests with an explicit caller-owned cancellation flag.
///
/// Active process capture performs bounded cleanup before returning. Later
/// children are not launched once cancellation is observed. Partial checks are
/// published as nonpassing evidence; this adapter installs no signal handlers.
///
/// # Errors
/// Returns the setup/publication errors of [`execute`], preserving a primary
/// failure if publishing its structured failure document also fails.
pub fn execute_with_cancellation(
    command: &TestCommand,
    layout: Layout,
    output: &mut impl io::Write,
    diagnostics: &mut impl io::Write,
    cancelled: &AtomicBool,
) -> Result<Completion, Error> {
    let mut tracked = crate::presentation::TrackedWriter::new(output);
    let result = execute_inner(command, layout, &mut tracked, diagnostics, cancelled);
    let error = match result {
        Ok(completion) => return Ok(completion),
        Err(error) => error,
    };
    if command.json() && !tracked.write_attempted() {
        let task = match command {
            TestCommand::Corpus(_) => "corpus",
            TestCommand::Backend(_) => "backend",
            TestCommand::Scalability(_) => "scalability",
        };
        let status = match error {
            Error::Corpus(_)
            | Error::Backend(_)
            | Error::Scalability(_)
            | Error::DurationOverflow => "preparation_failed",
            _ => "failed",
        };
        let failure = serde_json::json!({"schema":1,"format":"zetesis-test-failure","task":task,"status":status,"kind":error.code(),"detail":error.to_string()});
        if let Err(publication) = view::json(&failure, &mut tracked) {
            return Err(Error::FailurePublication {
                primary: Box::new(error),
                publication: Box::new(publication),
            });
        }
    }
    Err(error)
}

fn execute_inner(
    command: &TestCommand,
    layout: Layout,
    output: &mut impl io::Write,
    diagnostics: &mut impl io::Write,
    cancelled: &AtomicBool,
) -> Result<Completion, Error> {
    match command {
        TestCommand::Scalability(options) => {
            scalability::execute(options, layout, output, diagnostics, cancelled)
        }
        TestCommand::Corpus(options) => {
            let report = corpus(options, cancelled)?;
            retain(&options.view, |file| {
                view::json(&report.to_json().map_err(Error::Json)?, file)
            })?;
            view::corpus(&report, &options.view, layout, output)?;
            if !report.passed() {
                writeln!(
                    diagnostics,
                    "Corpus comparison did not pass; inspect the reported case outcomes."
                )
                .map_err(Error::Io)?;
            }
            Ok(completion(report.passed()))
        }
        TestCommand::Backend(options) => {
            let report = backend(options, cancelled)?;
            retain(&options.view, |file| view::backend_json(&report, file))?;
            view::backend(&report, &options.view, layout, output)?;
            if !report.passed() {
                writeln!(
                    diagnostics,
                    "Backend checks did not pass; this does not establish backend qualification."
                )
                .map_err(Error::Io)?;
            }
            Ok(completion(report.passed()))
        }
    }
}

fn completion(passed: bool) -> Completion {
    if passed {
        Completion::Passed
    } else {
        Completion::NonPass
    }
}

fn executable(path: Option<&PathBuf>) -> Result<PathBuf, Error> {
    path.map_or_else(std::env::current_exe, std::path::absolute)
        .map_err(Error::Io)
}

fn corpus(
    options: &CorpusOptions,
    cancelled: &AtomicBool,
) -> Result<corpus_comparison::Report, Error> {
    let request = corpus_comparison::Request {
        repo: options.repo.clone(),
        clingo: options.clingo.clone(),
        zetesis: executable(options.zetesis.as_ref())?,
        native_backend: options.backend,
        native_oracle: if options.backend.is_gpu() {
            corpus_comparison::NativeOracle::Countermodel
        } else {
            corpus_comparison::NativeOracle::Auto
        },
        native_stats: options.view.stats,
        timeout_ms: options
            .process
            .timeout_seconds
            .checked_mul(1000)
            .ok_or(Error::DurationOverflow)?,
        max_output_bytes: options.process.capture_bytes,
        ..corpus_comparison::Request::default()
    };
    corpus_comparison::run_with_cancellation(
        &request,
        corpus_comparison::NativeInvocation::Solve,
        cancelled,
        |_| {},
    )
    .map_err(Error::Corpus)
}

fn backend(
    options: &BackendOptions,
    cancelled: &AtomicBool,
) -> Result<backend_check::Report, Error> {
    backend_check::run_with_cancellation(
        &backend_check::Request {
            executable: executable(options.zetesis.as_ref())?,
            backend: options.backend,
            limits: zetesis_validation::process::Limits {
                timeout: Duration::from_secs(options.process.timeout_seconds),
                max_output_bytes: options.process.capture_bytes,
                ..zetesis_validation::process::Limits::default()
            },
        },
        cancelled,
    )
    .map_err(Error::Backend)
}

fn retain(
    view: &ViewOptions,
    write: impl FnOnce(&mut std::fs::File) -> Result<(), Error>,
) -> Result<(), Error> {
    use io::Write as _;

    if let Some(path) = &view.report {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(Error::Io)?;
        write(&mut file)?;
        file.flush().map_err(Error::Io)?;
    }
    Ok(())
}

impl Error {
    const fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::Corpus(_) => "corpus_preparation",
            Self::Backend(_) => "backend_preparation",
            Self::Scalability(_) => "scalability_preparation",
            Self::ScalabilityPublication(_) => "scalability_publication",
            Self::Json(_) => "json_publication",
            Self::Table(_) => "table_shape",
            Self::DurationOverflow => "duration_overflow",
            Self::FailurePublication { .. } => "failure_publication",
        }
    }
}
