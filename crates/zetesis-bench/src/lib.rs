//! Measure installed zetesis executables: corpus campaigns against clingo,
//! comparisons of saved reports, the instrumented profile matrix and the
//! maintained workload series.
//!
//! zetesis-bench measures the `zetesis` it is given, the installed one by
//! default, as a child process, and links no solver or GPU crate. Reports
//! record the identity of every executable they measure. Campaigns always
//! capture machine answers and statistics; human tables and structured views
//! consume typed observations independently of the solver's answer renderer.
//! The library holds the commands and their orchestration; the binary only
//! calls [`entry`].
#![forbid(unsafe_code)]

mod corpus;
mod perf;
mod process;
mod series;
mod view;

#[cfg(test)]
#[path = "../tests/support/bounded_writer.rs"]
mod test_writer;

use clap::{Args, Parser, Subcommand};
use std::{fmt, io, path::PathBuf, sync::atomic::AtomicBool};
use zetesis_presentation::{ColorMode, Layout, TrackedWriter};
use zetesis_validation::performance::series::{Comparison, ReportSource};

pub use corpus::{CorpusOptions, Grounder, NativeInterface, Suite};
pub use perf::PerfOptions;
pub use process::entry;
pub use series::SeriesOptions;

/// The `zetesis-bench` command line.
#[derive(Debug, Parser)]
#[command(
    name = "zetesis-bench",
    version,
    about = "Measure installed zetesis executables against clingo, and compare saved reports",
    subcommand_required = true
)]
pub struct Cli {
    /// The measurement or comparison to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Independently scoped measurement and comparison commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Compare complete selected answer families, timings and memory with clingo.
    Corpus(Box<CorpusOptions>),
    /// Compare saved reports with matching workload and profile identities.
    Compare(CompareOptions),
    /// Measure a suite with the ordinary campaign or the instrumented profile matrix.
    Perf(Box<PerfOptions>),
    /// Compare published series reports: medians, ratios, counters and a scoreboard.
    Series(SeriesOptions),
}

/// View policy, separate from measurement configuration.
#[derive(Clone, Debug, Args)]
pub struct ViewOptions {
    /// Emit structured JSON instead of human tables.
    #[arg(long, global = true)]
    pub json: bool,
    /// Terminal styling for human tables; JSON never contains styling.
    #[arg(long, global = true, value_enum, default_value_t)]
    pub color: ColorMode,
}

/// Inputs to the maintained bounded report comparison.
#[derive(Debug, Args)]
pub struct CompareOptions {
    /// Labelled report path; repeat in comparison order.
    #[arg(long = "report", value_name = "LABEL=PATH", required = true)]
    pub reports: Vec<String>,
    /// Retain the derived structured comparison at a new path.
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Maximum source bytes read from each report (not decoded allocator RSS).
    #[arg(long, default_value_t = 4_294_967_296)]
    pub report_bytes: u64,
    /// Human or structured presentation.
    #[command(flatten)]
    pub view: ViewOptions,
}

/// Outcome of the requested command, distinct from its individual observations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    /// The measurement qualified, or the comparison view completed successfully.
    Passed,
    /// The completed campaign retained one or more non-passing positions.
    NonPass,
}

/// A command could not complete. Published evidence prefixes remain available.
#[derive(Debug)]
pub enum Error {
    /// Path resolution, process metadata discovery or publication failed.
    Io(io::Error),
    /// Campaign preparation or checked publication failed.
    Campaign(zetesis_validation::performance::Error),
    /// Published input or comparison identity was refused.
    Comparison(zetesis_validation::performance::series::ReadError),
    /// A labelled report argument was malformed.
    ReportArgument(String),
    /// The requested options do not form one campaign.
    Usage(&'static str),
    /// Human table construction failed.
    Table(zetesis_presentation::TableError),
    /// Structured output failed.
    Json(serde_json::Error),
    /// The primary operation failed, followed by another reporting failure.
    Reporting {
        /// Original measurement or publication failure, never discarded.
        primary: Box<Error>,
        /// A later flush or human view failure.
        secondary: Box<Error>,
    },
}
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Campaign(error) => error.fmt(formatter),
            Self::Comparison(error) => error.fmt(formatter),
            Self::ReportArgument(argument) => {
                write!(formatter, "report must be LABEL=PATH: {argument}")
            }
            Self::Usage(reason) => formatter.write_str(reason),
            Self::Table(error) => error.fmt(formatter),
            Self::Json(error) => error.fmt(formatter),
            Self::Reporting { primary, secondary } => write!(
                formatter,
                "{primary}; secondary reporting failure: {secondary}"
            ),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Campaign(error) => Some(error),
            Self::Comparison(error) => Some(error),
            Self::Table(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Reporting { primary, .. } => Some(primary.as_ref()),
            Self::ReportArgument(_) | Self::Usage(_) => None,
        }
    }
}

impl Command {
    /// Requested human styling; the process adapter supplies terminal evidence.
    #[must_use]
    pub const fn color(&self) -> ColorMode {
        match self.view() {
            Some(view) => view.color,
            None => ColorMode::Auto,
        }
    }

    /// Whether the caller requested structured standard output.
    #[must_use]
    pub const fn json(&self) -> bool {
        match self.view() {
            Some(view) => view.json,
            None => false,
        }
    }

    const fn view(&self) -> Option<&ViewOptions> {
        match self {
            Self::Corpus(options) => Some(&options.view),
            Self::Compare(options) => Some(&options.view),
            Self::Perf(_) | Self::Series(_) => None,
        }
    }

    const fn name(&self) -> &'static str {
        match self {
            Self::Corpus(_) => "corpus",
            Self::Compare(_) => "compare",
            Self::Perf(_) => "perf",
            Self::Series(_) => "series",
        }
    }
}

/// Execute one command with injected presentation sinks and terminal layout.
///
/// Corpus execution measures the installed `zetesis` unless `--zetesis` names
/// another executable. Solver and clingo launches use the maintained bounded
/// campaign; sources must already exist locally, and nothing is downloaded. A
/// JSON failure before any stdout write attempt publishes one versioned failure
/// document. No second document is appended after an attempted write, including
/// a writer that returns an error after modifying its sink.
///
/// # Errors
/// Returns typed setup, measurement, comparison or publication errors. Corpus
/// non-passes remain a successful evidence publication with `NonPass`.
pub fn execute(
    command: &Command,
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

/// Execute with caller-owned cancellation of corpus child processes.
///
/// The corpus campaign polls `cancelled`, settles owned children and preserves
/// cancelled and unattempted positions in its report. This function installs no
/// signal handler. Presentation and failure publication follow [`execute`].
///
/// # Errors
/// Returns the same typed setup, measurement and publication errors as [`execute`].
pub fn execute_with_cancellation(
    command: &Command,
    layout: Layout,
    output: &mut impl io::Write,
    diagnostics: &mut impl io::Write,
    cancelled: &AtomicBool,
) -> Result<Completion, Error> {
    let mut tracked = TrackedWriter::new(output);
    let result = execute_inner(command, layout, &mut tracked, diagnostics, cancelled);
    let error = match result {
        Ok(completion) => return Ok(completion),
        Err(error) => error,
    };
    if !command.json() || tracked.write_attempted() {
        return Err(error);
    }
    let failure = serde_json::json!({
        "schema": 1, "format": "zetesis-benchmark-failure", "status": "failed",
        "command": command.name(),
        "error": { "kind": error.code(), "detail": error.to_string() },
    });
    let publication = serde_json::to_writer_pretty(&mut tracked, &failure)
        .map_err(Error::Json)
        .and_then(|()| {
            use io::Write as _;
            writeln!(tracked).map_err(Error::Io)
        });
    match publication {
        Ok(()) => Err(error),
        Err(secondary) => Err(Error::Reporting {
            primary: Box::new(error),
            secondary: Box::new(secondary),
        }),
    }
}

impl Error {
    const fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::Campaign(_) => "campaign",
            Self::Comparison(_) => "comparison",
            Self::ReportArgument(_) => "report_argument",
            Self::Usage(_) => "usage",
            Self::Table(_) => "table_shape",
            Self::Json(_) => "json_publication",
            Self::Reporting { .. } => "reporting",
        }
    }
}

fn execute_inner(
    command: &Command,
    layout: Layout,
    output: &mut impl io::Write,
    diagnostics: &mut impl io::Write,
    cancelled: &AtomicBool,
) -> Result<Completion, Error> {
    match command {
        Command::Corpus(options) => {
            corpus::execute(options, layout, output, diagnostics, cancelled)
        }
        Command::Compare(options) => {
            let comparison = compare(options)?;
            if let Some(path) = &options.output {
                retain(path, &comparison)?;
            }
            view::comparison(&comparison, options.view.json, layout, output)?;
            Ok(Completion::Passed)
        }
        Command::Perf(options) => perf::execute(options, output, diagnostics),
        Command::Series(options) => series::execute(options, output),
    }
}

/// This tool as the reports it writes name it.
pub(crate) fn tool() -> zetesis_validation::performance::matrix::Tool {
    zetesis_validation::performance::matrix::Tool {
        name: "zetesis-bench".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

/// Load and compare retained reports without launching a solver or writing output.
///
/// # Errors
/// Refuses malformed arguments, excessive documents or incompatible identities.
pub fn compare(options: &CompareOptions) -> Result<Comparison, Error> {
    read_compare(&options.reports, options.report_bytes)
}

/// Read labelled `LABEL=PATH` reports, each of at most `report_bytes` source
/// bytes, and compare them in the given order.
fn read_compare(reports: &[String], report_bytes: u64) -> Result<Comparison, Error> {
    let sources: Vec<_> = reports
        .iter()
        .map(|argument| {
            let (label, path) = argument
                .split_once('=')
                .filter(|(label, path)| !label.is_empty() && !path.is_empty())
                .ok_or_else(|| Error::ReportArgument(argument.clone()))?;
            Ok(ReportSource {
                label,
                path: std::path::Path::new(path),
            })
        })
        .collect::<Result<_, Error>>()?;
    zetesis_validation::performance::series::read_compare(&sources, report_bytes)
        .map_err(Error::Comparison)
}

/// Write a derived comparison as JSON to a new file; an existing file is refused.
fn retain(path: &std::path::Path, comparison: &Comparison) -> Result<(), Error> {
    use io::Write as _;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(Error::Io)?;
    serde_json::to_writer_pretty(&mut file, comparison).map_err(Error::Json)?;
    writeln!(file).map_err(Error::Io)?;
    file.flush().map_err(Error::Io)
}
