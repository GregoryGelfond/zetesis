//! Historical text/JSON report view over the shared raw capture implementation.

use std::ffi::OsString;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::decision::{CaptureStatus, InvocationFailure};
use super::exit::ExitEvidence;
use crate::process::{self, Invocation, Limits, PendingChild, Stop};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct Capture {
    pub(crate) status: CaptureStatus,
    #[serde(flatten)]
    pub(super) exit: ExitEvidence,
    pub(crate) elapsed_ms: u128,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    /// Explicitly lossy legacy views never qualify as completed text.
    pub(crate) lossy_text: bool,
    pub(crate) stdout_bytes: Vec<u8>,
    pub(crate) stderr_bytes: Vec<u8>,
    #[serde(rename = "capture_failure")]
    pub(crate) failure: Option<String>,
    pub(crate) cleanup_failure: Option<String>,
    pub(crate) pending_child_id: Option<u32>,
    #[serde(skip)]
    pub(crate) pending: Option<PendingChild>,
}

impl Capture {
    pub(crate) const fn cleanup_unresolved(&self) -> bool {
        self.pending_child_id.is_some()
    }
}

#[cfg(test)]
pub(super) fn invoke(
    executable: &Path,
    arguments: &[OsString],
    directory: &Path,
    timeout: Duration,
    output_limit: usize,
) -> Result<Capture, InvocationFailure> {
    invoke_with_cancellation(
        executable,
        arguments,
        directory,
        timeout,
        output_limit,
        &AtomicBool::new(false),
    )
}

pub(super) fn invoke_with_cancellation(
    executable: &Path,
    arguments: &[OsString],
    directory: &Path,
    timeout: Duration,
    output_limit: usize,
    cancelled: &AtomicBool,
) -> Result<Capture, InvocationFailure> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(InvocationFailure::Cancelled);
    }
    let search_path = std::env::var_os("PATH");
    let executable = process::resolve_executable(executable, search_path.as_deref())
        .map_err(|error| error.to_string())?;
    let directory = std::path::absolute(directory).map_err(|error| error.to_string())?;
    let outcome = process::invoke_with_cancellation(
        Invocation {
            executable: &executable,
            arguments,
            directory: &directory,
        },
        Limits {
            timeout,
            max_output_bytes: output_limit,
            cleanup_timeout: Duration::from_secs(1),
        },
        cancelled,
    )
    .map_err(|error| match error {
        process::StartError::Cancelled => InvocationFailure::Cancelled,
        error => InvocationFailure::Other(format!("{}: {error}", executable.display())),
    })?;
    let (capture, pending) = outcome.into_parts();
    let utf8 = capture.stdout_text().and_then(|_| capture.stderr_text());
    let lossy_text = utf8.is_err();
    let status = match capture.stop() {
        Stop::Completed if lossy_text => CaptureStatus::InvalidUtf8,
        Stop::Completed => CaptureStatus::Completed,
        Stop::Cancelled => CaptureStatus::Cancelled,
        Stop::Deadline => CaptureStatus::Timeout,
        Stop::OutputLimit => CaptureStatus::OutputLimit,
        Stop::Failure => CaptureStatus::CaptureFailure,
    };
    Ok(Capture {
        status,
        exit: ExitEvidence(capture.exit()),
        elapsed_ms: capture.elapsed().as_millis(),
        stdout: String::from_utf8_lossy(capture.stdout()).into_owned(),
        stderr: String::from_utf8_lossy(capture.stderr()).into_owned(),
        lossy_text,
        stdout_bytes: if lossy_text {
            capture.stdout().to_vec()
        } else {
            Vec::new()
        },
        stderr_bytes: if lossy_text {
            capture.stderr().to_vec()
        } else {
            Vec::new()
        },
        failure: capture.failure().map(ToString::to_string),
        cleanup_failure: capture.cleanup_failure().map(ToString::to_string),
        pending_child_id: pending.as_ref().map(PendingChild::id),
        pending,
    })
}
