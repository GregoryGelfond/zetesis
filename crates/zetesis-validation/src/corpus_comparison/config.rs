//! Configuration of the fixed non-clingcon corpus comparison.

use super::NativeOracle;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use zetesis_backend::Backend;

/// Native command protocol; the default preserves historical executables.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeInvocation {
    /// File-first command and checked human answer records.
    #[default]
    Legacy,
    /// Explicit `solve` command and checked schema-1/schema-2 JSON records.
    Solve,
}

/// A complete comparison request, without publication or progress destinations.
///
/// If neither source override is present, `repo/examples/correctness` is used.
/// Either override selects the original-source manifest mode, with the missing
/// path derived from `repo`. There is no fallback between source views.
#[derive(Clone, Debug)]
pub struct Request {
    /// Repository containing the self-contained examples/correctness collection.
    pub repo: PathBuf,
    /// Select an original-source corpus directory using historical manifest mode.
    pub corpus: Option<PathBuf>,
    /// Select a historical original-source manifest instead of clean examples.
    pub manifest: Option<PathBuf>,
    /// Independent clingo executable, resolved through PATH if not absolute.
    pub clingo: PathBuf,
    /// Native zetesis executable; never replaced by the reference solver.
    pub zetesis: PathBuf,
    /// Native reduct oracle policy, recorded in the report and invocation.
    pub native_oracle: NativeOracle,
    /// Native hardware policy; explicit GPU policies are passed through unchanged.
    pub native_backend: Backend,
    /// Requested native worker threads.
    pub native_threads: NonZeroUsize,
    /// Explicit named-memory allowance; absent preserves the executable default.
    pub native_memory_bytes: Option<u64>,
    /// Cooperative solver deadline, separate from the harness child timeout.
    pub native_time_limit_seconds: Option<std::num::NonZeroU64>,
    /// Capture native statistics; also enabled for physical formula campaigns.
    pub native_stats: bool,
    /// Run/check the reference only; success does not establish native support.
    pub reference_only: bool,
    /// Maximum elapsed milliseconds per child invocation.
    pub timeout_ms: u64,
    /// Maximum combined stdout/stderr bytes retained per child invocation.
    pub max_output_bytes: usize,
}

impl Request {
    pub(super) fn physical_formula(&self) -> bool {
        !self.reference_only
            && self.native_backend.is_gpu()
            && self.native_oracle == NativeOracle::Countermodel
    }

    pub(super) fn effective_native_stats(&self) -> bool {
        self.native_stats || self.physical_formula()
    }
}

impl Default for Request {
    fn default() -> Self {
        Self {
            repo: PathBuf::from("."),
            corpus: None,
            manifest: None,
            clingo: PathBuf::from("clingo"),
            zetesis: PathBuf::from("zetesis"),
            native_oracle: NativeOracle::Auto,
            native_backend: Backend::Cpu,
            native_threads: NonZeroUsize::MIN,
            native_memory_bytes: None,
            native_time_limit_seconds: None,
            native_stats: false,
            reference_only: false,
            timeout_ms: 30_000,
            max_output_bytes: 8_388_608,
        }
    }
}
