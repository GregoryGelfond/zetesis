//! Benchmark command adapters over reusable measurement and comparison libraries.
//!
//! Corpus measurements always capture machine answers and statistics. Human
//! tables and structured views consume typed observations independently of the
//! solver's answer renderer. Compatibility executables are never invoked as
//! wrappers; only the benchmark's bounded solver/reference children are launched.

mod corpus;
mod view;
#[cfg(feature = "gpu")]
mod primitives;

use clap::{Args, Subcommand};
use std::{fmt, io, path::PathBuf, sync::atomic::AtomicBool};
use zetesis_presentation::{ColorMode, Layout};
use zetesis_validation::performance::series;

pub use corpus::{CorpusOptions, Grounder, NativeInterface, Suite};
#[cfg(feature = "gpu")]
pub use primitives::{Primitive, PrimitiveOptions};

/// Independently scoped benchmark commands, with no optional statistics switch.
#[derive(Debug, Subcommand)]
pub enum BenchCommand {
    /// Compare complete selected answer families, timings and memory with clingo.
    Corpus(Box<CorpusOptions>),
    /// Measure matched relation, aggregate, tight or lazy execution primitives.
    Primitives(PrimitiveOptions),
    /// Compare retained reports with matching workload/profile identities.
    Compare(CompareOptions),
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

/// CPU-only builds expose an explicit capability refusal rather than loading
/// the experiment crate, whose primitive profiles currently require wgpu.
#[cfg(not(feature = "gpu"))]
#[derive(Debug, Args)]
pub struct PrimitiveOptions {
    /// Requested profile arguments, retained only to report the unavailable capability.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub arguments: Vec<std::ffi::OsString>,
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
    Comparison(series::ReadError),
    /// A labelled report argument was malformed.
    ReportArgument(String),
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
    /// The installed build does not include the requested capability.
    Unavailable(&'static str),
    /// A primitive measurement or its observer failed.
    #[cfg(feature = "gpu")]
    Primitive(zetesis_experiments::command::Error),
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
            Self::Table(error) => error.fmt(formatter),
            Self::Json(error) => error.fmt(formatter),
            Self::Unavailable(reason) => formatter.write_str(reason),
            Self::Reporting { primary, secondary } => write!(
                formatter,
                "{primary}; secondary reporting failure: {secondary}"
            ),
            #[cfg(feature = "gpu")]
            Self::Primitive(error) => error.fmt(formatter),
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
            #[cfg(feature = "gpu")]
            Self::Primitive(error) => Some(error),
            Self::ReportArgument(_) | Self::Unavailable(_) => None,
        }
    }
}
impl BenchCommand {
    /// Requested human styling; the process adapter supplies terminal evidence.
    #[must_use]
    pub const fn color(&self) -> ColorMode {
        self.view().color
    }

    /// Whether the caller requested structured standard output.
    #[must_use]
    pub const fn json(&self) -> bool {
        self.view().json
    }

    const fn view(&self) -> &ViewOptions {
        match self {
            Self::Corpus(options) => &options.view,
            Self::Primitives(options) => &options.view,
            Self::Compare(options) => &options.view,
        }
    }
}

/// Execute one benchmark with injected presentation sinks and terminal layout.
///
/// Corpus execution defaults to this installed executable's explicit `solve`
/// command. Solver and clingo launches use the maintained bounded campaign.
/// Sources must already exist locally; this adapter never downloads inputs.
/// A JSON failure before any stdout write attempt publishes one versioned failure
/// document. No second document is appended after an attempted write, including
/// a writer that returns an error after modifying its sink. Primitive JSON-lines
/// streams retain their existing event prefix and never receive a false Complete.
///
/// # Errors
/// Returns typed setup, incomplete primitive measurement or publication errors.
/// Corpus non-passes remain a successful evidence publication with `NonPass`.
pub fn execute(
    command: &BenchCommand,
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
/// cancelled and unattempted positions in its report. This adapter installs no
/// signal handler. Primitive measurements and saved-report comparison do not
/// consume this token. Presentation and failure publication follow [`execute`].
///
/// # Errors
/// Returns the same typed setup, measurement and publication errors as [`execute`].
pub fn execute_with_cancellation(
    command: &BenchCommand,
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
    if !command.json() || tracked.write_attempted() {
        return Err(error);
    }
    let failure = serde_json::json!({
        "schema": 1, "format": "zetesis-benchmark-failure", "status": "failed",
        "command": match command { BenchCommand::Corpus(_) => "corpus", BenchCommand::Primitives(_) => "primitives", BenchCommand::Compare(_) => "compare" },
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
            Self::Table(_) => "table_shape",
            Self::Json(_) => "json_publication",
            Self::Reporting { .. } => "reporting",
            Self::Unavailable(_) => "unavailable",
            #[cfg(feature = "gpu")]
            Self::Primitive(_) => "primitive",
        }
    }
}

fn execute_inner(
    command: &BenchCommand,
    layout: Layout,
    output: &mut impl io::Write,
    diagnostics: &mut impl io::Write,
    cancelled: &AtomicBool,
) -> Result<Completion, Error> {
    match command {
        BenchCommand::Corpus(options) => {
            corpus::execute(options, layout, output, diagnostics, cancelled)
        }
        BenchCommand::Compare(options) => {
            use io::Write as _;

            let comparison = compare(options)?;
            if let Some(path) = &options.output {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(Error::Io)?;
                serde_json::to_writer_pretty(&mut file, &comparison).map_err(Error::Json)?;
                writeln!(file).map_err(Error::Io)?;
                file.flush().map_err(Error::Io)?;
            }
            view::comparison(&comparison, options.view.json, layout, output)?;
            Ok(Completion::Passed)
        }
        #[cfg(feature = "gpu")]
        BenchCommand::Primitives(options) => primitives::execute(options, layout, output),
        #[cfg(not(feature = "gpu"))]
        BenchCommand::Primitives(_) => Err(Error::Unavailable(
            "bench primitives requires a build with the gpu feature; corpus and compare remain available in CPU-only builds",
        )),
    }
}

/// Load and compare retained reports without launching a solver or writing output.
///
/// # Errors
/// Refuses malformed arguments, excessive documents or incompatible identities.
pub fn compare(options: &CompareOptions) -> Result<series::Comparison, Error> {
    let sources: Vec<_> = options
        .reports
        .iter()
        .map(|argument| {
            let (label, path) = argument
                .split_once('=')
                .filter(|(label, path)| !label.is_empty() && !path.is_empty())
                .ok_or_else(|| Error::ReportArgument(argument.clone()))?;
            Ok(series::ReportSource {
                label,
                path: std::path::Path::new(path),
            })
        })
        .collect::<Result<_, Error>>()?;
    series::read_compare(&sources, options.report_bytes).map_err(Error::Comparison)
}
