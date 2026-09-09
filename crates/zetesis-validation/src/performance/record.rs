//! Raw invocation evidence and typed observation decisions.
use super::{Diagnostics, Slot};
use crate::{process, selected::InvocationFailure};
use serde::Serialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Campaign-level failure, retained independently of solver conclusions.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum Fault {
    /// No later invocation was launched after the campaign deadline.
    Deadline,
    /// The cumulative raw-capture budget was exhausted.
    CaptureBudget,
    /// A version/help invocation did not complete successfully.
    Metadata,
    /// The last sample failed qualification; no replacement sample was launched.
    Observation,
    /// Explicit retry of direct-child cleanup failed.
    ChildCleanup(String),
    /// A private verified source could not be written.
    InputWrite(String),
    /// Removing the private source directory failed.
    InputCleanup(String),
    /// Wall-clock finish metadata was unavailable; monotonic samples remain retained.
    Clock,
}
/// Completion and comparison classification of one scheduled observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Complete capture, pinned display contract and exact reference parity passed.
    Pass,
    /// Start, process, output or cleanup failure prevented complete capture.
    InvocationFailure,
    /// Captured output did not establish complete reported answers.
    InvalidReport,
    /// Selected displays, multiplicities, counts or costs differ from the contract/reference.
    ModelMismatch,
    /// Separate phase statistics were missing, malformed or incomplete.
    InvalidDiagnostics,
    /// A separate child resource record was absent, invalid or contradicted completion.
    InvalidMemory,
}
/// Exact primary invocation and its bounded raw outputs.
#[derive(Debug, Serialize)]
pub struct Capture {
    pub(super) executable: PathBuf,
    pub(super) arguments: Vec<OsString>,
    pub(super) directory: PathBuf,
    pub(super) started_unix_ns: Option<u128>,
    pub(super) elapsed_ns: Option<u128>,
    pub(super) stop: Option<process::Stop>,
    pub(super) exit: Option<process::Exit>,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) failure: Option<InvocationFailure>,
    pub(super) cleanup_failure: Option<InvocationFailure>,
    pub(super) unresolved_child: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) helper_child_id: Option<u32>,
}
impl Capture {
    /// Direct helper ID for separately supervised resource samples.
    /// Ordinary timing captures omit this optional extension.
    #[must_use]
    pub const fn helper_child_id(&self) -> Option<u32> {
        self.helper_child_id
    }
    /// Selected executable's absolute path.
    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }
    /// Complete argument vector; no shell or implicit solver flags are used.
    #[must_use]
    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }
    /// Private byte-preserving source directory used during this invocation.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    /// Wall-clock metadata captured immediately before the process invocation.
    /// Absence does not invent an epoch value; elapsed timing remains monotonic.
    #[must_use]
    pub const fn started_unix_ns(&self) -> Option<u128> {
        self.started_unix_ns
    }
    /// Initial process/capture/reaping attempt; later cleanup retries are excluded.
    /// Absent if spawning did not start. Failed durations are never timed samples.
    #[must_use]
    pub const fn elapsed_ns(&self) -> Option<u128> {
        self.elapsed_ns
    }
    /// Raw retained standard-output prefix.
    #[must_use]
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }
    /// Raw retained standard-error prefix.
    #[must_use]
    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
    /// Why authored process capture stopped.
    #[must_use]
    pub const fn stop(&self) -> Option<process::Stop> {
        self.stop
    }
    /// Reaped exit code/signal; absent when unavailable.
    #[must_use]
    pub const fn exit(&self) -> Option<process::Exit> {
        self.exit
    }
    /// Start/capture fault with its typed operation.
    #[must_use]
    pub const fn failure(&self) -> Option<&InvocationFailure> {
        self.failure.as_ref()
    }
    /// Cleanup fault independent of primary capture failure.
    #[must_use]
    pub const fn cleanup_failure(&self) -> Option<&InvocationFailure> {
        self.cleanup_failure.as_ref()
    }
    pub(super) fn complete(&self, reference: bool) -> bool {
        self.stop == Some(process::Stop::Completed)
            && self.failure.is_none()
            && self.cleanup_failure.is_none()
            && self.unresolved_child.is_none()
            && self.exit.is_some_and(|exit| {
                exit.signal.is_none()
                    && match exit.code {
                        Some(0) => true,
                        Some(10 | 20 | 30) => reference,
                        _ => false,
                    }
            })
    }
}
/// Exact comparison outcome, without duplicating every parsed display in memory.
#[derive(Debug, Serialize)]
pub struct Sample {
    pub(super) slot: Slot,
    pub(super) capture: Capture,
    pub(super) decision: Decision,
    pub(super) detail: Option<String>,
    pub(super) selected_models: Option<u64>,
    pub(super) cost: Option<Vec<i64>>,
    pub(super) diagnostics: Option<Diagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) memory: Option<process::memory::Measurement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) memory_record: Option<Vec<u8>>,
}
impl Sample {
    /// Immutable schedule coordinates.
    #[must_use]
    pub fn slot(&self) -> Slot {
        self.slot.clone()
    }
    /// Complete raw evidence from this observation.
    #[must_use]
    pub const fn capture(&self) -> &Capture {
        &self.capture
    }
    /// Typed qualification decision.
    #[must_use]
    pub const fn decision(&self) -> Decision {
        self.decision
    }
    /// Failure explanation, never a substitute for its typed decision.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
    /// Complete selected occurrence count when report parsing succeeded.
    #[must_use]
    pub const fn selected_models(&self) -> Option<u64> {
        self.selected_models
    }
    /// Final objective vector when optimization applies.
    #[must_use]
    pub fn cost(&self) -> Option<&[i64]> {
        self.cost.as_deref()
    }
    /// Separate validated phase observations, absent from timed/warmup samples.
    #[must_use]
    pub const fn diagnostics(&self) -> Option<&Diagnostics> {
        self.diagnostics.as_ref()
    }
    /// Separate child peak RSS and exit evidence; never a timed observation.
    #[must_use]
    pub const fn memory(&self) -> Option<process::memory::Measurement> {
        self.memory
    }
    /// Bounded raw helper record, including invalid prefixes, separate from pipe capture.
    #[must_use]
    pub fn memory_record(&self) -> Option<&[u8]> {
        self.memory_record.as_deref()
    }
}
