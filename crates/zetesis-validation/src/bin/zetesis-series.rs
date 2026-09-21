//! Thin view over published matrix reports of the same series cells.
use clap::Parser;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use zetesis_validation::performance::series::{ReportSource, read_compare};

#[derive(Parser)]
#[command(
    version,
    about = "Compare published series reports: medians, ratios, counters and a scoreboard against the reference per cell"
)]
struct Options {
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

fn execute(options: &Options) -> Result<(), Box<dyn std::error::Error>> {
    let sources: Vec<_> = options
        .reports
        .iter()
        .map(|argument| {
            let (label, path) = argument
                .split_once('=')
                .ok_or("each --report is LABEL=PATH")?;
            Ok(ReportSource {
                label,
                path: std::path::Path::new(path),
            })
        })
        .collect::<Result<_, &'static str>>()?;
    let comparison = read_compare(&sources, options.report_bytes)?;
    if let Some(path) = &options.json {
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        serde_json::to_writer_pretty(&mut output, &comparison)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    io::stdout()
        .lock()
        .write_all(comparison.markdown().as_bytes())?;
    Ok(())
}

fn main() -> ExitCode {
    match execute(&Options::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "zetesis-series: {error}");
            ExitCode::from(2)
        }
    }
}
