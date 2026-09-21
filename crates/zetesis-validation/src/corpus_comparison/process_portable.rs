//! Direct-child cancellation and bounded cleanup with capped temporary capture.
//!
//! This fallback does not own descendant processes. Failed reaping records the
//! abandoned direct-child ID; an unclosed pipe is an explicit cleanup failure.
//! Reader threads may outlive that failure until their external pipe closes.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use super::decision::{CaptureStatus, InvocationFailure};
use super::exit::ExitEvidence;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct Capture {
    pub(crate) status: CaptureStatus,
    #[serde(flatten)]
    pub(super) exit: ExitEvidence,
    pub(crate) elapsed_ms: u128,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    #[serde(rename = "capture_failure")]
    pub(crate) failure: Option<String>,
    pub(crate) cleanup_failure: Option<String>,
    /// Unreaped direct child explicitly abandoned after the cleanup ceiling.
    pub(crate) pending_child_id: Option<u32>,
}

impl Capture {
    pub(crate) fn cleanup_unresolved(&self) -> bool {
        self.pending_child_id.is_some() || self.cleanup_failure.is_some()
    }
}

#[cfg(all(test, unix))]
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
    let stdout = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
    let stderr = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
    let stdout_copy = stdout.reopen().map_err(|error| error.to_string())?;
    let stderr_copy = stderr.reopen().map_err(|error| error.to_string())?;
    let started = Instant::now();
    let deadline = started
        .checked_add(timeout)
        .ok_or("timeout is not representable")?;
    let search_path = std::env::var_os("PATH");
    let executable = crate::process::resolve_executable(executable, search_path.as_deref())
        .map_err(|error| error.to_string())?;
    if cancelled.load(Ordering::Relaxed) {
        return Err(InvocationFailure::Cancelled);
    }
    let mut child = Command::new(&executable)
        .args(arguments)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("spawn {}: {error}", executable.display()))?;
    let remaining = Arc::new(AtomicUsize::new(output_limit));
    let exceeded = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    drain(
        child.stdout.take().ok_or("missing child stdout")?,
        stdout_copy,
        remaining.clone(),
        exceeded.clone(),
        sender.clone(),
    );
    drain(
        child.stderr.take().ok_or("missing child stderr")?,
        stderr_copy,
        remaining,
        exceeded.clone(),
        sender,
    );
    let mut capture = monitor(&mut child, deadline, &exceeded, cancelled);
    // Drain completion is bounded separately after killing/exiting. The command
    // operates on trusted solver executables, without an intermediate shell.
    for _ in 0..2 {
        match receiver.recv_timeout(Duration::from_secs(1)) {
            Ok(Ok(())) => (),
            Ok(Err(error)) => {
                capture
                    .failure
                    .get_or_insert_with(|| format!("child capture: {error}"));
            }
            Err(_) => {
                capture.cleanup_failure.get_or_insert_with(|| {
                    "child capture did not close after process termination".into()
                });
            }
        }
    }
    if capture.status == CaptureStatus::Completed && exceeded.load(Ordering::Relaxed) {
        capture.status = CaptureStatus::OutputLimit;
    }
    // Reopened readers own independent cursors. If a pipe did not close, a
    // bounded prefix can be inspected without rewinding its active writer.
    let complete = capture.status == CaptureStatus::Completed && !capture.cleanup_unresolved();
    for (file, target) in [
        (&stdout, &mut capture.stdout),
        (&stderr, &mut capture.stderr),
    ] {
        match read_capture(file, complete, output_limit) {
            Ok(text) => *target = text,
            Err(error) => {
                capture.failure.get_or_insert(error);
            }
        }
    }
    if capture.status == CaptureStatus::Completed
        && (capture.failure.is_some() || capture.cleanup_unresolved())
    {
        capture.status = CaptureStatus::CaptureFailure;
    }
    capture.elapsed_ms = started.elapsed().as_millis();
    Ok(capture)
}

fn monitor(
    child: &mut std::process::Child,
    deadline: Instant,
    exceeded: &AtomicBool,
    cancelled: &AtomicBool,
) -> Capture {
    let mut capture = Capture {
        status: CaptureStatus::Completed,
        exit: ExitEvidence(None),
        elapsed_ms: 0,
        stdout: String::new(),
        stderr: String::new(),
        failure: None,
        cleanup_failure: None,
        pending_child_id: None,
    };
    loop {
        if cancelled.load(Ordering::Relaxed) {
            capture.status = CaptureStatus::Cancelled;
        } else if exceeded.load(Ordering::Relaxed) {
            capture.status = CaptureStatus::OutputLimit;
        } else if Instant::now() >= deadline {
            capture.status = CaptureStatus::Timeout;
        } else {
            match child.try_wait() {
                Ok(Some(exit)) => {
                    capture.exit = ExitEvidence(Some(exit.into()));
                    return capture;
                }
                Ok(None) => {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(error) => {
                    capture.status = CaptureStatus::CaptureFailure;
                    capture.failure = Some(error.to_string());
                }
            }
        }
        break;
    }
    if let Err(error) = child.kill() {
        capture.cleanup_failure = Some(format!("direct-child kill: {error}"));
    }
    let cleanup_deadline = Instant::now().checked_add(Duration::from_secs(1));
    loop {
        match child.try_wait() {
            Ok(Some(exit)) => {
                capture.exit = ExitEvidence(Some(exit.into()));
                break;
            }
            Ok(None) if cleanup_deadline.is_some_and(|deadline| Instant::now() < deadline) => {
                thread::sleep(Duration::from_millis(5));
            }
            outcome => {
                capture.pending_child_id = Some(child.id());
                capture
                    .cleanup_failure
                    .get_or_insert_with(|| match outcome {
                        Err(error) => format!("direct-child reap: {error}"),
                        _ => "direct-child cleanup deadline reached".into(),
                    });
                break;
            }
        }
    }
    capture
}

fn drain(
    mut input: impl Read + Send + 'static,
    mut output: std::fs::File,
    remaining: Arc<AtomicUsize>,
    exceeded: Arc<AtomicBool>,
    sender: mpsc::Sender<std::io::Result<()>>,
) {
    thread::spawn(move || {
        let result = (|| {
            let mut buffer = [0u8; 8192];
            loop {
                let count = input.read(&mut buffer)?;
                if count == 0 {
                    return Ok(());
                }
                let previous = remaining
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |available| {
                        Some(available.saturating_sub(count))
                    })
                    .expect("the atomic update closure always returns Some");
                let retained = previous.min(count);
                output.write_all(&buffer[..retained])?;
                if retained != count {
                    exceeded.store(true, Ordering::Relaxed);
                    return Ok(());
                }
            }
        })();
        let _ = sender.send(result);
    });
}

fn read_capture(
    file: &tempfile::NamedTempFile,
    require_utf8: bool,
    limit: usize,
) -> Result<String, String> {
    let mut file = file
        .reopen()
        .map_err(|error| error.to_string())?
        .take(u64::try_from(limit).unwrap_or(u64::MAX));
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if require_utf8 {
        String::from_utf8(bytes)
            .map_err(|error| format!("solver emitted non-UTF-8 output: {error}"))
    } else {
        // A hard byte ceiling can split a multibyte character. Such partial
        // evidence is never normalized or accepted as a solver result.
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::{CaptureStatus, invoke};
    use std::path::Path;
    use std::time::Duration;

    #[test]
    fn signal_termination_retains_portable_exit_evidence() {
        let captured = invoke(
            Path::new("/bin/sh"),
            &["-c".into(), "kill -TERM $$".into()],
            Path::new("/"),
            Duration::from_secs(2),
            128,
        )
        .unwrap();
        assert_eq!(captured.status, CaptureStatus::Completed);
        assert_eq!(
            captured.exit.0,
            Some(crate::process::Exit {
                code: None,
                signal: Some(15)
            })
        );
        assert!(!captured.cleanup_unresolved());
        let json = serde_json::to_value(captured).unwrap();
        assert!(json["exit_code"].is_null());
        assert_eq!(json["exit_signal"], 15);
    }

    #[test]
    fn normal_exit_retains_portable_exit_evidence() {
        let captured = invoke(
            Path::new("/bin/sh"),
            &["-c".into(), "exit 17".into()],
            Path::new("/"),
            Duration::from_secs(2),
            128,
        )
        .unwrap();
        assert_eq!(
            captured.exit.0,
            Some(crate::process::Exit {
                code: Some(17),
                signal: None
            })
        );
        let json = serde_json::to_value(captured).unwrap();
        assert_eq!(json["exit_code"], 17);
        assert!(json["exit_signal"].is_null());
    }

    #[test]
    fn capture_limit_bounds_both_streams_and_preserves_exit_classification() {
        let captured = invoke(
            Path::new("/bin/sh"),
            &["-c".into(), "printf '%050000d' 0".into()],
            Path::new("/"),
            Duration::from_secs(2),
            128,
        )
        .unwrap();
        assert_eq!(captured.status, CaptureStatus::OutputLimit);
        assert!(captured.stdout.len() + captured.stderr.len() <= 128);
    }

    #[test]
    fn timeout_kills_the_direct_child() {
        let captured = invoke(
            Path::new("/bin/sh"),
            &["-c".into(), "exec sleep 2".into()],
            Path::new("/"),
            Duration::from_millis(10),
            128,
        )
        .unwrap();
        assert_eq!(captured.status, CaptureStatus::Timeout);
        assert!(captured.elapsed_ms < 1_000);
    }

    #[test]
    fn cancellation_retains_portable_capture() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let directory = tempfile::tempdir().unwrap();
        let cancelled = AtomicBool::new(false);
        let (captured, observed) = std::thread::scope(|scope| {
            let marker = directory.path().join("started");
            let flag = &cancelled;
            let sender = scope.spawn(move || {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                while !marker.exists() && std::time::Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(5));
                }
                let observed = marker.exists();
                flag.store(true, Ordering::Relaxed);
                observed
            });
            let captured = super::invoke_with_cancellation(
                Path::new("/bin/sh"),
                &[
                    "-c".into(),
                    "printf partial; : > started; exec sleep 30".into(),
                ],
                directory.path(),
                Duration::from_secs(10),
                128,
                &cancelled,
            )
            .unwrap();
            (captured, sender.join().unwrap())
        });
        assert!(observed);
        assert_eq!(captured.status, CaptureStatus::Cancelled);
        assert_eq!(captured.stdout, "partial");
        assert!(!captured.cleanup_unresolved());
    }
}
