//! Independent full-corpus regression command. Never a production solver path.
#![forbid(unsafe_code)]

use zetesis_validation::corpus_comparison;

use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
enum NativeOracle {
    #[default]
    Auto,
    Closure,
    Countermodel,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
enum NativeBackend {
    #[default]
    Cpu,
    Auto,
    Gpu,
    Metal,
    Vulkan,
    Dx12,
    Gl,
    Nvidia,
}
#[derive(Debug, Parser)]
#[command(
    name = "zetesis-validate",
    version,
    about = "Validate the complete pinned non-clingcon kr-domains target"
)]
struct Options {
    /// Repository containing the self-contained examples/kr-domains collection.
    #[arg(long, default_value = ".")]
    repo: PathBuf,
    /// Select an original-source corpus directory using historical manifest mode.
    #[arg(long)]
    corpus: Option<PathBuf>,
    /// Select a historical original-source manifest instead of clean examples.
    #[arg(long)]
    manifest: Option<PathBuf>,
    /// Independent clingo executable, resolved through PATH if not absolute.
    #[arg(long, default_value = "clingo")]
    clingo: PathBuf,
    /// Native zetesis executable; never replaced by the reference solver.
    #[arg(long, default_value = "zetesis")]
    zetesis: PathBuf,
    /// Native reduct oracle policy, recorded in the report and invocation.
    #[arg(long, value_enum, default_value_t)]
    native_oracle: NativeOracle,
    /// Native hardware policy; explicit GPU policies are passed through unchanged.
    #[arg(long, value_enum, default_value_t)]
    native_backend: NativeBackend,
    /// Positive native candidate batch size, recorded and passed to zetesis.
    #[arg(long, default_value = "64")]
    native_batch_size: NonZeroUsize,
    /// Positive exact formula completion worker request, separate from closure workers.
    #[arg(long, default_value = "1")]
    native_completion_workers: NonZeroUsize,
    /// Logical completion-batch scratch bytes; zero is a valid native refusal budget.
    #[arg(long, default_value_t = 268_435_456)]
    native_max_completion_scratch_bytes: u64,
    /// Capture native statistics; also enabled for physical formula campaigns
    /// or multiple requested completion workers.
    #[arg(long)]
    native_stats: bool,
    /// Run/check the reference only; success does not establish native support.
    #[arg(long)]
    reference_only: bool,
    /// Write the JSON report here; omit to write JSON to stdout.
    #[arg(long)]
    report: Option<PathBuf>,
    /// Maximum elapsed milliseconds per child invocation.
    #[arg(long, default_value_t = 30_000)]
    timeout_ms: u64,
    /// Maximum combined stdout/stderr bytes retained per child invocation.
    #[arg(long, default_value_t = 8_388_608)]
    max_output_bytes: usize,
}

fn main() -> ExitCode {
    let options = Options::parse();
    match execute(&options) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "zetesis-validate: {error}");
            ExitCode::from(2)
        }
    }
}

fn execute(options: &Options) -> Result<bool, String> {
    let request = corpus_comparison::Request {
        repo: options.repo.clone(),
        corpus: options.corpus.clone(),
        manifest: options.manifest.clone(),
        clingo: options.clingo.clone(),
        zetesis: options.zetesis.clone(),
        native_oracle: options.native_oracle.into(),
        native_backend: options.native_backend.into(),
        native_batch_size: options.native_batch_size,
        native_completion_workers: options.native_completion_workers,
        native_max_completion_scratch_bytes: options.native_max_completion_scratch_bytes,
        native_stats: options.native_stats,
        reference_only: options.reference_only,
        timeout_ms: options.timeout_ms,
        max_output_bytes: options.max_output_bytes,
    };
    let report = corpus_comparison::run(&request, |case| {
        eprintln!("{}: {}", case.decision().label(), case.path());
    })
    .map_err(|error| error.to_string())?;
    let passed = report.passed();
    let mut bytes =
        serde_json::to_vec_pretty(&report.to_json().map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    if let Some(path) = &options.report {
        std::fs::write(path, bytes)
            .map_err(|error| format!("report {}: {error}", path.display()))?;
    } else {
        io::stdout()
            .lock()
            .write_all(&bytes)
            .map_err(|error| error.to_string())?;
    }
    Ok(passed)
}

impl From<NativeOracle> for corpus_comparison::NativeOracle {
    fn from(value: NativeOracle) -> Self {
        match value {
            NativeOracle::Auto => Self::Auto,
            NativeOracle::Closure => Self::Closure,
            NativeOracle::Countermodel => Self::Countermodel,
        }
    }
}

impl From<NativeBackend> for corpus_comparison::NativeBackend {
    fn from(value: NativeBackend) -> Self {
        match value {
            NativeBackend::Cpu => Self::Cpu,
            NativeBackend::Auto => Self::Auto,
            NativeBackend::Gpu => Self::Gpu,
            NativeBackend::Metal => Self::Metal,
            NativeBackend::Vulkan => Self::Vulkan,
            NativeBackend::Dx12 => Self::Dx12,
            NativeBackend::Gl => Self::Gl,
            NativeBackend::Nvidia => Self::Nvidia,
        }
    }
}
