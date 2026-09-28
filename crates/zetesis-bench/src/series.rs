//! The `series` command: a Markdown view over published matrix reports of the
//! same series cells.

use crate::{Completion, Error};
use clap::Args;
use std::io::Write;
use std::path::PathBuf;

/// Published series reports to compare, in order.
#[derive(Debug, Args)]
pub struct SeriesOptions {
    /// `LABEL=PATH` of a published report; repeat in comparison order, for
    /// example `main=…`, `before=…`, `after=…`.
    #[arg(long = "report", value_name = "LABEL=PATH", required = true)]
    reports: Vec<String>,
    /// Write the derived comparison as JSON to this new path.
    #[arg(long)]
    json: Option<PathBuf>,
    /// Maximum bytes read from one report file.
    #[arg(long, default_value_t = 4_294_967_296)]
    report_bytes: u64,
}

/// Compare the reports: medians, ratios, counters and a scoreboard against
/// the reference per cell, written to `output` as Markdown.
pub(crate) fn execute(
    options: &SeriesOptions,
    output: &mut impl Write,
) -> Result<Completion, Error> {
    let comparison = crate::read_compare(&options.reports, options.report_bytes)?;
    if let Some(path) = &options.json {
        crate::retain(path, &comparison)?;
    }
    output
        .write_all(comparison.markdown().as_bytes())
        .map_err(Error::Io)?;
    Ok(Completion::Passed)
}
