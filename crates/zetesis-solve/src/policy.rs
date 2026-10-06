//! Ordinary execution policies, independent of argument parsing and presentation.
//!
//! The execution backend is the shared vocabulary [`crate::Backend`].

/// Materialization policy, independent of execution hardware.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Grounder {
    /// Prefer lazy source grounding where admitted, independently of hardware.
    #[default]
    Auto,
    /// Instantiate on demand: source joins without a complete ground rule
    /// store, or, for formula programs, a producer core with eligible
    /// constraints streamed and terminal definitions reconstructed per answer.
    /// A GPU backend checks joined sources in immutable relational rounds; the
    /// hybrid formula schedule runs on the CPU.
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
