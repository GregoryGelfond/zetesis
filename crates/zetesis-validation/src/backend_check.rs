//! Small installed-command conformance checks, distinct from physical qualification.
//!
//! Three fixed eager formula programs exercise complete tight support, a general
//! reduct query and optimum ties. Each runs against CPU and the requested CPU or
//! Metal route, with known full-family and priority/cost contracts. Internal
//! statistics are mandatory route evidence. There is no optional instrumentation
//! claim and no claim of the repository's complete physical test population.
//! Six sequential children have individual capture/deadline/cleanup bounds;
//! retained bytes are O(6 * capture limit), plus bounded decoded records. Cleanup
//! ownership is settled before any later child starts. A failed retry stops the
//! remaining cases and retains its abandoned child identity explicitly.

mod record;
mod run;
mod validate;

use std::{fmt, io, path::PathBuf};

pub use crate::selected::Backend;
pub use record::{Attempt, CaseResult, Decision, Family, Report};

/// Fixed-workload execution settings. No source text or extra arguments are accepted.
#[derive(Clone, Debug)]
pub struct Request {
    /// The modern zetesis executable; an explicit path, not a PATH search.
    pub executable: PathBuf,
    /// Exact backend to check. Current decoder support is CPU and Metal.
    pub backend: Backend,
    /// Per-child process and cleanup limits.
    pub limits: crate::process::Limits,
}

/// Preparation refused before any conformance result was available.
#[derive(Debug)]
pub enum Error {
    /// Timeout, capture or cleanup limits were zero.
    InvalidLimits,
    /// Temporary source preparation failed.
    Io(io::Error),
    /// The bounded primary-executable identity could not be established.
    Identity(String),
}
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => formatter
                .write_str("backend checks require positive process, capture and cleanup limits"),
            Self::Io(error) => error.fmt(formatter),
            Self::Identity(error) => formatter.write_str(error),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Io(error) = self {
            Some(error)
        } else {
            None
        }
    }
}

/// Check the fixed programs through the installed command's bounded process API.
///
/// Results retain raw capture and explicit nonpasses, including missing devices,
/// partial output and unresolved cleanup. Primary-executable before/after seals
/// do not attest dependencies, transient changes or descendant quiescence.
/// This library performs no global output or report publication.
///
/// # Errors
/// Refuses invalid limits or input preparation failures. Launched invocation,
/// decoding, semantic and route failures belong to the returned report.
pub fn run(request: &Request) -> Result<Report, Error> {
    run_with_cancellation(request, &std::sync::atomic::AtomicBool::new(false))
}

/// Run fixed checks with explicit caller-owned cancellation.
///
/// Cancellation stops the active capture through bounded group cleanup and
/// prevents later children. Retained attempts remain in a nonpassing report;
/// fewer than the required cases never establishes conformance. No signal
/// handlers are installed. The flag must remain set once requested.
///
/// # Errors
/// Returns the same preparation errors as [`run`].
pub fn run_with_cancellation(
    request: &Request,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Report, Error> {
    run::run(request, cancelled)
}
