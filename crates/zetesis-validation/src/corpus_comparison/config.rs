//! Configuration of the fixed non-clingcon corpus comparison.

use super::NativeOracle;
use std::num::NonZeroUsize;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// Requested native hardware policy; observed execution is checked separately.
pub enum NativeBackend {
    #[default]
    /// Request CPU execution.
    Cpu,
    /// Let the native solver choose the applicable policy.
    Auto,
    /// Request an available physical GPU.
    Gpu,
    /// Request a Metal adapter.
    Metal,
    /// Request a Vulkan adapter.
    Vulkan,
    /// Request a DirectX 12 adapter.
    Dx12,
    /// Request an OpenGL adapter.
    Gl,
    /// Request an NVIDIA adapter.
    Nvidia,
}
impl NativeBackend {
    /// Stable command/report spelling.
    #[must_use]
    pub const fn label(self) -> &'static str {
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

    pub(super) const fn physical(self) -> bool {
        !matches!(self, Self::Cpu | Self::Auto)
    }
}

/// A complete comparison request, without publication or progress destinations.
///
/// If neither source override is present, `repo/examples/kr-domains` is used.
/// Either override selects the original-source manifest mode, with the missing
/// path derived from `repo`. There is no fallback between source views.
#[derive(Clone, Debug)]
pub struct Request {
    /// Repository containing the self-contained examples/kr-domains collection.
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
    pub native_backend: NativeBackend,
    /// Positive native candidate batch size, recorded and passed to zetesis.
    pub native_batch_size: NonZeroUsize,
    /// Positive exact formula completion worker request, separate from closure workers.
    pub native_completion_workers: NonZeroUsize,
    /// Logical completion-batch scratch bytes; zero is a valid native refusal budget.
    pub native_max_completion_scratch_bytes: u64,
    /// Capture native statistics; also enabled for physical formula campaigns
    /// or multiple requested completion workers.
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
            && self.native_backend.physical()
            && self.native_oracle == NativeOracle::Countermodel
    }

    pub(super) fn effective_native_stats(&self) -> bool {
        self.native_stats || self.physical_formula() || self.native_completion_workers.get() > 1
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
            native_backend: NativeBackend::Cpu,
            native_batch_size: NonZeroUsize::new(64).unwrap(),
            native_completion_workers: NonZeroUsize::new(1).unwrap(),
            native_max_completion_scratch_bytes: 268_435_456,
            native_stats: false,
            reference_only: false,
            timeout_ms: 30_000,
            max_output_bytes: 8_388_608,
        }
    }
}
