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
    /// Select a manifest-relative clean corpus case; repeat for an ordinary CPU campaign.
    #[arg(long = "case", conflicts_with_all = ["profile", "workers", "completion_workers", "clingo_workers", "batch_size", "native_report_bytes"])]
    cases: Vec<String>,
    /// Separate child RSS rounds per solver/case, zero through 41 (ordinary CPU only).
    #[arg(long, default_value_t = 0)]
    memory_runs: usize,
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
    /// Established CPU baseline, all six curated queens encodings, or instrumented full corpus.
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
        if !options.cases.is_empty() || options.memory_runs > 0 {
            return Err("--case and --memory-runs require the ordinary CPU campaign".into());
        }
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
    let schedule = if options.cases.is_empty() {
        Schedule::for_suite(
            match options.suite {
                SuiteArgument::Baseline => Suite::Baseline,
                SuiteArgument::Queens => Suite::Queens,
                SuiteArgument::Corpus => return Err("corpus requires matrix dispatch".into()),
            },
            options.warmups,
            options.repetitions.unwrap_or(21),
        )?
    } else {
        if !matches!(options.suite, SuiteArgument::Baseline) {
            return Err("explicit --case cannot be combined with a named nondefault suite".into());
        }
        Schedule::for_cases(
            options.cases,
            options.warmups,
            options.repetitions.unwrap_or(21),
        )?
    }
    .with_memory(options.memory_runs)?;
    let request = Request {
        corpus: &options.root,
        native: &native,
        reference: &reference,
        report: &options.report,
        schedule,
        limits,
    };
    let report = if options.memory_runs > 0 || request.schedule.suite().is_none() {
        performance::run_with_runner(&request, &std::env::current_exe()?)?
    } else {
        performance::run(&request)?
    };
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
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("__measure-child")) {
        return measure_child();
    }
    match execute(Options::parse()) {
        Ok(code) => code,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "zetesis-perf: {error}");
            ExitCode::from(2)
        }
    }
}

#[derive(Parser)]
struct ChildOptions {
    /// Private new resource-record path.
    record: PathBuf,
    /// Absolute solver executable followed by its unmodified arguments.
    #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
    command: Vec<std::ffi::OsString>,
}

fn measure_child() -> ExitCode {
    let options = ChildOptions::parse_from(std::env::args_os().skip(1));
    match child_record(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "zetesis-perf child RSS: {error}");
            ExitCode::from(2)
        }
    }
}

fn child_record(options: ChildOptions) -> Result<(), Box<dyn std::error::Error>> {
    let (executable, arguments) = options.command.split_first().ok_or("missing solver")?;
    // This entry point starts no other children: resource usage belongs solely
    // to this one waited-for solver invocation, excluding the helper itself.
    let record =
        zetesis_validation::process::memory::measure(zetesis_validation::process::Invocation {
            executable: std::path::Path::new(executable),
            arguments,
            directory: &std::env::current_dir()?,
        })?;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(options.record)?;
    serde_json::to_writer(&mut output, &record)?;
    output.flush()?;
    Ok(())
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
