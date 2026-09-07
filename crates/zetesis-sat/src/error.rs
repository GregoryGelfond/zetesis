use std::fmt;

use crate::AdmissionError;
use zetesis_cpu::Stop;

/// Why no logical decision is available. Every variant leaves coverage open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Incomplete {
    /// Shared cancellation was observed.
    Cancelled,
    /// The absolute deadline expired.
    Deadline,
    /// Charged SAT work reached its inclusive ceiling.
    WorkLimit,
    /// Starting another decision would exceed the ceiling.
    DecisionLimit,
    /// Checking another classical candidate would exceed the ceiling.
    CandidateLimit,
    /// Pending candidate storage would exceed the caller's byte ceiling.
    PendingBytes,
    /// Concurrent completion workspaces and ordered results exceed logical scratch.
    CompletionScratch,
    /// A retained batch exceeds the current call's candidate ceiling.
    BatchCandidateLimit,
    /// Scalar iteration cannot discard an outstanding candidate batch.
    PendingBatch,
    /// Fallible storage reservation failed.
    Allocation,
    /// A finite encoding or blocking clause exceeded admission limits.
    Admission(AdmissionError),
    /// A candidate belongs to a different admitted theory instance.
    WrongTheory,
    /// A candidate-only restriction declares a different semantic atom count.
    RestrictionUniverse {
        /// Original semantic atom count.
        expected: usize,
        /// Restriction's declared atom count.
        actual: usize,
    },
    /// A fused iterator cannot restart after exhaustion or terminal failure.
    ClosedEnumerator,
    /// Independent formula evaluation stopped without validating a witness.
    Verification(Stop),
    /// An internal SAT witness failed independent semantic validation.
    InvalidWitness,
    /// An accounting counter cannot be incremented without overflow.
    CounterOverflow,
}
impl From<AdmissionError> for Incomplete {
    fn from(error: AdmissionError) -> Self {
        Self::Admission(error)
    }
}
impl From<Stop> for Incomplete {
    fn from(error: Stop) -> Self {
        match error {
            Stop::Cancelled => Self::Cancelled,
            Stop::Deadline => Self::Deadline,
            Stop::Allocation => Self::Allocation,
            Stop::WrongProgram => Self::WrongTheory,
            _ => Self::Verification(error),
        }
    }
}
impl fmt::Display for Incomplete {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("SAT operation cancelled"),
            Self::Deadline => f.write_str("SAT operation deadline expired"),
            Self::WorkLimit => f.write_str("SAT work limit reached"),
            Self::DecisionLimit => f.write_str("SAT decision limit reached"),
            Self::CandidateLimit => f.write_str("stable candidate limit reached"),
            Self::CompletionScratch => f.write_str("completion logical scratch byte limit reached"),
            Self::PendingBytes => f.write_str("pending candidate byte limit reached"),
            Self::BatchCandidateLimit => f.write_str("pending batch candidate limit reached"),
            Self::PendingBatch => f.write_str("an unresolved candidate batch remains"),
            Self::Allocation => f.write_str("SAT storage reservation failed"),
            Self::Admission(error) => error.fmt(f),
            Self::WrongTheory => f.write_str("candidate belongs to a different theory"),
            Self::RestrictionUniverse { expected, actual } => write!(
                f,
                "candidate restriction declares {actual} atoms; expected {expected}"
            ),
            Self::ClosedEnumerator => f.write_str("candidate enumeration is already closed"),
            Self::Verification(error) => write!(f, "independent verification: {error}"),
            Self::InvalidWitness => f.write_str("SAT witness failed independent verification"),
            Self::CounterOverflow => f.write_str("SAT accounting counter overflow"),
        }
    }
}
impl std::error::Error for Incomplete {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::Verification(error) => Some(error),
            _ => None,
        }
    }
}
