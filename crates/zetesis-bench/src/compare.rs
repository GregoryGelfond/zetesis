//! The `compare` command: saved reports side by side, as tables, JSON or
//! Markdown. It launches nothing.

use crate::{Completion, Error, ViewOptions};
use clap::Args;
use std::{
    io,
    path::{Path, PathBuf},
};
use zetesis_presentation::Layout;
use zetesis_validation::performance::series::{self, Comparison, ReportSource};

/// Saved reports to compare, in order, and how to show the comparison.
#[derive(Debug, Args)]
pub struct CompareOptions {
    /// Saved reports as `LABEL=PATH`, in comparison order, for example
    /// `baseline=old.json candidate=new.json`.
    #[arg(value_name = "LABEL=PATH", required = true)]
    pub reports: Vec<String>,
    /// Maximum source bytes read from each report (not decoded allocator RSS).
    #[arg(long, default_value_t = 4_294_967_296)]
    pub input_bytes: u64,
    /// Keep the derived comparison as JSON at this new path.
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Print the comparison as Markdown tables: medians, ratios, counters and
    /// a scoreboard against clingo per cell.
    #[arg(long, conflicts_with = "json")]
    pub markdown: bool,
    /// Human or structured presentation.
    #[command(flatten)]
    pub view: ViewOptions,
}

/// Load and compare saved reports without launching a solver or writing output.
///
/// # Errors
/// Refuses malformed arguments, excessive documents or incompatible identities.
pub fn compare(options: &CompareOptions) -> Result<Comparison, Error> {
    let sources: Vec<_> = options
        .reports
        .iter()
        .map(|argument| {
            let (label, path) = argument
                .split_once('=')
                .filter(|(label, path)| !label.is_empty() && !path.is_empty())
                .ok_or_else(|| Error::ReportArgument(argument.clone()))?;
            Ok(ReportSource {
                label,
                path: Path::new(path),
            })
        })
        .collect::<Result<_, Error>>()?;
    series::read_compare(&sources, options.input_bytes).map_err(Error::Comparison)
}

/// Compare the reports, keep the comparison when asked, and write the view.
pub(crate) fn execute(
    options: &CompareOptions,
    layout: Layout,
    output: &mut impl io::Write,
) -> Result<Completion, Error> {
    let comparison = compare(options)?;
    if let Some(path) = &options.output {
        retain(path, &comparison)?;
    }
    if options.markdown {
        output
            .write_all(comparison.markdown().as_bytes())
            .map_err(Error::Io)?;
    } else {
        crate::view::comparison(&comparison, options.view.json, layout, output)?;
    }
    Ok(Completion::Passed)
}

/// Write a derived comparison as JSON to a new file; an existing file is refused.
fn retain(path: &Path, comparison: &Comparison) -> Result<(), Error> {
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
