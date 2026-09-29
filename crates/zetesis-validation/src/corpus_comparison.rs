//! Complete comparison of the pinned 94-case non-clingcon corpus.
//!
//! Loading verifies the selected source view before invoking trusted solvers.
//! Reference and native complete displayed-model multisets, costs and source
//! contracts are checked independently. Hidden atoms are not reconstructed.
//! Requests for physical formula execution additionally check retained route
//! telemetry; a requested backend alone never establishes physical execution.
//!
//! Execution is sequential. The clean source view uses the defaults of
//! [`crate::examples::Limits`]. The original-source view caps its manifest at
//! 4 MiB and each source at 1 MiB. Each child has a timeout and combined output
//! allowance. At most
//! 188 child captures are retained, so raw capture space is O(94 * allowance),
//! plus parsed answers and JSON rendering. There is no campaign-wide deadline.
//! Executable selection preserves per-invocation PATH lookup for bare names;
//! absolute paths bypass lookup. Environment variables are inherited.
//!
//! Linux/macOS use the shared bounded process-group capture. Other platforms
//! retain the command's direct-child adapter, which has weaker cleanup behavior
//! and a smaller capture report. Neither path proves descendant quiescence.

mod config;
mod corpus;
mod decision;
mod execution;
mod exit;
mod normalize;
mod record;
mod runner;
mod view;
mod capture;

pub use crate::selected::Oracle as NativeOracle;
pub use config::{NativeInvocation, Request};
pub use decision::{CaptureFailure, Decision, Producer};
pub use record::{CaseResult, PhysicalStatus, Report};

/// Failure before a corpus comparison can produce per-source outcomes.
#[derive(Debug)]
pub enum Error {
    /// The per-child timeout or output allowance is zero.
    InvalidLimits,
    /// The source view could not establish the pinned corpus contract.
    Corpus(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLimits => f.write_str("timeout and output ceilings must be positive"),
            Self::Corpus(error) => f.write_str(error),
        }
    }
}
impl std::error::Error for Error {}

/// Load and compare the complete fixed corpus, retaining unsuccessful case results.
///
/// `on_case` observes each completed case in manifest order and performs no role
/// in acceptance. It must return promptly and must not panic, mutate corpus
/// inputs or the execution environment, or interfere with child processes. It runs outside each invocation's
/// deadline. Presentation failures belong to the caller; they must not be
/// represented as answer-set decisions. Pass `|_| {}` for silent operation.
/// The library performs no global stdout/stderr writes or report publication.
/// Pending cleanup stops further cases and receives one bounded retry; any
/// unresolved direct-child ownership is explicitly recorded as abandoned.
///
/// # Errors
/// Refuses zero process limits or a source view that fails corpus loading.
/// Invocation/output failures are retained in [`Report`] instead.
pub fn run(request: &Request, on_case: impl FnMut(&CaseResult)) -> Result<Report, Error> {
    run_with_invocation(request, NativeInvocation::Legacy, on_case)
}

/// Compare the corpus using an explicitly selected native command protocol.
///
/// `Solve` requests structured full-model records and compares their selected
/// displays with clingo. Hidden full atoms are validated by the native decoder,
/// but are not inferred for clingo. Process, callback and cleanup contracts are
/// identical to [`run`]. The legacy entry point retains its original protocol.
/// Structured decoding keeps the native decoder's default atom/value ceilings;
/// its input and selected-display spelling are bounded by the captured input
/// and configured capture allowance, respectively.
///
/// # Errors
/// Refuses invalid process limits or an unverified corpus before execution.
pub fn run_with_invocation(
    request: &Request,
    invocation: NativeInvocation,
    on_case: impl FnMut(&CaseResult),
) -> Result<Report, Error> {
    run_with_cancellation(
        request,
        invocation,
        &std::sync::atomic::AtomicBool::new(false),
        on_case,
    )
}

/// Compare the corpus with cooperative cancellation of launches and captures.
///
/// The caller owns a monotone cancellation flag; this library installs no global
/// signal handler. Source verification and callbacks are finite operations but
/// are not preempted. Once cancellation is observed, no later solver starts.
/// Active captures retain partial output and their normal cleanup evidence;
/// completed earlier cases remain unchanged. [`Report::cancelled`] distinguishes
/// the incomplete campaign even when no solver was launched. The platform's
/// existing cleanup limitations still apply. Compatibility entry points use an
/// uncancelled flag.
///
/// # Errors
/// Refuses invalid process limits or an unverified corpus before execution.
pub fn run_with_cancellation(
    request: &Request,
    invocation: NativeInvocation,
    cancelled: &std::sync::atomic::AtomicBool,
    on_case: impl FnMut(&CaseResult),
) -> Result<Report, Error> {
    if request.timeout_ms == 0 || request.max_output_bytes == 0 {
        return Err(Error::InvalidLimits);
    }
    let corpus = corpus::load(request).map_err(Error::Corpus)?;
    Ok(runner::run_with_cancellation(
        request, corpus, invocation, cancelled, on_case,
    ))
}
