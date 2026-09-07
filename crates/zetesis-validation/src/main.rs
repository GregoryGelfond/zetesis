//! Independent full-corpus regression command. Never a production solver path.
#![forbid(unsafe_code)]

mod corpus;
mod execution;
mod normalize;
mod phase;
#[cfg_attr(
    any(target_os = "linux", target_os = "macos"),
    path = "legacy_process.rs"
)]
#[cfg_attr(
    not(any(target_os = "linux", target_os = "macos")),
    path = "process_portable.rs"
)]
mod process;
mod runner;
mod stage;

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
impl NativeBackend {
    const fn label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Auto => "auto",
            Self::Gpu => "gpu",
            Self::Metal => "metal",
            Self::Vulkan => "vulkan",
            Self::Dx12 => "dx12",
            Self::Gl => "gl",
            Self::Nvidia => "nvidia",
        }
    }

    const fn physical(self) -> bool {
        !matches!(self, Self::Cpu | Self::Auto)
    }
}
impl NativeOracle {
    const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Closure => "closure",
            Self::Countermodel => "countermodel",
        }
    }
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

impl Options {
    fn physical_formula(&self) -> bool {
        !self.reference_only
            && self.native_backend.physical()
            && self.native_oracle == NativeOracle::Countermodel
    }

    fn effective_native_stats(&self) -> bool {
        self.native_stats || self.physical_formula() || self.native_completion_workers.get() > 1
    }
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
    if options.timeout_ms == 0 || options.max_output_bytes == 0 {
        return Err("timeout and output ceilings must be positive".into());
    }
    let loaded = corpus::load(options)?;
    let (report, passed) = runner::run(options, &loaded);
    let mut bytes = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
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
