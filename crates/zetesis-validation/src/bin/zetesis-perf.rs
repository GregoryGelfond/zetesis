//! Thin installed view of the bounded ordinary CPU comparison library.
use clap::Parser;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use zetesis_validation::performance::{self, Limits, Request, Schedule};

#[derive(Parser)]
#[command(
    version,
    about = "Compare three pinned clean examples using complete CPU answer-set families"
)]
struct Options {
    /// Self-contained clean examples/kr-domains directory.
    root: PathBuf,
    /// Native zetesis executable path.
    #[arg(long)]
    zetesis: PathBuf,
    /// Independent clingo executable path.
    #[arg(long)]
    clingo: PathBuf,
    /// New evidence path; existing files are never replaced.
    #[arg(long)]
    report: PathBuf,
    /// Timed pairs per case, following successful qualification and warmups.
    #[arg(long, default_value_t = 21)]
    repetitions: usize,
    /// Untimed preparation pairs per case, zero through five.
    #[arg(long, default_value_t = 3)]
    warmups: usize,
    /// Per-invocation timeout in seconds.
    #[arg(long, default_value_t = 30)]
    timeout_seconds: u64,
    /// Total campaign scheduling deadline in seconds.
    #[arg(long, default_value_t = 180)]
    campaign_seconds: u64,
}
fn execute(options: Options) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let native = std::path::absolute(options.zetesis)?;
    let reference = std::path::absolute(options.clingo)?;
    let mut limits = Limits::default();
    limits.process.timeout = Duration::from_secs(options.timeout_seconds);
    limits.campaign_timeout = Duration::from_secs(options.campaign_seconds);
    let report = performance::run(&Request {
        corpus: &options.root,
        native: &native,
        reference: &reference,
        report: &options.report,
        schedule: Schedule::new(options.warmups, options.repetitions)?,
        limits,
    })?;
    report.publish()?;
    writeln!(
        io::stdout().lock(),
        "{}: {} retained observations; evidence {}",
        if report.passed() { "pass" } else { "fail" },
        report.samples().len(),
        options.report.display()
    )?;
    Ok(if report.passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
fn main() -> ExitCode {
    match execute(Options::parse()) {
        Ok(code) => code,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "zetesis-perf: {error}");
            ExitCode::from(2)
        }
    }
}
