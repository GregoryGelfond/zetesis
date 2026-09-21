use crate::{performance::matrix::Observation, process, selected};
use serde::{Serialize, Serializer};

/// A full selected family with reported objective priorities preserved.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Family {
    /// Canonical atom sets, retaining repeated model occurrences.
    pub models: Vec<Vec<String>>,
    /// Final priority/value vector, distinct from merely displayed costs.
    pub costs: Option<Vec<(i32, i64)>>,
}

/// Conformance disposition; failure never establishes an empty answer family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Full families, known contracts and mandatory execution evidence agree.
    Passed,
    /// Spawn, capture, exit or cleanup could not establish completion.
    ProcessFailure,
    /// The process deadline expired.
    Timeout,
    /// The caller cancelled before or during this check.
    Cancelled,
    /// The process output ceiling was reached.
    CaptureLimit,
    /// Search or publication was incomplete.
    Incomplete,
    /// The structured answer report was malformed or contradictory.
    InvalidReport,
    /// The completed full family or objective vector violated its known contract.
    FamilyMismatch,
    /// Actual backend, procedure or work evidence was absent or contradictory.
    InvalidExecution,
    /// The CPU reference did not pass, so the selected route was not launched.
    ReferenceFailed,
}
impl Decision {
    /// Stable human/structured status spelling.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Passed => "pass",
            Self::ProcessFailure => "process_failure",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::CaptureLimit => "capture_limit",
            Self::Incomplete => "incomplete",
            Self::InvalidReport => "invalid_report",
            Self::FamilyMismatch => "family_mismatch",
            Self::InvalidExecution => "invalid_execution",
            Self::ReferenceFailed => "reference_failed",
        }
    }
}

/// One launched or refused child, with raw evidence separated from its decision.
#[derive(Debug)]
pub struct Attempt {
    pub(super) arguments: Vec<String>,
    pub(super) capture: Option<process::Capture>,
    pub(super) decision: Decision,
    pub(super) detail: Option<String>,
    pub(super) family: Option<Family>,
    pub(super) observation: Option<Observation>,
    pub(super) cleanup: Option<Cleanup>,
}
impl Attempt {
    /// Checked disposition of this child.
    #[must_use]
    pub const fn decision(&self) -> Decision {
        self.decision
    }
    /// Retained process evidence, absent when spawn was refused.
    #[must_use]
    pub const fn capture(&self) -> Option<&process::Capture> {
        self.capture.as_ref()
    }
    /// Reconciled mandatory route and work evidence.
    #[must_use]
    pub const fn observation(&self) -> Option<&Observation> {
        self.observation.as_ref()
    }
    /// Human explanation of a nonpass.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

#[derive(Debug, Serialize)]
pub(super) struct Cleanup {
    pub exit: Option<process::Exit>,
    pub failure: Option<String>,
    pub abandoned_child: Option<u32>,
}

/// One immutable program's CPU-reference and selected-backend checks.
#[derive(Debug, Serialize)]
pub struct CaseResult {
    pub(super) name: &'static str,
    pub(super) source: &'static str,
    pub(super) reference: Attempt,
    pub(super) selected: Option<Attempt>,
    pub(super) decision: Decision,
}
impl CaseResult {
    /// Stable fixture name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }
    /// Selected-backend attempt, absent when the CPU reference failed.
    #[must_use]
    pub const fn selected(&self) -> Option<&Attempt> {
        self.selected.as_ref()
    }
    /// CPU reference evidence, including an explanation when it failed.
    #[must_use]
    pub const fn reference(&self) -> &Attempt {
        &self.reference
    }
    /// Complete case disposition.
    #[must_use]
    pub const fn decision(&self) -> Decision {
        self.decision
    }
}

/// Complete small-check report; unrun cases remain visible through expected count.
#[derive(Debug, Serialize)]
pub struct Report {
    pub(super) schema: u8,
    pub(super) format: &'static str,
    pub(super) scope: &'static str,
    pub(super) backend: selected::Backend,
    pub(super) expected_cases: usize,
    pub(super) executable: selected::FileSeal,
    pub(super) executable_after: selected::Change,
    pub(super) cases: Vec<CaseResult>,
    pub(super) input_cleanup_failure: Option<String>,
}
impl Report {
    /// Number of independently required fixture contracts.
    #[must_use]
    pub const fn required_cases(&self) -> usize {
        self.expected_cases
    }
    /// All expected checks and the primary-executable recheck passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.cases.len() == self.expected_cases
            && self.executable_after.unchanged()
            && self.input_cleanup_failure.is_none()
            && self
                .cases
                .iter()
                .all(|case| case.decision == Decision::Passed)
    }
    /// Cases in fixed order; unresolved cleanup can leave a shorter prefix.
    #[must_use]
    pub fn cases(&self) -> &[CaseResult] {
        &self.cases
    }
    /// Render all evidence and explicit final acceptance without performing I/O.
    ///
    /// # Errors
    /// Returns a serialization failure for retained evidence.
    pub fn to_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self.json())
    }
    /// Borrowed structured view suitable for streaming without a JSON value tree.
    #[must_use]
    pub fn json(&self) -> impl Serialize + '_ {
        #[derive(Serialize)]
        struct View<'a> {
            #[serde(flatten)]
            report: &'a Report,
            passed: bool,
        }
        View {
            report: self,
            passed: self.passed(),
        }
    }
}

impl Serialize for Attempt {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct View<'a> {
            arguments: &'a [String],
            capture: Option<CaptureView<'a>>,
            decision: Decision,
            detail: Option<&'a str>,
            family: Option<&'a Family>,
            observation: Option<&'a Observation>,
            cleanup: Option<&'a Cleanup>,
        }
        View {
            arguments: &self.arguments,
            capture: self.capture.as_ref().map(CaptureView::from),
            decision: self.decision,
            detail: self.detail.as_deref(),
            family: self.family.as_ref(),
            observation: self.observation.as_ref(),
            cleanup: self.cleanup.as_ref(),
        }
        .serialize(serializer)
    }
}

#[derive(Serialize)]
struct CaptureView<'a> {
    stop: process::Stop,
    exit: Option<process::Exit>,
    elapsed_ns: u128,
    stdout: &'a [u8],
    stderr: &'a [u8],
    failure: Option<String>,
    cleanup_failure: Option<String>,
}
impl<'a> From<&'a process::Capture> for CaptureView<'a> {
    fn from(capture: &'a process::Capture) -> Self {
        Self {
            stop: capture.stop(),
            exit: capture.exit(),
            elapsed_ns: capture.elapsed().as_nanos(),
            stdout: capture.stdout(),
            stderr: capture.stderr(),
            failure: capture.failure().map(ToString::to_string),
            cleanup_failure: capture.cleanup_failure().map(ToString::to_string),
        }
    }
}
