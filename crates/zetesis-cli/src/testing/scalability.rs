//! Argument mapping for the shared authored workload qualification campaign.

use std::{io, num::NonZeroUsize, path::PathBuf, sync::atomic::AtomicBool, time::Duration};

use clap::Args;
use zetesis_presentation::Layout;
use zetesis_validation::{
    performance::{self, matrix, scalability},
    selected::{FormulaJoins, NativeExecution, SearchMethod},
};

use super::{Completion, Error, ProcessOptions, ViewOptions};

/// Complete-family checks of the maintained scalability workloads.
/// A new --report path is required for the complete sealed evidence.
#[derive(Debug, Args)]
#[command(group(clap::ArgGroup::new("scalability_report").args(["report"]).required(true)))]
pub struct ScalabilityOptions {
    /// Verified correctness catalog used by the three unchanged corpus cases.
    #[arg(default_value = "examples/correctness")]
    pub root: PathBuf,
    /// Authored examples root; metadata and source digests are checked.
    #[arg(long, default_value = "examples")]
    pub examples: PathBuf,
    /// Include the unchanged Einstein riddle as a tenth workload.
    #[arg(long)]
    pub include_einstein: bool,
    /// Modern zetesis executable; omitted uses this installed executable's solve.
    #[arg(long)]
    pub zetesis: Option<PathBuf>,
    /// External clingo used for one complete qualification per workload.
    #[arg(long, default_value = "clingo")]
    pub clingo: PathBuf,
    /// CPU region thread counts, one through eight profiles, each at most 256.
    #[arg(long, value_delimiter = ',', num_args = 1.., default_value = "1,2,4,8,14")]
    pub threads: Vec<NonZeroUsize>,
    /// Explicit native grounding expansion ceiling, retained in every profile.
    #[arg(long)]
    pub max_expansion_work: Option<usize>,
    /// Total campaign scheduling deadline, including all qualification children.
    #[arg(long, default_value_t = 1800)]
    pub campaign_seconds: u64,
    /// Combined retained captures across the complete campaign.
    #[arg(long, default_value_t = 512 * 1024 * 1024)]
    pub total_capture_bytes: usize,
    /// Serialized complete evidence ceiling.
    #[arg(long, default_value_t = 1024 * 1024 * 1024)]
    pub report_bytes: usize,
    /// Bounded per-child execution.
    #[command(flatten)]
    pub process: ProcessOptions,
    /// Presentation and required complete evidence destination.
    #[command(flatten)]
    pub view: ViewOptions,
}

impl ScalabilityOptions {
    /// Construct complete-family qualifications without measurement rounds.
    ///
    /// # Errors
    /// Refuses profiles outside the maintained matrix bounds.
    pub fn plan(&self) -> Result<matrix::Plan, performance::Error> {
        let profiles = self
            .threads
            .iter()
            .map(|&workers| NativeExecution {
                workers,
                formula_joins: Some(FormulaJoins::Indexed),
                search: Some(SearchMethod::Regions),
                max_expansion_work: self.max_expansion_work,
                ..NativeExecution::default()
            })
            .collect();
        matrix::Plan::qualification(matrix::Suite::Scalability, profiles, NonZeroUsize::MIN)
    }
}

pub(super) fn execute(
    options: &ScalabilityOptions,
    layout: Layout,
    output: &mut impl io::Write,
    diagnostics: &mut impl io::Write,
    cancelled: &AtomicBool,
) -> Result<Completion, Error> {
    let current = std::env::current_exe().map_err(Error::Io)?;
    let search_path = std::env::var_os("PATH");
    let resolve = |path| {
        zetesis_validation::process::resolve_executable(path, search_path.as_deref())
            .map_err(Error::Io)
    };
    let native = resolve(options.zetesis.as_deref().unwrap_or(&current))?;
    let reference = resolve(&options.clingo)?;
    let destination = options.view.report.as_deref().ok_or(Error::Scalability(
        performance::Error::Configuration(
            "test scalability requires --report with a new evidence path",
        ),
    ))?;
    let mut limits = performance::Limits::default();
    limits.process.timeout = Duration::from_secs(options.process.timeout_seconds);
    limits.process.max_output_bytes = options.process.capture_bytes;
    limits.campaign_timeout = Duration::from_secs(options.campaign_seconds);
    limits.max_total_capture_bytes = options.total_capture_bytes;
    limits.max_report_bytes = options.report_bytes;
    let report = scalability::run_with_cancellation(
        &matrix::Request {
            corpus: &options.root,
            native: &native,
            reference: &reference,
            report: destination,
            plan: options.plan().map_err(Error::Scalability)?,
            limits,
            native_answers: zetesis_validation::answers::native_json::Limits::default(),
            max_spelling_bytes: limits.answers.max_input_bytes,
            helper: None,
        },
        &options.examples,
        options.include_einstein,
        matrix::NativeInvocation::Solve,
        cancelled,
    )
    .map_err(Error::Scalability)?;
    report.publish().map_err(Error::ScalabilityPublication)?;
    super::view::scalability(&report, &options.view, layout, output)?;
    if !report.passed() {
        writeln!(
            diagnostics,
            "Scalability conformance did not pass; inspect the retained outcomes in {}.",
            destination.display()
        )
        .map_err(Error::Io)?;
    }
    Ok(super::completion(report.passed()))
}
