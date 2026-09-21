//! Owned semantic outcomes; JSON is a separate compatibility view.
use super::{Decision, Request, capture, corpus, execution, normalize};
use crate::{phase, stage};
use serde::Serialize;
use std::ffi::OsString;

#[derive(Debug, Default)]
pub(super) enum Observation<T> {
    #[default]
    Absent,
    Available(T),
    Malformed(String),
}

/// Retained result for one independently checked source.
/// Fields are immutable; callers inspect decisions without parsing report JSON.
#[derive(Debug)]
pub struct CaseResult {
    pub(super) evidence: CaseEvidence,
    pub(super) decision: Decision,
}

// Partial observations do not carry a provisional semantic decision.
#[derive(Debug)]
pub(super) struct CaseEvidence {
    pub(super) source: corpus::Case,
    pub(super) reference_process: Option<capture::Capture>,
    pub(super) native_process: Option<capture::Capture>,
    pub(super) reference_answer: Option<normalize::Answer>,
    pub(super) native_answer: Option<normalize::Answer>,
    pub(super) native_arguments: Option<Vec<OsString>>,
    pub(super) phase: Observation<phase::PhaseTimings>,
    pub(super) stage: Observation<stage::StageTimings>,
    pub(super) execution: Option<execution::FormulaExecution>,
    pub(super) answer_parity: bool,
}
impl CaseEvidence {
    pub(super) fn new(source: &corpus::Case) -> Self {
        Self {
            source: source.clone(),
            reference_process: None,
            native_process: None,
            reference_answer: None,
            native_answer: None,
            native_arguments: None,
            phase: Observation::Absent,
            stage: Observation::Absent,
            execution: None,
            answer_parity: false,
        }
    }
}
impl CaseResult {
    /// Original corpus-relative source path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.evidence.source.path
    }
    /// Source identity used by the corpus loader.
    #[must_use]
    pub fn source_sha256(&self) -> &str {
        &self.evidence.source.sha256
    }
    /// Checked per-source outcome, independent of its rendered spelling.
    #[must_use]
    pub const fn decision(&self) -> &Decision {
        &self.decision
    }
    /// Whether native and reference answer parity was established, even if a
    /// subsequent physical-route obligation failed.
    #[must_use]
    pub const fn answer_parity_passed(&self) -> bool {
        self.evidence.answer_parity
    }
    /// Completed reference answers, when normalization established them.
    #[must_use]
    pub fn reference_answers(&self) -> Option<&crate::answers::ReportedAnswers> {
        self.evidence
            .reference_answer
            .as_ref()
            .map(|answer| &answer.reported)
    }
    /// Completed native answers, when normalization established them.
    #[must_use]
    pub fn native_answers(&self) -> Option<&crate::answers::ReportedAnswers> {
        self.evidence
            .native_answer
            .as_ref()
            .map(|answer| &answer.reported)
    }
    /// Captured reference process elapsed milliseconds, not a benchmark sample.
    #[must_use]
    pub fn reference_elapsed_ms(&self) -> Option<u128> {
        self.evidence
            .reference_process
            .as_ref()
            .map(|capture| capture.elapsed_ms)
    }
    /// Captured native process elapsed milliseconds, not a benchmark sample.
    #[must_use]
    pub fn native_elapsed_ms(&self) -> Option<u128> {
        self.evidence
            .native_process
            .as_ref()
            .map(|capture| capture.elapsed_ms)
    }

    /// Observed reference exit, including its terminating signal on Unix.
    /// Absence means no reaped exit was retained; it is not an exit code.
    #[must_use]
    pub fn reference_exit(&self) -> Option<crate::process::Exit> {
        self.evidence
            .reference_process
            .as_ref()
            .and_then(|capture| capture.exit.0)
    }

    /// Observed native exit, independent of capture stop and semantic outcome.
    /// A normal exit alone does not establish completed answer enumeration.
    #[must_use]
    pub fn native_exit(&self) -> Option<crate::process::Exit> {
        self.evidence
            .native_process
            .as_ref()
            .and_then(|capture| capture.exit.0)
    }
}

#[derive(Debug, Serialize)]
pub(super) struct Cleanup {
    pub child_id: u32,
    pub exit: Option<crate::process::Exit>,
    pub failure: Option<String>,
    pub abandoned_unreaped_child: Option<u32>,
}

/// Whether the requested physical formula route was established for the full target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhysicalStatus {
    /// No physical formula qualification was requested.
    NotRequested,
    /// Every case passed, but none exercised device membership.
    NotExercised,
    /// The complete target passed and device membership was exercised.
    Qualified,
    /// At least one requested condition remains unestablished.
    Unqualified,
}

/// Owned comparison results and source identities. Constructed only by execution.
#[derive(Debug)]
pub struct Report {
    pub(super) request: Request,
    pub(super) invocation: super::NativeInvocation,
    pub(super) required_cases: usize,
    pub(super) corpus: corpus::Loaded,
    pub(super) cases: Vec<CaseResult>,
    pub(super) cleanup: Vec<Cleanup>,
    pub(super) cancelled: bool,
}
impl Report {
    /// Caller cancellation prevented a required launch or stopped an active
    /// capture. Completed earlier cases remain valid; the requested corpus
    /// comparison is incomplete. A late flag after all cases does not erase
    /// already completed evidence.
    #[must_use]
    pub const fn cancelled(&self) -> bool {
        self.cancelled
    }
    /// Number of independently required sources in the pinned target.
    #[must_use]
    pub const fn required_cases(&self) -> usize {
        self.required_cases
    }
    /// Results in source-manifest order, stopping on cancellation or unresolved cleanup.
    #[must_use]
    pub fn cases(&self) -> &[CaseResult] {
        &self.cases
    }
    /// Whether all 94 cases satisfy the requested mode and its route requirements.
    #[must_use]
    pub fn passed(&self) -> bool {
        !self.cancelled
            && self.cases.len() == self.required_cases()
            && self.cases.iter().all(|case| {
                if self.request.reference_only {
                    case.decision == Decision::ReferencePassed
                } else {
                    case.decision == Decision::Passed
                }
            })
            && (!self.request.physical_formula() || self.gpu_exercised() > 0)
    }
    /// Full native answer parity; reference-only execution cannot establish it.
    #[must_use]
    pub fn answer_parity_passed(&self) -> bool {
        !self.cancelled
            && !self.request.reference_only
            && self.cases.len() == self.required_cases()
            && self.cases.iter().all(|case| case.evidence.answer_parity)
    }
    pub(super) fn gpu_exercised(&self) -> usize {
        self.cases
            .iter()
            .filter(|case| {
                case.evidence
                    .execution
                    .as_ref()
                    .is_some_and(|e| e.status == execution::Status::GpuExercised)
            })
            .count()
    }
    pub(super) fn outer_unsat(&self) -> usize {
        self.cases
            .iter()
            .filter(|case| {
                case.evidence
                    .execution
                    .as_ref()
                    .is_some_and(|e| e.status == execution::Status::OuterUnsatWithoutMembership)
            })
            .count()
    }
    /// Requested physical-route result, separate from answer parity.
    #[must_use]
    pub fn physical_status(&self) -> PhysicalStatus {
        if !self.request.physical_formula() {
            PhysicalStatus::NotRequested
        } else if !self.cancelled
            && self.cases.len() == self.required_cases()
            && self
                .cases
                .iter()
                .all(|case| case.decision == Decision::Passed)
            && self.gpu_exercised() == 0
        {
            PhysicalStatus::NotExercised
        } else if self.passed() {
            PhysicalStatus::Qualified
        } else {
            PhysicalStatus::Unqualified
        }
    }
}
