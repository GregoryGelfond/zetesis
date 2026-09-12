//! Execution failures, independent of source loading and answer publication.

use std::fmt;

use crate::{Backend, Grounder, Oracle, PhaseTimings, PreparedProfile, SemanticOutcome, Subject};

/// An unsuccessful preparation or execution operation; never evidence of UNSAT.
#[derive(Debug)]
pub enum SolveError {
    /// The owned CPU pool or batch could not be admitted.
    Batch(zetesis_cpu::BatchError),
    /// The exact-completion worker pool could not be constructed.
    CompletionPool(rayon::ThreadPoolBuildError),
    /// The requested device backend was not compiled.
    BackendUnavailable,
    /// The countermodel oracle requires eager finite formula admission.
    UnsupportedOracle {
        /// Requested execution hardware.
        backend: Backend,
        /// Requested materialization policy.
        grounder: Grounder,
    },
    /// Shared source traversal requires relational lazy CPU execution.
    UnsupportedSourceBatching,
    /// The supplied representation cannot honor the requested policy.
    PreparedInput {
        /// Representation supplied by the caller.
        profile: PreparedProfile,
        /// Requested membership policy.
        oracle: Oracle,
        /// Requested materialization policy.
        grounder: Grounder,
    },
    /// The caller's synchronous observer failed. This cannot trigger fallback.
    ExecutionObservation(Box<dyn std::error::Error + Send + Sync>),
    /// Explicit static grounding was refused.
    Static(zetesis_core::StaticError),
    /// Device capability, submission or result transport failed.
    #[cfg(feature = "gpu")]
    Gpu(zetesis_wgpu::GpuError),
    /// Lazy device execution failed with retained source progress.
    #[cfg(feature = "gpu")]
    LazyGpu(zetesis_cpu::lazy::Failure<zetesis_wgpu::GpuError>),
    /// Cumulative lazy counters cannot represent another batch.
    LazyStatisticsOverflow,
    /// Shared CPU evaluation violated its round protocol.
    SharedCpu(zetesis_cpu::lazy::shared::Cause),
    /// Static closure decoding refused its words or selected-position storage.
    Words(zetesis_core::WordError),
    /// An accepted interpretation could not retain its checked atom selection.
    Model(zetesis_core::ModelError),
    /// An injected checker violated the ordered result-count contract.
    FormulaBatchShape {
        /// Original candidates supplied.
        expected: usize,
        /// Returned verdicts.
        actual: usize,
    },
}

impl fmt::Display for SolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Batch(error) => error.fmt(formatter),
            Self::CompletionPool(error) => write!(formatter, "completion worker pool: {error}"),
            Self::BackendUnavailable => formatter.write_str("GPU support was not compiled"),
            Self::UnsupportedOracle { backend, grounder } => write!(formatter,
                "the countermodel oracle requires eager or automatic grounding; requested {} with {}", backend.label(), grounder.label()),
            Self::UnsupportedSourceBatching => formatter.write_str("shared source batching requires the relational closure route with lazy/auto grounding and cpu/auto backend"),
            Self::PreparedInput { profile, oracle, grounder } => write!(formatter,
                "prepared {profile:?} cannot honor oracle {} with grounder {}", oracle.label(), grounder.label()),
            Self::ExecutionObservation(error) => write!(formatter, "execution observer: {error}"),
            Self::Static(error) => error.fmt(formatter),
            #[cfg(feature = "gpu")]
            Self::Gpu(error) => error.fmt(formatter),
            #[cfg(feature = "gpu")]
            Self::LazyGpu(error) => error.fmt(formatter),
            Self::LazyStatisticsOverflow => formatter.write_str("lazy execution statistics overflow"),
            Self::SharedCpu(error) => error.fmt(formatter),
            Self::Words(error) => error.fmt(formatter),
            Self::Model(error) => error.fmt(formatter),
            Self::FormulaBatchShape { expected, actual } => write!(formatter,
                "formula checker returned {actual} results for {expected} candidates"),
        }
    }
}

impl std::error::Error for SolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Batch(error) => Some(error),
            Self::CompletionPool(error) => Some(error),
            Self::ExecutionObservation(error) => Some(error.as_ref()),
            Self::Static(error) => Some(error),
            #[cfg(feature = "gpu")]
            Self::Gpu(error) => Some(error),
            #[cfg(feature = "gpu")]
            Self::LazyGpu(error) => Some(error),
            Self::SharedCpu(error) => Some(error),
            Self::Words(error) => Some(error),
            Self::Model(error) => Some(error),
            Self::BackendUnavailable
            | Self::UnsupportedOracle { .. }
            | Self::UnsupportedSourceBatching
            | Self::PreparedInput { .. }
            | Self::LazyStatisticsOverflow
            | Self::FormulaBatchShape { .. } => None,
        }
    }
}

/// Original execution failure with any already established semantic evidence.
/// Preparation can fail before a session starts; absent coverage remains absent.
#[derive(Debug)]
pub struct SolveFailure {
    /// Original typed cause, independent of subsequent consumer effects.
    pub cause: Box<SolveError>,
    /// Attempted host timing, independently of semantic completion.
    pub phase_timings: Option<Box<PhaseTimings>>,
    pub(crate) subject: Option<Subject>,
    pub(crate) semantic: Option<Box<SemanticOutcome>>,
}

impl SolveFailure {
    /// Known original input, including a prepared session's setup failure.
    #[must_use]
    pub fn subject(&self) -> Option<&Subject> {
        self.semantic()
            .and_then(SemanticOutcome::subject)
            .or(self.subject.as_ref())
    }

    /// Evidence established before the failed operation stopped execution.
    #[must_use]
    pub fn semantic(&self) -> Option<&SemanticOutcome> {
        self.semantic.as_deref()
    }

    /// Move the cause and evidence into a consumer's error representation.
    /// The subject matches [`Self::subject`], including identity retained in the
    /// semantic outcome. Decomposition cannot strengthen semantic evidence.
    #[must_use]
    pub fn into_parts(self) -> FailureParts {
        let subject = self.subject().cloned();
        FailureParts {
            cause: self.cause,
            subject,
            semantic: self.semantic,
            phase_timings: self.phase_timings,
        }
    }
}

/// Owned decomposition of a failed solve for a consumer's typed adapter.
#[derive(Debug)]
pub struct FailureParts {
    /// Original execution cause.
    pub cause: Box<SolveError>,
    /// Known original input, matching [`SolveFailure::subject`] before decomposition.
    pub subject: Option<Subject>,
    /// Established evidence, absent when no semantic session began.
    pub semantic: Option<Box<SemanticOutcome>>,
    /// Optional attempted host timings.
    pub phase_timings: Option<Box<PhaseTimings>>,
}

impl From<SolveError> for SolveFailure {
    fn from(cause: SolveError) -> Self {
        Self {
            cause: Box::new(cause),
            subject: None,
            semantic: None,
            phase_timings: None,
        }
    }
}

impl fmt::Display for SolveFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(formatter)
    }
}

impl std::error::Error for SolveFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
