//! Complete selected-curated comparison, composed from shared validation APIs.
//!
//! The immutable 24-case target has no show projection or external source roots.
//! Its clingo displays can therefore be compared against the recorded full-model
//! contracts; that implication does not apply to arbitrary ASP sources. Native
//! full atoms come from typed JSON records, independently of shown values.
//!
//! Requests select named executables and a bounded execution configuration;
//! source paths, all-model enumeration and output controls belong to the campaign.
//! Primary executables and every curated input are sealed before and after runs.
//! Seals do not cover dynamic libraries, the inherited environment, hardware or
//! changes that are restored between checks. Completed children do not establish
//! descendant termination or system quiescence. Solvers must be trusted.

pub(crate) mod identity;
pub(crate) mod publication;
mod run;
mod view;

use std::fmt;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use crate::{answers, curated, process};

pub use identity::{Change, FileSeal};
pub use run::{CaseResult, Decision, InvocationFailure, InvocationFault, InvocationRecord};

/// Number of original assertions in this immutable selected campaign.
pub const CASE_COUNT: usize = curated::CASE_COUNT;

/// An effect failure outside an individual producer's capture.
#[derive(Debug, Serialize)]
#[serde(tag = "operation", content = "detail", rename_all = "snake_case")]
pub enum CampaignFault {
    /// A verified source could not be copied into the private invocation directory.
    InputWrite(String),
    /// The private invocation directory could not be removed after execution.
    InputCleanup(String),
    /// An explicit retry of direct-child cleanup failed.
    ChildCleanup(String),
}

/// Requested hardware policy; observed execution is retained separately in raw
/// native JSON/statistics and is never inferred from this request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    /// Require CPU execution.
    #[default]
    Cpu,
    /// Request physical Metal; unavailable or unsupported execution must fail.
    Metal,
}
impl Backend {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Metal => "metal",
        }
    }
}

/// Requested native reduct procedure, without changing the selected source task.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Oracle {
    /// Let the native solver choose an applicable reduct procedure.
    #[default]
    Auto,
    /// Request reduct closure explicitly.
    Closure,
    /// Request general reduct countermodel checking explicitly.
    Countermodel,
}
impl Oracle {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Closure => "closure",
            Self::Countermodel => "countermodel",
        }
    }
}

/// Requested materialization policy, independent of requested hardware.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Grounder {
    /// Request complete eager materialization.
    #[default]
    Eager,
    /// Request lazy source joins; unsupported cases remain failures.
    Lazy,
    /// Request the solver's documented adaptive policy.
    Auto,
}
impl Grounder {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Eager => "eager",
            Self::Lazy => "lazy",
            Self::Auto => "auto",
        }
    }
}

/// Execution requests only; no extra source, constants, stdin or output options.
/// Other solver budgets retain the sealed executable's defaults.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct NativeExecution {
    /// Requested execution hardware.
    pub backend: Backend,
    /// Requested reduct procedure.
    pub oracle: Oracle,
    /// Requested materialization policy.
    pub grounder: Grounder,
    /// Optional eager-formula join strategy. Omission preserves the sealed
    /// executable's default and permits comparison with versions before this flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula_joins: Option<FormulaJoins>,
    /// Optional formula candidate proposal. Omission preserves the sealed
    /// executable's default and permits comparison with versions before this flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates: Option<CandidateSearch>,
    /// Closure worker request.
    pub workers: NonZeroUsize,
    /// Formula completion worker request.
    pub completion_workers: NonZeroUsize,
    /// Native candidate batch ceiling.
    pub batch_size: NonZeroUsize,
    /// Logical completion scratch allowance; zero is a valid requested ceiling.
    pub max_completion_scratch_bytes: u64,
    /// Cooperative deadline passed as `--time-limit`, in whole seconds. Absent
    /// means no deadline; a deadline changes what the solver polls at every
    /// charged unit, so it is part of the profile's identity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_limit_seconds: Option<std::num::NonZeroU64>,
}
impl Default for NativeExecution {
    fn default() -> Self {
        Self {
            backend: Backend::Cpu,
            oracle: Oracle::Auto,
            grounder: Grounder::Eager,
            formula_joins: None,
            candidates: None,
            workers: NonZeroUsize::new(1).expect("one is nonzero"),
            completion_workers: NonZeroUsize::new(1).expect("one is nonzero"),
            batch_size: NonZeroUsize::new(64).expect("64 is nonzero"),
            max_completion_scratch_bytes: 268_435_456,
            time_limit_seconds: None,
        }
    }
}

/// Requested source-join strategy, independent of backend and reduct procedure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FormulaJoins {
    /// Existing shortest-posting joins.
    Indexed,
    /// Prepared table masks for eligible completed-support patterns.
    Table,
}

impl FormulaJoins {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Indexed => "indexed",
            Self::Table => "table",
        }
    }
}

/// How the native formula route proposes classical candidates to the reduct.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CandidateSearch {
    /// Leaves of the region tree narrowed by the theory's readings.
    Regions,
    /// The retained classical search over a clause form of the theory.
    Clauses,
}

impl CandidateSearch {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Regions => "regions",
            Self::Clauses => "clauses",
        }
    }
}

/// Input, execution, retained-capture, normalization and publication ceilings.
/// These are independent logical measures, not peak heap or wall-clock bounds.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Exact curated input verification ceilings.
    pub corpus: curated::Limits,
    /// Per-invocation raw capture and lifetime polling ceilings.
    pub process: process::Limits,
    /// Combined raw stdout/stderr retained across all launched invocations.
    pub max_total_capture_bytes: usize,
    /// Maximum bytes read while sealing each primary executable.
    pub max_executable_bytes: usize,
    /// Generic clingo report normalization ceilings.
    pub reference: answers::Limits,
    /// Native typed-record normalization ceilings.
    pub native: answers::native_json::Limits,
    /// Combined bytes rendered from native full atoms for each case. Reference
    /// spellings are already present in its separately bounded JSON input.
    pub max_spelling_bytes: usize,
    /// Serialized final report bytes, including raw captures and diagnostics.
    pub max_report_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            corpus: curated::Limits::default(),
            process: process::Limits {
                timeout: Duration::from_secs(5),
                max_output_bytes: 1_114_112,
                cleanup_timeout: Duration::from_secs(1),
            },
            max_total_capture_bytes: 33_554_432,
            max_executable_bytes: 268_435_456,
            reference: answers::Limits::default(),
            native: answers::native_json::Limits::default(),
            max_spelling_bytes: 8_388_608,
            max_report_bytes: 134_217_728,
        }
    }
}

/// Entire sealed comparison request. Paths are supplied by the application;
/// executable PATH lookup and command-line parsing do not belong to this library.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    /// Curated root containing its pinned manifest, license and 24 sources.
    pub corpus: &'a Path,
    /// Absolute independent reference executable.
    pub reference: &'a Path,
    /// Absolute native executable.
    pub native: &'a Path,
    /// New report path; its existing parent must be exclusively controlled.
    pub report: &'a Path,
    /// Explicit native execution request.
    pub execution: NativeExecution,
    /// Authored resource ceilings.
    pub limits: Limits,
}

/// A setup, integrity or report-publication failure, never semantic UNSAT.
#[derive(Debug)]
pub enum Error {
    /// The selected corpus did not establish its pinned integrity contract.
    Corpus(curated::Error),
    /// The strong process backend is not supported on this platform.
    UnsupportedPlatform,
    /// A path or input-closure policy was refused before execution/publication.
    Path {
        /// Relevant requested path.
        path: PathBuf,
        /// Diagnostic policy explanation; not an interchange discriminator.
        detail: &'static str,
    },
    /// A filesystem operation failed.
    Io {
        /// Relevant path.
        path: PathBuf,
        /// Original failure.
        source: std::io::Error,
    },
    /// A bounded file or serialized report exceeded its inclusive byte allowance.
    Bytes {
        /// Named file or report.
        path: PathBuf,
        /// Inclusive byte ceiling.
        limit: usize,
    },
    /// A final report could not be serialized.
    Json(serde_json::Error),
    /// Primary publication failed and scoped temporary-file cleanup also failed.
    Cleanup {
        /// Primary publication failure, retained without replacement.
        primary: Box<Error>,
        /// Secondary cleanup failure.
        cleanup: std::io::Error,
    },
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Corpus(error) => write!(f, "{error}"),
            Self::UnsupportedPlatform => {
                f.write_str("selected campaign requires the Linux/macOS process backend")
            }
            Self::Path { path, detail } => write!(f, "{}: {detail}", path.display()),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Bytes { path, limit } => {
                write!(f, "{} exceeds byte ceiling {limit}", path.display())
            }
            Self::Json(error) => write!(f, "report JSON: {error}"),
            Self::Cleanup { primary, cleanup } => {
                write!(f, "{primary}; temporary cleanup: {cleanup}")
            }
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Corpus(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::Json(error) => Some(error),
            Self::Cleanup { primary, .. } => Some(primary.as_ref()),
            _ => None,
        }
    }
}

/// Complete retained campaign evidence. Only [`run`] constructs this owner.
#[derive(Debug, Serialize)]
pub struct Report {
    schema: u32,
    target: &'static str,
    manifest_sha256: &'static str,
    requested_execution: NativeExecution,
    requested_limits: Limits,
    full_model_comparison_scope: &'static str,
    physical_execution_qualified: bool,
    before: Vec<FileSeal>,
    after: Vec<Change>,
    cases: Vec<CaseResult>,
    total_capture_bytes: usize,
    unresolved_children: Vec<u32>,
    faults: Vec<CampaignFault>,
    #[serde(skip)]
    destination: publication::Destination,
    #[serde(skip)]
    max_report_bytes: usize,
}
impl Report {
    /// True only for all 24 complete exact comparisons and unchanged inputs.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.cases.len() == CASE_COUNT
            && self
                .cases
                .iter()
                .all(|case| case.decision() == Decision::Pass)
            && self.after.iter().all(Change::unchanged)
            && self.unresolved_children.is_empty()
            && self.faults.is_empty()
    }
    /// Every attempted case, including all retained failures.
    #[must_use]
    pub fn cases(&self) -> &[CaseResult] {
        &self.cases
    }
    /// Complete primary-input seals established before any solver starts.
    #[must_use]
    pub fn before(&self) -> &[FileSeal] {
        &self.before
    }
    /// Rechecks performed after the last invocation, including read failures.
    #[must_use]
    pub fn after(&self) -> &[Change] {
        &self.after
    }
    /// Effect failures retained independently of semantic comparison.
    #[must_use]
    pub fn faults(&self) -> &[CampaignFault] {
        &self.faults
    }
    /// Child IDs explicitly abandoned after the bounded cleanup retry failed.
    /// No further launches occur after this condition. This is diagnostic
    /// evidence, not a live process handle or a descendant-termination claim.
    #[must_use]
    pub fn unresolved_children(&self) -> &[u32] {
        &self.unresolved_children
    }
    /// Publish this evidence without overwriting an existing path.
    /// The destination is revalidated before publication under the documented
    /// exclusive-parent assumption; this is not an adversarial filesystem lease.
    ///
    /// # Errors
    /// Refuses report aliases, existing destinations, serialization/byte ceilings
    /// and filesystem failures. Temporary cleanup failures retain the primary cause.
    pub fn publish(&self) -> Result<(), Error> {
        publication::write(
            &view::Published {
                passed: self.passed(),
                report: self,
            },
            &self.destination,
            self.max_report_bytes,
        )
    }
}

/// Verify all inputs and the output path, then execute the complete selected target.
/// Sources are copied byte-for-byte from the owned verified corpus into a private
/// temporary directory. Every started invocation uses the shared process runner.
/// Failure records are retained; no retry silently turns an incomplete solve into
/// a pass. Unresolved direct-child cleanup stops further launches.
///
/// # Errors
/// Returns setup/integrity errors before any solver launch. Individual invocation
/// and semantic failures remain in the returned report. A temporary-input cleanup
/// failure is also retained as campaign failure evidence.
pub fn run(request: &Request<'_>) -> Result<Report, Error> {
    run::campaign(request)
}
