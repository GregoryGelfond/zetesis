//! Ordinary execution policies, independent of argument parsing and presentation.

/// Execution policy. Explicit GPU requests require a real selected device.
/// General formulas use GPU propagation with exact native CPU residual search.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backend {
    /// CPU formula search; closure may use GPU batches of 32 or more after its first seed.
    #[default]
    Auto,
    /// Source joins or static closure scans on an owned Rayon pool.
    Cpu,
    /// Exact integer GPU batches, including explicit lazy relational execution.
    Gpu,
    /// Require a physical GPU using Metal.
    Metal,
    /// Require a physical GPU using Vulkan.
    Vulkan,
    /// Require a physical GPU using DirectX 12.
    Dx12,
    /// Require a physical GPU using OpenGL or OpenGL ES.
    Gl,
    /// Require an NVIDIA GPU through a compiled graphics API; this is not CUDA.
    Nvidia,
}

impl Backend {
    /// Provisional minimum population for a delayed automatic GPU attempt.
    /// This scheduling heuristic is not a measured performance crossover.
    pub const AUTO_GPU_MIN_BATCH: usize = 32;

    /// Stable spelling for configuration and machine-readable execution reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Metal => "metal",
            Self::Vulkan => "vulkan",
            Self::Dx12 => "dx12",
            Self::Gl => "gl",
            Self::Nvidia => "nvidia",
        }
    }
}

/// Materialization policy, independent of execution hardware.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Grounder {
    /// Prefer lazy source grounding where admitted, independently of hardware.
    #[default]
    Auto,
    /// Require source joins without materializing a complete ground rule store.
    /// Explicit GPU requests use immutable relational rounds; Auto may discover a device after the first seed.
    Lazy,
    /// Materialize a bounded static program before checking on CPU or GPU.
    Eager,
}

impl Grounder {
    /// Stable spelling for configuration and execution reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Lazy => "lazy",
            Self::Eager => "eager",
        }
    }
}

/// Relational CPU source traversal across candidate occurrences.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceBatching {
    /// Each candidate owns an independent relational join traversal.
    #[default]
    Independent,
    /// Share the union carrier; evaluate each frozen candidate on Rayon.
    Union,
    /// Prune source prefixes with per-world membership; evaluate on Rayon.
    Worlds,
}

impl SourceBatching {
    /// Stable policy spelling for diagnostics and machine-readable reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Independent => "independent",
            Self::Union => "union",
            Self::Worlds => "worlds",
        }
    }

    pub(crate) const fn selection(self) -> Option<zetesis_cpu::lazy::SourceSelection> {
        match self {
            Self::Independent => None,
            Self::Union => Some(zetesis_cpu::lazy::SourceSelection::Union),
            Self::Worlds => Some(zetesis_cpu::lazy::SourceSelection::Worlds),
        }
    }
}

/// Exact stable-model oracle selection, independent of language support.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Oracle {
    /// Select reduct closure, checked tight support, or general reduct checking.
    #[default]
    Auto,
    /// Require reduct closure with sparse gate candidates on CPU or static GPU batches.
    Closure,
    /// Require eager Ferraris search: CPU, or GPU propagation with exact CPU residuals.
    Countermodel,
}

impl Oracle {
    /// Stable spelling for configuration and execution reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Closure => "closure",
            Self::Countermodel => "countermodel",
        }
    }
}
