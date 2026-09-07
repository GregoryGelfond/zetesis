//! Bounded ordinary CPU characterization of three pinned clean examples.
//!
//! Fresh processes enumerate all ordinary answers or final optimum ties. Source
//! loading, parsing, grounding, solving and captured output are inside each wall
//! interval; comparison and hashing are outside it. Native human output and
//! clingo JSON have different encoding costs. Instrumented native diagnostics
//! are separate samples and never enter the uninstrumented timing population.
//! This campaign measures neither peak RSS nor GPU execution. It is not the
//! complete eager/lazy × CPU/Metal corpus matrix.
//!
//! Callers arrange a quiet measurement window after build qualification. Input
//! and executable seals do not cover dynamic libraries, hardware, environment,
//! transient file changes or thermal state. Trusted solvers are required.

mod config;
mod record;
mod run;
mod timing;
mod view;
#[path = "phase.rs"]
mod phase;
#[path = "stage.rs"]
mod stage;

use std::fmt;
use std::path::Path;

use serde::Serialize;

use crate::selected::{Change, FileSeal, publication};

pub use config::{Case, Limits, Phase, Producer, Request, Schedule, Slot};
pub use record::{Capture, Decision, Fault, Sample};
pub use timing::{Diagnostics, Measurement};

/// Setup or evidence-publication failure, never an answer-set verdict.
#[derive(Debug)]
pub enum Error {
    /// The clean corpus did not establish its sealed input contract.
    Corpus(crate::examples::Error),
    /// Shared bounded identity or new-file publication refused the request.
    Boundary(crate::selected::Error),
    /// The authored experiment configuration is outside its supported bounds.
    Configuration(&'static str),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Corpus(error) => error.fmt(f),
            Self::Boundary(error) => error.fmt(f),
            Self::Configuration(reason) => write!(f, "performance configuration: {reason}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Corpus(error) => Some(error),
            Self::Boundary(error) => Some(error),
            Self::Configuration(_) => None,
        }
    }
}
impl From<crate::selected::Error> for Error {
    fn from(error: crate::selected::Error) -> Self {
        Self::Boundary(error)
    }
}

/// Retained campaign owner. Failed or stopped attempts remain inspectable.
#[derive(Debug, Serialize)]
pub struct Report {
    schema: u32,
    manifest_sha256: &'static str,
    schedule: Schedule,
    limits: Limits,
    started_unix_ns: u128,
    finished_unix_ns: Option<u128>,
    wall_scope: &'static str,
    comparison_scope: &'static str,
    peak_rss: &'static str,
    gpu_measurement: &'static str,
    before: Vec<FileSeal>,
    after: Vec<Change>,
    metadata: Vec<Capture>,
    samples: Vec<Sample>,
    total_capture_bytes: usize,
    faults: Vec<Fault>,
    unresolved_children: Vec<u32>,
    #[serde(skip)]
    destination: publication::Destination,
}
impl Report {
    /// Whether every scheduled invocation completed with unchanged sealed inputs,
    /// matching display contracts, successful diagnostics and no effect failure.
    /// This is qualification of these observations, not evidence of a speedup.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.samples.len() == self.schedule.slots().len()
            && self
                .samples
                .iter()
                .all(|sample| sample.decision() == Decision::Pass)
            && self.metadata.len() == 3
            && self.metadata.iter().all(|capture| capture.complete(false))
            && self.after.iter().all(Change::unchanged)
            && self.finished_unix_ns.is_some()
            && self.faults.is_empty()
            && self.unresolved_children.is_empty()
    }
    /// Exact fixed execution order, independent of sample success or duration.
    #[must_use]
    pub const fn schedule(&self) -> Schedule {
        self.schedule
    }
    /// Complete requested resource configuration, with each independent ceiling.
    #[must_use]
    pub const fn limits(&self) -> &Limits {
        &self.limits
    }
    /// Wall-clock start metadata in nanoseconds since the Unix epoch.
    /// This is distinct from monotonic elapsed measurements.
    #[must_use]
    pub const fn started_unix_ns(&self) -> u128 {
        self.started_unix_ns
    }
    /// Wall-clock finish metadata; absent if the system clock could not supply it.
    #[must_use]
    pub const fn finished_unix_ns(&self) -> Option<u128> {
        self.finished_unix_ns
    }
    /// Every launched solve, including qualification, warmups and diagnostics.
    #[must_use]
    pub fn samples(&self) -> &[Sample] {
        &self.samples
    }
    /// Native version/help and reference version captures, outside solve timing.
    #[must_use]
    pub fn metadata(&self) -> &[Capture] {
        &self.metadata
    }
    /// Before-run primary-file seals, including each selected include closure.
    #[must_use]
    pub fn before(&self) -> &[FileSeal] {
        &self.before
    }
    /// Post-run identity observations, including read failures.
    #[must_use]
    pub fn after(&self) -> &[Change] {
        &self.after
    }
    /// Campaign-level effect failures and stopped scheduling.
    #[must_use]
    pub fn faults(&self) -> &[Fault] {
        &self.faults
    }
    /// Direct children whose ownership was explicitly abandoned after bounded cleanup.
    #[must_use]
    pub fn unresolved_children(&self) -> &[u32] {
        &self.unresolved_children
    }
    /// Total retained stdout and stderr bytes; excludes allocator and JSON overhead.
    #[must_use]
    pub const fn total_capture_bytes(&self) -> usize {
        self.total_capture_bytes
    }
    /// Write a new report using the shared no-clobber publication boundary.
    /// The destination's parent must remain exclusively controlled by the caller.
    ///
    /// # Errors
    /// Refuses existing/aliased paths, the serialized-byte ceiling or I/O failure.
    pub fn publish(&self) -> Result<(), Error> {
        publication::write(
            &view::Published {
                passed: self.passed(),
                report: self,
            },
            &self.destination,
            self.limits.max_report_bytes,
        )
        .map_err(Error::Boundary)
    }
}

/// Verify and seal the clean input closure, then run the fixed paired schedule.
/// All three input pairs must qualify before any warmup or timed pair is launched.
/// Individual capture, contract and diagnostic failures remain in the report;
/// failures stop subsequent launches and never cause replacement measurements.
/// Primary files are rechecked even after interrupted execution. Sources are
/// copied from the owned verified corpus into a private directory, retaining
/// byte content and relative include paths; temporary absolute paths differ.
///
/// Storage is bounded by retained capture plus sealed source and parsed-answer
/// ceilings. Process polling and the campaign deadline do not bound OS latency.
///
/// # Errors
/// Returns setup, identity or configuration failure before solver execution.
pub fn run(request: &Request<'_>) -> Result<Report, Error> {
    run::campaign(request)
}

fn io(path: &Path, source: std::io::Error) -> Error {
    crate::selected::identity::io(path, source).into()
}
