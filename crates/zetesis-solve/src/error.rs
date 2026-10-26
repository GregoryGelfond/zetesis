//! Execution failures, independent of source loading and answer publication.

use std::fmt;

use crate::{Backend, Grounder, Oracle, PhaseTimings, PreparedProfile, SemanticOutcome, Subject};

/// An unsuccessful preparation or execution operation; never evidence of UNSAT.
#[derive(Debug)]
pub enum SolveError {
    /// A verified base answer could not be completely reconstructed.
    Reconstruction(zetesis_themelios::ReconstructionError),
    /// Consumed base-answer accounting cannot represent another answer.
    TerminalStatisticsOverflow,
    /// A streamed source constraint could not be completely evaluated.
    Constraint(zetesis_themelios::ConstraintCheckFailure),
    /// The candidate traversal reported a source failure without its typed cause.
    ConstraintFailureMissing,
    /// Consumed core-answer accounting cannot represent another answer.
    HybridStatisticsOverflow,
    /// Explicit batch executor selection, protocol, or original external failure.
    Executor(crate::ExecutorError),
    /// Fixed-domain answer projection failed after preserving full membership.
    Projection(crate::ProjectionError),
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
    /// Device capability, submission or result transport failed.
    #[cfg(feature = "gpu")]
    Gpu(zetesis_wgpu::GpuError),
    /// An accepted interpretation could not retain its checked atom selection.
    Model(zetesis_core::ModelError),
    /// Cumulative model-construction work or completed count cannot represent another operation.
    ModelStatisticsOverflow,
    /// Membership execution ended without exhausted candidate coverage.
    CandidateStreamNotExhausted,
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
            Self::Reconstruction(error) => error.fmt(formatter),
            Self::TerminalStatisticsOverflow => {
                formatter.write_str("terminal answer accounting overflow")
            }
            Self::Constraint(error) => error.fmt(formatter),
            Self::ConstraintFailureMissing => {
                formatter.write_str("source region check stopped without its failure receipt")
            }
            Self::HybridStatisticsOverflow => {
                formatter.write_str("hybrid answer accounting overflow")
            }
            Self::Executor(error) => error.fmt(formatter),
            Self::Projection(error) => error.fmt(formatter),
            Self::CompletionPool(error) => write!(formatter, "completion worker pool: {error}"),
            Self::BackendUnavailable => {
                formatter.write_str("GPU support was not compiled into this build")
            }
            Self::UnsupportedOracle { backend, grounder } => write!(
                formatter,
                "the countermodel oracle requires eager or automatic grounding; requested {} with {}",
                backend.label(),
                grounder.label()
            ),
            Self::PreparedInput {
                profile,
                oracle,
                grounder,
            } => write!(
                formatter,
                "prepared {profile:?} cannot honor oracle {} with grounder {}",
                oracle.label(),
                grounder.label()
            ),
            Self::ExecutionObservation(error) => write!(formatter, "execution observer: {error}"),
            #[cfg(feature = "gpu")]
            Self::Gpu(error) => error.fmt(formatter),
            Self::Model(error) => error.fmt(formatter),
            Self::ModelStatisticsOverflow => {
                formatter.write_str("model construction statistics overflow")
            }
            Self::CandidateStreamNotExhausted => {
                formatter.write_str("membership execution ended before candidate exhaustion")
            }
            Self::FormulaBatchShape { expected, actual } => write!(
                formatter,
                "formula checker returned {actual} results for {expected} candidates"
            ),
        }
    }
}

impl std::error::Error for SolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Reconstruction(error) => Some(error),
            Self::Constraint(error) => Some(error),
            Self::Executor(error) => Some(error),
            Self::Projection(error) => Some(error),
            Self::CompletionPool(error) => Some(error),
            Self::ExecutionObservation(error) => Some(error.as_ref()),
            #[cfg(feature = "gpu")]
            Self::Gpu(error) => Some(error),
            Self::Model(error) => Some(error),
            Self::BackendUnavailable
            | Self::TerminalStatisticsOverflow
            | Self::ConstraintFailureMissing
            | Self::HybridStatisticsOverflow
            | Self::UnsupportedOracle { .. }
            | Self::PreparedInput { .. }
            | Self::FormulaBatchShape { .. }
            | Self::CandidateStreamNotExhausted => None,
            Self::ModelStatisticsOverflow => None,
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
