//! Thin installed views of bounded baseline and instrumented profile comparisons.
use clap::{Parser, ValueEnum};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use zetesis_validation::performance::{self, Limits, Request, Schedule, Suite};

#[derive(Clone, Copy, ValueEnum)]
enum SuiteArgument {
    Baseline,
    Queens,
    Corpus,
}

#[derive(Parser)]
#[command(
    version,
    about = "Compare clean ASP cases with retained parity, timing and execution evidence"
)]
struct Options {
    /// Explicit instrumented profile; repeat for a matrix. Corpus defaults to all four.
    #[arg(long, value_enum)]
    profile: Vec<ProfileArgument>,
    /// Matrix closure workers (default 4; requires --profile or --suite corpus).
    #[arg(long)]
    workers: Option<std::num::NonZeroUsize>,
    /// Matrix formula completion workers (default 4).
    #[arg(long)]
    completion_workers: Option<std::num::NonZeroUsize>,
    /// Reference workers in the instrumented matrix (default 1).
    #[arg(long)]
    clingo_workers: Option<std::num::NonZeroUsize>,
    /// Candidate batch ceiling for matrix native profiles (default 64).
    #[arg(long)]
    batch_size: Option<std::num::NonZeroUsize>,
    /// Per-invocation stdout/stderr ceiling (default retains the established protocol bound).
    #[arg(long)]
    sample_bytes: Option<usize>,
    /// Matrix native JSON decoder byte ceiling (default 8 MiB; separate from capture).
    #[arg(long)]
    native_report_bytes: Option<usize>,
    /// Combined retained stdout/stderr bytes for the campaign.
    #[arg(long, default_value_t = 134_217_728)]
    capture_bytes: usize,
    /// Serialized evidence ceiling, including all raw captures.
    #[arg(long, default_value_t = 536_870_912)]
    report_bytes: usize,
    /// Self-contained clean examples/kr-domains directory.
    root: PathBuf,
    /// Established CPU baseline, all six original queens encodings, or instrumented full corpus.
    #[arg(long, value_enum, default_value = "baseline")]
    suite: SuiteArgument,
    /// Native zetesis executable path.
    #[arg(long)]
    zetesis: PathBuf,
    /// Independent clingo executable path.
    #[arg(long)]
    clingo: PathBuf,
    /// New evidence path; existing files are never replaced.
    #[arg(long)]
    report: PathBuf,
    /// Timed rounds per case (legacy default 21 pairs; instrumented matrix default 20).
    #[arg(long)]
    repetitions: Option<usize>,
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
    if matches!(options.suite, SuiteArgument::Corpus) || !options.profile.is_empty() {
        return matrix(options);
    }
    if options.workers.is_some()
        || options.completion_workers.is_some()
        || options.clingo_workers.is_some()
        || options.batch_size.is_some()
        || options.native_report_bytes.is_some()
    {
        return Err("matrix controls require --profile or --suite corpus".into());
    }
    let native = std::path::absolute(options.zetesis)?;
    let reference = std::path::absolute(options.clingo)?;
    let mut limits = Limits::default();
    limits.process.timeout = Duration::from_secs(options.timeout_seconds);
    limits.campaign_timeout = Duration::from_secs(options.campaign_seconds);
    if let Some(bytes) = options.sample_bytes {
        limits.process.max_output_bytes = bytes;
    }
    limits.max_total_capture_bytes = options.capture_bytes;
    limits.max_report_bytes = options.report_bytes;
    let report = performance::run(&Request {
        corpus: &options.root,
        native: &native,
        reference: &reference,
        report: &options.report,
        schedule: Schedule::for_suite(
            match options.suite {
                SuiteArgument::Baseline => Suite::Baseline,
                SuiteArgument::Queens => Suite::Queens,
                SuiteArgument::Corpus => return Err("corpus requires matrix dispatch".into()),
            },
            options.warmups,
            options.repetitions.unwrap_or(21),
        )?,
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

#[derive(Clone, Copy, ValueEnum)]
enum ProfileArgument {
    CpuEager,
    CpuLazy,
    MetalEager,
    MetalLazy,
}

fn matrix(options: Options) -> Result<ExitCode, Box<dyn std::error::Error>> {
    use zetesis_validation::{
        performance::matrix,
        selected::{Backend, Grounder, NativeExecution},
    };
    let profiles = if options.profile.is_empty() {
        vec![
            ProfileArgument::CpuEager,
            ProfileArgument::CpuLazy,
            ProfileArgument::MetalEager,
            ProfileArgument::MetalLazy,
        ]
    } else {
        options.profile
    };
    let profiles = profiles
        .into_iter()
        .map(|profile| {
            let (backend, grounder) = match profile {
                ProfileArgument::CpuEager => (Backend::Cpu, Grounder::Eager),
                ProfileArgument::CpuLazy => (Backend::Cpu, Grounder::Lazy),
                ProfileArgument::MetalEager => (Backend::Metal, Grounder::Eager),
                ProfileArgument::MetalLazy => (Backend::Metal, Grounder::Lazy),
            };
            NativeExecution {
                backend,
                grounder,
                workers: options
                    .workers
                    .unwrap_or(std::num::NonZeroUsize::new(4).expect("four is nonzero")),
                completion_workers: options
                    .completion_workers
                    .unwrap_or(std::num::NonZeroUsize::new(4).expect("four is nonzero")),
                batch_size: options
                    .batch_size
                    .unwrap_or(std::num::NonZeroUsize::new(64).expect("64 is nonzero")),
                ..NativeExecution::default()
            }
        })
        .collect();
    let suite = match options.suite {
        SuiteArgument::Baseline => matrix::Suite::Baseline,
        SuiteArgument::Queens => matrix::Suite::Queens,
        SuiteArgument::Corpus => matrix::Suite::Corpus,
    };
    let native = std::path::absolute(options.zetesis)?;
    let reference = std::path::absolute(options.clingo)?;
    let mut limits = Limits::default();
    limits.process.timeout = Duration::from_secs(options.timeout_seconds);
    limits.campaign_timeout = Duration::from_secs(options.campaign_seconds);
    if let Some(bytes) = options.sample_bytes {
        limits.process.max_output_bytes = bytes;
    }
    limits.max_total_capture_bytes = options.capture_bytes;
    limits.max_report_bytes = options.report_bytes;
    let mut native_answers = zetesis_validation::answers::native_json::Limits::default();
    if let Some(bytes) = options.native_report_bytes {
        native_answers.report.max_input_bytes = bytes;
    }
    let plan = matrix::Plan::new(
        suite,
        profiles,
        options
            .clingo_workers
            .unwrap_or(std::num::NonZeroUsize::new(1).expect("one is nonzero")),
        options.warmups,
        options.repetitions.unwrap_or(20),
    )?;
    writeln!(
        io::stderr().lock(),
        "Recording instrumented solver matrix; evidence will be written to {}",
        options.report.display()
    )?;
    let report = matrix::run(&matrix::Request {
        corpus: &options.root,
        native: &native,
        reference: &reference,
        report: &options.report,
        plan,
        limits,
        native_answers,
        max_spelling_bytes: limits.answers.max_input_bytes,
    })?;
    report.publish()?;
    writeln!(
        io::stdout().lock(),
        "{}: {} matrix positions retained; accounted={}; evidence {}",
        if report.passed() {
            "pass"
        } else {
            "non-pass cells retained"
        },
        report.samples().len(),
        report.accounted(),
        options.report.display()
    )?;
    Ok(if report.passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
