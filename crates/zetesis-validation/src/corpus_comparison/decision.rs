//! Typed per-case decisions and retained capture classifications.
use serde::Serialize;

/// Solver whose invocation or output is being checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Producer {
    /// Independent clingo reference.
    Reference,
    /// Native zetesis implementation.
    Native,
}

/// Why a process capture cannot establish a completed solver result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureFailure {
    /// The invocation deadline was reached.
    Timeout,
    /// The combined output allowance was reached.
    OutputLimit,
    /// An OS or capture operation failed.
    Capture,
    /// Completed output was not UTF-8.
    InvalidUtf8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CaptureStatus {
    Completed,
    Timeout,
    OutputLimit,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[serde(rename = "capture_failure")]
    CaptureFailure,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    InvalidUtf8,
}
impl CaptureStatus {
    pub(super) const fn failure(self) -> Option<CaptureFailure> {
        match self {
            Self::Completed => None,
            Self::Timeout => Some(CaptureFailure::Timeout),
            Self::OutputLimit => Some(CaptureFailure::OutputLimit),
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::CaptureFailure => Some(CaptureFailure::Capture),
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::InvalidUtf8 => Some(CaptureFailure::InvalidUtf8),
        }
    }
}

/// Result of checking one source. A reference pass does not establish native support.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Native and reference completed answers and required execution agree.
    Passed,
    /// Only the reference was requested and its contracts hold.
    ReferencePassed,
    /// A solver could not be invoked.
    InvocationFailed(Producer, String),
    /// Solver output capture did not complete under the requested contract.
    CaptureFailed(Producer, CaptureFailure),
    /// The reference returned an unexpected exit code.
    ReferenceError,
    /// The reference output failed its normalization contract.
    ReferenceOutputError(String),
    /// The reference failed the source's independent expectations.
    ReferenceContractMismatch(String),
    /// Native enumeration was interrupted or exhausted a budget.
    NativeIncomplete,
    /// Native source admission explicitly refused the input.
    NativeSourceRefused,
    /// The native invocation returned another failure exit.
    NativeError,
    /// Native output failed its normalization contract.
    NativeOutputUnsupported(String),
    /// The completed answer families disagree.
    Mismatch,
    /// Native answers failed the source's independent expectations.
    NativeContractMismatch(String),
    /// Matching answers lacked the required physical-route evidence.
    NativeExecutionUnqualified(String),
}
impl Decision {
    /// Stable schema-1 status spelling, also used in command progress output.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Passed => "pass",
            Self::ReferencePassed => "reference_pass",
            Self::InvocationFailed(Producer::Reference, _) => "reference_invocation_error",
            Self::InvocationFailed(Producer::Native, _) => "native_invocation_error",
            Self::CaptureFailed(Producer::Reference, CaptureFailure::Timeout) => {
                "reference_timeout"
            }
            Self::CaptureFailed(Producer::Reference, CaptureFailure::OutputLimit) => {
                "reference_output_limit"
            }
            Self::CaptureFailed(Producer::Reference, CaptureFailure::Capture) => {
                "reference_capture_failure"
            }
            Self::CaptureFailed(Producer::Reference, CaptureFailure::InvalidUtf8) => {
                "reference_invalid_utf8"
            }
            Self::CaptureFailed(Producer::Native, CaptureFailure::Timeout) => "native_timeout",
            Self::CaptureFailed(Producer::Native, CaptureFailure::OutputLimit) => {
                "native_output_limit"
            }
            Self::CaptureFailed(Producer::Native, CaptureFailure::Capture) => {
                "native_capture_failure"
            }
            Self::CaptureFailed(Producer::Native, CaptureFailure::InvalidUtf8) => {
                "native_invalid_utf8"
            }
            Self::ReferenceError => "reference_error",
            Self::ReferenceOutputError(_) => "reference_output_error",
            Self::ReferenceContractMismatch(_) => "reference_contract_mismatch",
            Self::NativeIncomplete => "native_incomplete",
            Self::NativeSourceRefused => "native_source_refused",
            Self::NativeError => "native_error",
            Self::NativeOutputUnsupported(_) => "native_output_unsupported",
            Self::Mismatch => "mismatch",
            Self::NativeContractMismatch(_) => "native_contract_mismatch",
            Self::NativeExecutionUnqualified(_) => "native_execution_unqualified",
        }
    }
    /// Diagnostic explanation, absent for the two successful decisions.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        match self {
            Self::Passed | Self::ReferencePassed => None,
            Self::InvocationFailed(_, error)
            | Self::ReferenceOutputError(error)
            | Self::ReferenceContractMismatch(error)
            | Self::NativeOutputUnsupported(error)
            | Self::NativeContractMismatch(error)
            | Self::NativeExecutionUnqualified(error) => Some(error),
            Self::CaptureFailed(Producer::Reference, _) => {
                Some("reference invocation did not complete")
            }
            Self::CaptureFailed(Producer::Native, _) => Some("native invocation did not complete"),
            Self::ReferenceError => Some("reference returned an unexpected exit code"),
            Self::NativeIncomplete => Some("native solver exhausted a budget or was interrupted"),
            Self::NativeSourceRefused | Self::NativeError => {
                Some("native solver did not return a completed result")
            }
            Self::Mismatch => Some("native and reference completed results differ"),
        }
    }
}
