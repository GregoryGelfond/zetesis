//! Shared bounded raw invocation; interpretation belongs to each campaign.
use std::ffi::OsString;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::Capture;
use crate::{process, selected::InvocationFailure};

pub(super) fn unix_ns() -> Option<u128> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_nanos())
}

pub(super) fn invoke(
    executable: &Path,
    arguments: Vec<OsString>,
    directory: &Path,
    limits: process::Limits,
) -> (Capture, Option<String>) {
    let mut cleanup_fault = None;
    let mut record = Capture {
        executable: executable.into(),
        arguments,
        directory: directory.into(),
        started_unix_ns: unix_ns(),
        elapsed_ns: None,
        stop: None,
        exit: None,
        stdout: Vec::new(),
        stderr: Vec::new(),
        failure: None,
        cleanup_failure: None,
        unresolved_child: None,
    };
    match process::invoke(
        process::Invocation {
            executable,
            arguments: &record.arguments,
            directory,
        },
        limits,
    ) {
        Err(error) => record.failure = Some(InvocationFailure::start(&error)),
        Ok(outcome) => {
            let (capture, pending) = outcome.into_parts();
            record.stop = Some(capture.stop());
            record.exit = capture.exit();
            record.elapsed_ns = Some(capture.elapsed().as_nanos());
            record.stdout = capture.stdout().to_vec();
            record.stderr = capture.stderr().to_vec();
            record.failure = capture.failure().map(InvocationFailure::capture);
            record.cleanup_failure = capture.cleanup_failure().map(InvocationFailure::capture);
            if let Some(child) = pending {
                let cleanup = child.retry(limits.cleanup_timeout);
                record.exit = cleanup.exit.or(record.exit);
                if let Some(failure) = cleanup.failure {
                    cleanup_fault = Some(failure.to_string());
                }
                if let Some(child) = cleanup.pending {
                    let id = child.abandon();
                    record.unresolved_child = Some(id);
                }
            }
        }
    }
    (record, cleanup_fault)
}
