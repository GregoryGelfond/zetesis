//! Raw child-process evidence with bounded capture and cleanup polling.
//!
//! The strong backend currently supports Linux and macOS. It starts a fresh
//! process group, drains both pipes without reader threads, and retains at most
//! `max_output_bytes` bytes across the two streams. Completion means that the
//! direct child was reaped and both pipes reached EOF. It proves no descendant
//! termination property: even a same-group descendant can survive after closing
//! both inherited pipes. Process-group termination is attempted only for stopped
//! or faulted capture. Completion says nothing about a solver's enumeration,
//! answer correctness, or system quiescence. Calls require exclusive ownership
//! of the child wait status.
//!
//! Deadlines bound authored polling, not the latency of OS calls or allocation.
//! Retained vectors use fallible amortized growth. Capture bytes exclude vector
//! capacity and OS pipe storage. A stopped capture
//! is partial evidence. UTF-8 conversion is a separate, fallible view.

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::Path;
use std::process::{Child, ExitStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod posix;

pub mod memory;

mod executable;
pub use executable::resolve_executable;

/// Borrowed process arguments. Both paths must be absolute.
#[derive(Clone, Copy, Debug)]
pub struct Invocation<'a> {
    /// Executable selected and resolved by the caller; no shell is inserted.
    pub executable: &'a Path,
    /// Arguments excluding the executable name.
    pub arguments: &'a [OsString],
    /// Working directory, independent of executable resolution.
    pub directory: &'a Path,
}

/// Inclusive raw capture ceiling and independent polling intervals.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Invocation deadline, starting before spawn. Zero permits no polling work.
    pub timeout: Duration,
    /// Retained stdout plus stderr bytes. Zero still permits empty output.
    pub max_output_bytes: usize,
    /// Additional interval for group termination and direct-child reaping.
    pub cleanup_timeout: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_output_bytes: 8 * 1024 * 1024,
            cleanup_timeout: Duration::from_secs(1),
        }
    }
}

/// Why capture polling ended. This is not a solver result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stop {
    /// Direct child reaped and both streams closed without a capture fault.
    /// This establishes no descendant-termination or system-quiescence property.
    Completed,
    /// The invocation deadline was reached.
    Deadline,
    /// The caller requested cancellation; partial capture and cleanup are retained.
    Cancelled,
    /// A byte beyond the combined retained-output ceiling was observed.
    OutputLimit,
    /// A capture or child-lifetime operation failed; inspect the failure fields.
    Failure,
}

/// Direct-child exit evidence, independent of the reason capture stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Exit {
    /// Numeric normal-exit code, absent for signal termination.
    pub code: Option<i32>,
    /// Terminating POSIX signal, absent for normal exit.
    pub signal: Option<i32>,
}

impl From<ExitStatus> for Exit {
    fn from(value: ExitStatus) -> Self {
        #[cfg(unix)]
        let signal = {
            use std::os::unix::process::ExitStatusExt;
            value.signal()
        };
        #[cfg(not(unix))]
        let signal = None;
        Self {
            code: value.code(),
            signal,
        }
    }
}

/// Failed operation; error text is diagnostic rather than a classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// Making a child pipe nonblocking.
    ConfigurePipe,
    /// Reading the standard-output pipe.
    ReadStdout,
    /// Reading the standard-error pipe.
    ReadStderr,
    /// Reserving retained byte storage before copying output.
    RetainBytes,
    /// Observing child termination without consuming its wait status.
    ObserveExit,
    /// Sending a termination signal to the still-owned process group.
    TerminateGroup,
    /// Reaping the direct child without a blocking wait.
    ReapChild,
}

/// An OS or allocation failure associated with a specific capture operation.
#[derive(Debug)]
pub struct Failure {
    operation: Operation,
    cause: io::Error,
}

impl Failure {
    /// Operation that failed.
    #[must_use]
    pub const fn operation(&self) -> Operation {
        self.operation
    }
    /// Original diagnostic error.
    #[must_use]
    pub const fn cause(&self) -> &io::Error {
        &self.cause
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(super) fn new(operation: Operation, cause: io::Error) -> Self {
        Self { operation, cause }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.operation, self.cause)
    }
}
impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// Refusal before a child is started.
#[derive(Debug)]
pub enum StartError {
    /// The caller requested cancellation before the child was started.
    Cancelled,
    /// The strong process-group backend is unavailable on this platform.
    UnsupportedPlatform,
    /// Executable or working-directory path is relative.
    RelativePath,
    /// An invocation or cleanup deadline cannot be represented by `Instant`.
    DeadlineOverflow,
    /// Starting the requested executable failed.
    Spawn(io::Error),
}
impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("process invocation cancelled before spawn"),
            Self::UnsupportedPlatform => {
                f.write_str("bounded process groups require Linux or macOS")
            }
            Self::RelativePath => f.write_str("process executable and directory must be absolute"),
            Self::DeadlineOverflow => f.write_str("process deadline is not representable"),
            Self::Spawn(error) => write!(f, "spawn: {error}"),
        }
    }
}
impl std::error::Error for StartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Spawn(error) = self {
            Some(error)
        } else {
            None
        }
    }
}

/// Immutable retained bytes and direct-child evidence.
#[derive(Debug)]
pub struct Capture {
    child_id: u32,
    stop: Stop,
    exit: Option<Exit>,
    elapsed: Duration,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    failure: Option<Failure>,
    cleanup_failure: Option<Failure>,
}
impl Capture {
    /// Direct child created by this invocation; identity is not a liveness claim.
    #[must_use]
    pub const fn child_id(&self) -> u32 {
        self.child_id
    }
    /// Reason capture polling stopped.
    #[must_use]
    pub const fn stop(&self) -> Stop {
        self.stop
    }
    /// Direct-child exit if successfully reaped.
    #[must_use]
    pub const fn exit(&self) -> Option<Exit> {
        self.exit
    }
    /// Host elapsed time including the initial cleanup attempt.
    #[must_use]
    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }
    /// Exact retained standard-output prefix.
    #[must_use]
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }
    /// Exact retained standard-error prefix.
    #[must_use]
    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
    /// Strict UTF-8 view; no replacement characters are inserted.
    ///
    /// # Errors
    /// Returns the first invalid UTF-8 sequence, including in a stopped prefix.
    pub fn stdout_text(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.stdout)
    }
    /// Strict UTF-8 view of retained standard error.
    ///
    /// # Errors
    /// Returns the first invalid UTF-8 sequence.
    pub fn stderr_text(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.stderr)
    }
    /// Primary capture fault, without replacing a preceding limit/deadline stop.
    #[must_use]
    pub const fn failure(&self) -> Option<&Failure> {
        self.failure.as_ref()
    }
    /// First fault during the bounded cleanup attempt.
    #[must_use]
    pub const fn cleanup_failure(&self) -> Option<&Failure> {
        self.cleanup_failure.as_ref()
    }
}

/// Capture plus any direct child whose cleanup still requires caller action.
#[derive(Debug)]
#[must_use = "inspect capture and handle pending cleanup explicitly"]
pub struct Outcome {
    capture: Capture,
    pending: Option<PendingChild>,
}
impl Outcome {
    /// Evidence retained even if cleanup remains unresolved.
    #[must_use]
    pub const fn capture(&self) -> &Capture {
        &self.capture
    }
    /// Whether bounded cleanup left an owned direct child.
    #[must_use]
    pub const fn cleanup_pending(&self) -> bool {
        self.pending.is_some()
    }
    /// Separate evidence from the explicit cleanup obligation.
    #[must_use]
    pub fn into_parts(self) -> (Capture, Option<PendingChild>) {
        (self.capture, self.pending)
    }
}

/// Exclusive ownership of a direct child not yet successfully reaped.
///
/// `Drop` performs no wait, signal, or thread creation. Call [`Self::retry`] or
/// explicitly [`Self::abandon`] at the application's failure boundary. Neither
/// the handle nor its numeric ID proves that escaped descendants have stopped.
#[derive(Debug)]
#[must_use = "retry or explicitly report abandoned child ownership"]
pub struct PendingChild {
    child: Child,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    group_owned: bool,
}
impl PendingChild {
    /// Numeric direct-child identity retained for failure reporting.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.child.id()
    }
    /// Attempt group termination and nonblocking reaping for one more interval.
    /// A failed attempt returns ownership again, with the first cleanup fault.
    pub fn retry(self, timeout: Duration) -> Cleanup {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            posix::cleanup(self, timeout)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = timeout;
            Cleanup {
                exit: None,
                pending: Some(self),
                failure: None,
            }
        }
    }
    /// Explicitly stop owning cleanup and return the unreaped child ID.
    ///
    /// This is a failure disposition, never successful termination. It performs
    /// no hidden wait and must be retained in the application's failure report.
    #[must_use]
    pub fn abandon(self) -> u32 {
        self.child.id()
    }
}

/// Result of an explicit bounded cleanup attempt.
#[derive(Debug)]
#[must_use = "retain cleanup failure and handle any returned child"]
pub struct Cleanup {
    /// Reaped direct-child exit, when established.
    pub exit: Option<Exit>,
    /// Ownership returned if reaping was not established.
    pub pending: Option<PendingChild>,
    /// First termination or reap failure from this attempt.
    pub failure: Option<Failure>,
}

/// Start and capture one invocation under the strong process-group contract.
///
/// Environment variables are inherited by `Command`; the caller chooses the
/// executable and working directory explicitly. No library stdout/stderr writes
/// occur. Memory retained for pipe bytes is linear in the combined byte ceiling.
///
/// # Errors
/// Returns a typed refusal only before spawning. Every post-spawn failure
/// returns an [`Outcome`] with partial capture and explicit cleanup ownership.
pub fn invoke(invocation: Invocation<'_>, limits: Limits) -> Result<Outcome, StartError> {
    invoke_with_cancellation(invocation, limits, &AtomicBool::new(false))
}

/// Capture a child while observing a caller-owned cancellation flag.
///
/// The caller may set the flag from another thread or its own signal handler.
/// This library installs no handlers. A set flag refuses a new launch; after
/// spawn it stops polling and performs the same bounded, ownership-checked group
/// cleanup as a deadline. Cancellation never shortens the cleanup obligation.
/// The flag must remain set once cancellation is requested.
///
/// # Errors
/// Returns [`StartError::Cancelled`] before spawn, or the refusals of [`invoke`].
pub fn invoke_with_cancellation(
    invocation: Invocation<'_>,
    limits: Limits,
    cancelled: &AtomicBool,
) -> Result<Outcome, StartError> {
    start(invocation, limits, false, cancelled)
}

/// Capture a helper whose descendants inherit its process group.
///
/// Unlike [`invoke`], nonzero or signalled helper completion also attempts group
/// termination while its waitable leader reserves its group ID, before reaping it.
/// This covers failed helpers whose solver closed both output pipes. It does
/// not certify escaped-descendant cleanup or provide their wait status. The
/// trusted helper must return zero only after waiting for its sole solver and
/// publishing the solver's separate exit record. Zero is not a general process
/// tree completion guarantee; the caller must validate that additional record.
/// This extra supervision is excluded from the ordinary timing protocol.
///
/// # Errors
/// Returns the same pre-spawn refusals as [`invoke`]; post-spawn faults retain
/// bounded capture and explicit helper cleanup ownership.
pub fn invoke_supervised(
    invocation: Invocation<'_>,
    limits: Limits,
) -> Result<Outcome, StartError> {
    invoke_supervised_with_cancellation(invocation, limits, &AtomicBool::new(false))
}

/// Supervised helper capture with the cancellation contract of
/// [`invoke_with_cancellation`] and the group ownership of [`invoke_supervised`].
///
/// # Errors
/// Returns the same pre-spawn refusals as [`invoke_with_cancellation`].
pub fn invoke_supervised_with_cancellation(
    invocation: Invocation<'_>,
    limits: Limits,
    cancelled: &AtomicBool,
) -> Result<Outcome, StartError> {
    start(invocation, limits, true, cancelled)
}

fn start(
    invocation: Invocation<'_>,
    limits: Limits,
    supervised: bool,
    cancelled: &AtomicBool,
) -> Result<Outcome, StartError> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(StartError::Cancelled);
    }
    if !invocation.executable.is_absolute() || !invocation.directory.is_absolute() {
        return Err(StartError::RelativePath);
    }
    let started = Instant::now();
    let deadline = started
        .checked_add(limits.timeout)
        .ok_or(StartError::DeadlineOverflow)?;
    deadline
        .checked_add(limits.cleanup_timeout)
        .ok_or(StartError::DeadlineOverflow)?;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        posix::invoke(invocation, limits, started, deadline, supervised, cancelled)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (deadline, supervised);
        Err(StartError::UnsupportedPlatform)
    }
}
