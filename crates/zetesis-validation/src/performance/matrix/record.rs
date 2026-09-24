//! Typed observations retain uncertainty instead of substituting requested policy.
use super::{
    super::{Capture, Diagnostics},
    Slot,
};
use crate::selected::Backend;
use serde::Serialize;

/// Disposition independent of timing magnitude or diagnostic prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Complete selected-family parity and required instrumented telemetry agree.
    Pass,
    /// The native typed envelope reports a source or combination refusal.
    Refused,
    /// The native envelope explicitly reports unavailable compiled backend support.
    BackendUnavailable,
    /// Search or publication is incomplete; independent coverage remains in raw JSON.
    Incomplete,
    /// Authored process timeout expired.
    Timeout,
    /// The caller cancelled this invocation; its partial capture remains retained.
    Cancelled,
    /// Authored capture ceiling was reached.
    CaptureLimit,
    /// Spawn/exit/capture/cleanup failed; a GPU error is not assumed to mean absence.
    InvocationFailure,
    /// Captured producer output is malformed or contradictory.
    InvalidReport,
    /// Completed selected answers violate the source contract or reference family.
    ParityMismatch,
    /// Reported route or instrumentation fails the requested observation contract.
    InvalidTelemetry,
    /// A complete reference was unavailable for comparison.
    ReferenceUnavailable,
    /// Prespecified scheduling policy prevented this position from launching.
    NotAttempted,
    /// A memory round's separate child-resource record was absent, invalid or
    /// contradicted the helper's evidence.
    InvalidMemory,
}
/// Actual reported semantic checker, including certified specializations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Procedure {
    /// Least reduct closure.
    Closure,
    /// General reduct countermodel procedure.
    Countermodel,
    /// Certified tight-program support checking.
    TightSupport,
    /// Certified positive consequences followed by original constraint validation.
    PositiveConsequences,
}
/// Decoded general-device outcomes requiring exact residual membership checks.
/// These count device verdicts, separately from host completion attempts and
/// commits. Historical observations can lack this receipt entirely.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct FormulaResidualStatistics {
    /// A complete no-change sweep left the proper-subset query unresolved.
    pub fixed_point: u64,
    /// The configured full-sweep ceiling was reached.
    pub round_limit: u64,
    /// The next full sweep could not fit its device work allowance.
    pub work_limit: u64,
}
/// Actual reported device activity; unavailable is never replaced by zero.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeviceWork {
    /// CPU route reports no device execution.
    Cpu,
    /// Typed lazy device counters, including complete occurrence accounting.
    Lazy {
        /// Dispatch count.
        dispatches: u64,
        /// Rule/world checks.
        world_instances: u64,
        /// Uploaded bytes.
        uploaded_bytes: u64,
        /// Downloaded bytes.
        downloaded_bytes: u64,
        /// Completed candidate occurrences.
        completed_candidates: u64,
    },
    /// Formula propagation with exact host residual completion.
    Formula {
        /// Device batches.
        batches: u64,
        /// Candidates returned by successfully decoded batches.
        candidates: u64,
        /// Propagation work units.
        work: u64,
        /// Exact CPU residual completions.
        cpu_residuals: u64,
        /// Decoded residual reasons, when reported. In these complete
        /// observations their sum equals the committed CPU residual count;
        /// absence remains unavailable rather than becoming zero.
        #[serde(skip_serializing_if = "Option::is_none")]
        gpu_residuals: Option<FormulaResidualStatistics>,
        /// Peak logical accounted GPU bytes.
        peak_accounted_bytes: u64,
    },
    /// Complete tight-support scans, with submitted work distinct from decoded
    /// results. Propagation sweeps do not describe this primitive.
    TightSupport {
        /// Successfully decoded device batches.
        batches: u64,
        /// Candidates returned by decoded batches.
        candidates: u64,
        /// Complete support-scan work in decoded results.
        work: u64,
        /// Submitted batches, including any without a decoded result.
        submitted_batches: u64,
        /// Submitted candidates, including any without a decoded result.
        submitted_candidates: u64,
        /// Scheduled complete-scan work, not a claim that readback completed.
        scheduled_work: u64,
        /// Configured complete support-scan work ceiling per candidate.
        work_per_candidate_limit: u64,
        /// Peak logical accounted GPU bytes from decoded batches.
        peak_accounted_bytes: u64,
    },
    /// Existing static closure driver exposes adapter identity but no device work total.
    Unavailable {
        /// Explicit measurement limitation.
        reason: &'static str,
    },
}
/// Native execution identity reported by the existing authored output views.
#[derive(Debug, Serialize)]
pub struct Execution {
    /// Backend established by reported route metadata, not copied from the request.
    pub backend: Backend,
    /// Reported procedure.
    pub procedure: Procedure,
    /// Device adapter where a device route was entered.
    pub adapter: Option<String>,
    /// Device activity with its measured or unavailable scope.
    pub device: DeviceWork,
}
/// One instrumented native observation; raw JSON preserves all other statistics.
#[derive(Debug, Serialize)]
pub struct Observation {
    /// Strict existing phase/stage partition; lazy grounding is interleaved.
    pub timing: Diagnostics,
    /// Actual execution metadata and measured device counters.
    pub execution: Execution,
    /// Completed streamed-source checking, absent on ordinary eager/lazy routes.
    /// Core search counters remain separate from original-program acceptance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hybrid: Option<HybridStatistics>,
}

/// Completed hybrid source-checking receipt, independent of device execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct HybridStatistics {
    /// Optional early region checks; absent in records from earlier executables.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regions: Option<RegionChecks>,
    /// Core answers consumed by the checker, including rejected ones.
    pub core_answers: u64,
    /// Original-program answers after checking all required constraints.
    pub accepted: u64,
    /// Core answers with an observed source-constraint violation.
    pub rejected: u64,
    /// Unfinished consumed checks; zero for a complete observation.
    pub pending: u64,
    /// Cumulative checker work, including its snapshot preparation.
    pub work: u64,
    /// Complete source substitutions visited by checks.
    pub substitutions: u64,
    /// Cumulative copied scalar payload, not retained memory or RSS.
    pub scalar_bytes: u64,
}

/// Completed candidate-region restriction counts, independent of membership.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct RegionChecks {
    /// Worker-local checker preparations, including attempts.
    pub preparations: u64,
    /// Candidate regions checked before splitting or membership.
    pub checks: u64,
    /// Regions rejected by a certain original constraint violation.
    pub refuted: u64,
}
impl Observation {
    /// Reconcile already-parsed native statistics with their captured text view.
    ///
    /// This pure decoder accepts only the supported finite phase/stage schemas
    /// and reconciles the actual route with the explicit request. It does not
    /// establish process completion, answer-family parity or semantic correctness.
    /// The caller bounds and parses `document` and bounds `stderr` before calling.
    /// The decoder scans the text linearly and reads a fixed set of JSON fields.
    /// Temporary line storage is linear in the supplied text; retained storage
    /// contains the finite measurements and reported adapter name.
    ///
    /// # Errors
    /// Refuses malformed, incomplete, unsupported or contradictory telemetry,
    /// including a route that differs from an explicit requested procedure.
    pub fn from_statistics(
        document: &serde_json::Value,
        stderr: &[u8],
        request: crate::selected::NativeExecution,
    ) -> Result<Self, String> {
        super::telemetry::observe(document, stderr, request)
    }
}
/// A schedule position with its raw capture when launched.
#[derive(Debug, Serialize)]
pub struct Sample {
    pub(super) slot: Slot,
    #[serde(serialize_with = "super::serialization::optional_capture")]
    pub(super) capture: Option<Capture>,
    pub(super) decision: Decision,
    pub(super) detail: Option<String>,
    pub(super) blocked_by: Option<usize>,
    pub(super) selected_models: Option<u64>,
    pub(super) cost: Option<Vec<i64>>,
    pub(super) observation: Option<Observation>,
    /// The child's peak resident set, on a memory round whose record was valid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) memory: Option<crate::process::memory::Measurement>,
}
impl Sample {
    /// The child's peak resident set, present only on a memory round whose
    /// separate record was valid.
    #[must_use]
    pub const fn memory(&self) -> Option<&crate::process::memory::Measurement> {
        self.memory.as_ref()
    }
    /// Fixed requested schedule position.
    #[must_use]
    pub const fn slot(&self) -> Slot {
        self.slot
    }
    /// Typed final disposition.
    #[must_use]
    pub const fn decision(&self) -> Decision {
        self.decision
    }
    /// Retained explanation of a refusal, mismatch or skipped position.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
    /// Decoded selected model count, when answer normalization succeeded.
    /// A count alone does not establish that the position passed qualification.
    #[must_use]
    pub const fn selected_models(&self) -> Option<u64> {
        self.selected_models
    }
    /// Decoded selected cost vector, when one was reported and normalized.
    #[must_use]
    pub fn cost(&self) -> Option<&[i64]> {
        self.cost.as_deref()
    }
    /// Exact launched process record, absent for a skipped position.
    #[must_use]
    pub const fn capture(&self) -> Option<&Capture> {
        self.capture.as_ref()
    }
    /// Earlier failed sample disabling this cell, when applicable.
    #[must_use]
    pub const fn blocked_by(&self) -> Option<usize> {
        self.blocked_by
    }
    /// Checked instrumented route/phase observation.
    #[must_use]
    pub const fn observation(&self) -> Option<&Observation> {
        self.observation.as_ref()
    }
}
