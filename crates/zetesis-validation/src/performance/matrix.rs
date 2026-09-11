//! Instrumented end-to-end profile comparisons over clean source workloads.
//!
//! Every native invocation uses JSON and statistics. Wall time includes their
//! overhead, setup, grounding, solving and captured output. This is distinct from
//! the legacy uninstrumented CPU protocol and the eventual full uninstrumented
//! corpus matrix. Selected displays/counts/costs are compared with clingo; native
//! full records are retained, but hidden clingo interpretations are unavailable.
//!
//! First-observed refusals and failures disable only their cell's later launches;
//! every fixed schedule position remains recorded. No failed sample is replaced.
//! Callers arrange a qualified executable and quiet physical-device window.

mod config;
mod outcome;
mod record;
mod run;
mod serialization;
mod telemetry;
mod workload;

pub use config::{Plan, Producer, Request, Slot, Suite};
pub use record::{Decision, DeviceWork, Execution, Observation, Procedure, Sample};
pub use workload::{ConstantAmendment, Workload, WorkloadLimits};

use super::{Capture, Error, Fault};
use crate::selected::{Change, FileSeal, publication};
use serde::Serialize;

/// Owned complete matrix evidence, including unlaunched positions and refusals.
#[derive(Debug, Serialize)]
pub struct Report {
    schema: u32,
    protocol: &'static str,
    manifest_sha256: &'static str,
    plan: Plan,
    limits: super::Limits,
    native_normalization_limits: serde_json::Value,
    cases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workloads: Option<Vec<Workload>>,
    started_unix_ns: u128,
    finished_unix_ns: Option<u128>,
    wall_scope: &'static str,
    comparison_scope: &'static str,
    peak_rss: &'static str,
    before: Vec<FileSeal>,
    after: Vec<Change>,
    #[serde(serialize_with = "serialization::captures")]
    metadata: Vec<Capture>,
    samples: Vec<Sample>,
    total_capture_bytes: usize,
    faults: Vec<Fault>,
    unresolved_children: Vec<u32>,
    #[serde(skip)]
    destination: publication::Destination,
}
impl Report {
    /// Every planned position is represented; this does not require solver parity.
    #[must_use]
    pub fn accounted(&self) -> bool {
        self.plan
            .slots(self.cases.len())
            .is_ok_and(|slots| slots.len() == self.samples.len())
    }
    /// Every requested invocation passed parity/telemetry with unchanged inputs.
    /// Refused and skipped cells make this false even in a fully accounted report.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.accounted()
            && self.samples.iter().all(|s| s.decision == Decision::Pass)
            && self.metadata.len() == 3
            && self.metadata.iter().all(|c| c.complete(false))
            && self.after.iter().all(Change::unchanged)
            && self.faults.is_empty()
            && self.unresolved_children.is_empty()
            && self.finished_unix_ns.is_some()
    }
    /// Exact requested and actual disposition at each schedule position.
    #[must_use]
    pub fn samples(&self) -> &[Sample] {
        &self.samples
    }
    /// Entry paths in schedule order; explicit workloads may repeat a path.
    #[must_use]
    pub fn cases(&self) -> &[String] {
        &self.cases
    }
    /// Explicit workload identities in schedule order; absent for default suites.
    #[must_use]
    pub fn workloads(&self) -> Option<&[Workload]> {
        self.workloads.as_deref()
    }
    /// Setup/capture scheduling faults, separate from solver conclusions.
    #[must_use]
    pub fn faults(&self) -> &[Fault] {
        &self.faults
    }
    /// Immutable requested profile and schedule configuration.
    #[must_use]
    pub const fn plan(&self) -> &Plan {
        &self.plan
    }
    /// Pre-run primary and private-copy source/executable seals.
    #[must_use]
    pub fn before(&self) -> &[FileSeal] {
        &self.before
    }
    /// Post-run seal checks, including failed reads.
    #[must_use]
    pub fn after(&self) -> &[Change] {
        &self.after
    }
    /// Retained executable version/help captures outside solve observations.
    #[must_use]
    pub fn metadata(&self) -> &[Capture] {
        &self.metadata
    }
    /// Direct child IDs explicitly abandoned after bounded cleanup failed.
    #[must_use]
    pub fn unresolved_children(&self) -> &[u32] {
        &self.unresolved_children
    }
    /// Raw retained capture bytes, excluding JSON serialization and allocator overhead.
    #[must_use]
    pub const fn total_capture_bytes(&self) -> usize {
        self.total_capture_bytes
    }
    /// Publish evidence without replacing inputs, executables or existing files.
    /// The destination parent must remain exclusively controlled by the caller.
    ///
    /// # Errors
    /// Refuses aliases, changed destination identity, byte ceilings or I/O failures.
    pub fn publish(&self) -> Result<(), Error> {
        #[derive(Serialize)]
        struct Published<'a> {
            passed: bool,
            accounted: bool,
            report: &'a Report,
        }
        publication::write(
            &Published {
                passed: self.passed(),
                accounted: self.accounted(),
                report: self,
            },
            &self.destination,
            self.limits.max_report_bytes,
        )
        .map_err(Error::Boundary)
    }
}
/// Seal the verified source/include closure and binaries, then run the bounded
/// fixed matrix. Sources retain exact bytes in a private directory. Refusals,
/// timeouts and mismatches remain evidence rather than replacing samples.
/// Direct-child cleanup failure prevents further process launches. Bounds cover
/// requested capture/storage quantities, not allocator RSS or OS response latency.
///
/// # Errors
/// Returns configuration/source/identity failures before any solver is launched.
pub fn run(request: &Request<'_>) -> Result<Report, Error> {
    run::campaign(request, None)
}

/// Run explicit constant variants drawn from the plan's allowed source suite.
///
/// The existing profile schedule, process capture and complete-answer decoder
/// are shared with `run`. Each instance has a private source closure and a
/// distinct content identity, even when several instances share an entry path.
/// Default contracts remain provenance for amended workloads; those workloads
/// require a complete clingo family instead of the default model count.
/// Reports use schema 2. No first-answer or memory samples are added.
///
/// # Errors
/// Refuses empty/oversized populations, repeated content identities, foreign
/// corpus identities, sources outside the allowed suite and resource excess.
/// Materialization failures are retained in the returned report before any
/// solver invocation.
pub fn run_workloads(request: &Request<'_>, workloads: &[Workload]) -> Result<Report, Error> {
    run::campaign(request, Some(workloads))
}

#[cfg(test)]
#[path = "../../tests/support/matrix_reports.rs"]
mod fixtures;
