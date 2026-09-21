//! Historical text/JSON report view over the shared raw capture implementation.

use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::decision::{CaptureStatus, InvocationFailure};
use crate::process::{self, Invocation, Limits, PendingChild, Stop};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct Capture {
    pub(crate) status: CaptureStatus,
    pub(crate) exit_code: Option<i32>,
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
    let executable = resolve(executable)?;
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
        exit_code: capture.exit().and_then(|exit| exit.code),
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

fn resolve(executable: &Path) -> Result<PathBuf, String> {
    if executable.components().count() != 1 || executable.is_absolute() {
        return std::path::absolute(executable).map_err(|error| error.to_string());
    }
    // Resolve anew for each invocation under this comparison's PATH policy.
    // The shared capture API accepts only a selected absolute executable.
    let path = std::env::var_os("PATH").ok_or("PATH is absent")?;
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join(executable);
        if std::fs::metadata(&candidate)
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        {
            return std::path::absolute(candidate).map_err(|error| error.to_string());
        }
    }
    Err(format!("executable not found: {}", executable.display()))
}
