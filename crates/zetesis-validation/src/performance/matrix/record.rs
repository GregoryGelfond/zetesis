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
    /// A solver reports incomplete coverage; its interruption remains in raw JSON.
    Incomplete,
    /// Authored process timeout expired.
    Timeout,
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
        /// Submitted candidates.
        candidates: u64,
        /// Propagation work units.
        work: u64,
        /// Exact CPU residual completions.
        cpu_residuals: u64,
        /// Peak logical accounted GPU bytes.
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
}
impl Sample {
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
