//! Settle owned children before any fallible capture rendering or retention.

use std::{error::Error, fmt, fs, path::Path, time::Duration};

use serde_json::json;
use zetesis_validation::process::{Capture, Outcome, PendingChild, Stop};

use super::{Result, digest, json_file, require};

pub(super) struct Retained {
    pub(super) capture: Capture,
    pub(super) output: Vec<u8>,
    pub(super) log: String,
    pub(super) layout: &'static str,
    pub(super) sha256: String,
}

#[derive(Debug)]
struct Cleanup {
    attempted: bool,
    exit: Option<zetesis_validation::process::Exit>,
    failure: Option<zetesis_validation::process::Failure>,
    unreaped_child: Option<u32>,
}

impl Cleanup {
    fn succeeded(&self) -> bool {
        (!self.attempted || self.exit.is_some())
            && self.failure.is_none()
            && self.unreaped_child.is_none()
    }
}

fn settle(pending: Option<PendingChild>) -> Cleanup {
    let Some(child) = pending else {
        return Cleanup {
            attempted: false,
            exit: None,
            failure: None,
            unreaped_child: None,
        };
    };
    let cleanup = child.retry(Duration::from_secs(2));
    Cleanup {
        attempted: true,
        exit: cleanup.exit,
        failure: cleanup.failure,
        unreaped_child: cleanup.pending.map(PendingChild::abandon),
    }
}

#[derive(Debug)]
struct RetentionFailure {
    identity: String,
    cleanup: Cleanup,
    cause: Option<Box<dyn Error>>,
}

impl fmt::Display for RetentionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}; cleanup disposition={:?}",
            self.identity, self.cleanup
        )?;
        if let Some(cause) = &self.cause {
            write!(formatter, "; retention failed: {cause}")?;
        }
        Ok(())
    }
}

impl Error for RetentionFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause.as_deref()
    }
}

pub(super) fn retain(
    outcome: Outcome,
    stage: &Path,
    evidence: &Path,
    name: &str,
) -> Result<Retained> {
    let (capture, pending) = outcome.into_parts();
    // No encoding, allocation for a log, or filesystem operation may return
    // before the pending handle is either reaped or explicitly reported.
    let cleanup = settle(pending);
    let identity = format!(
        "{name}: child {}; stop={:?}; exit={:?}; capture failure={:?}; initial cleanup failure={:?}",
        capture.child_id(),
        capture.stop(),
        capture.exit(),
        capture.failure(),
        capture.cleanup_failure()
    );
    let retained = (|| {
        // Raw streams precede every fallible UTF-8/rendered view. A failure may
        // leave partial files, but cannot publish a successful proof record.
        fs::write(evidence.join("stdout"), capture.stdout())?;
        fs::write(evidence.join("stderr"), capture.stderr())?;
        json_file(
            &evidence.join("result.json"),
            &json!({
                "child_id": capture.child_id(), "stop": capture.stop(), "exit": capture.exit(),
                "elapsed_seconds": capture.elapsed().as_secs_f64(),
                "capture_failure": capture.failure().map(ToString::to_string),
                "initial_cleanup_failure": capture.cleanup_failure().map(ToString::to_string),
                "cleanup_retry_attempted": cleanup.attempted, "cleanup_retry_exit": cleanup.exit,
                "cleanup_retry_failure": cleanup.failure.as_ref().map(ToString::to_string),
                "unreaped_child": cleanup.unreaped_child
            }),
        )?;
        write(capture, stage, name)
    })();
    match retained {
        Ok(retained) if cleanup.succeeded() => Ok(retained),
        Ok(_) => Err(RetentionFailure {
            identity,
            cleanup,
            cause: None,
        }
        .into()),
        Err(cause) => Err(RetentionFailure {
            identity,
            cleanup,
            cause: Some(cause),
        }
        .into()),
    }
}

fn write(capture: Capture, stage: &Path, name: &str) -> Result<Retained> {
    let mut output = capture.stdout().to_vec();
    output.extend_from_slice(capture.stderr());
    // Preserve cargo's trailing blank lines in an exact, reversible view.
    let (log, layout, rendered) = if name == "record-regressions" {
        let view = json!({
            "stdout": std::str::from_utf8(capture.stdout())?,
            "stderr": std::str::from_utf8(capture.stderr())?
        });
        let mut encoded = serde_json::to_vec_pretty(&view)?;
        encoded.push(b'\n');
        (
            format!("verification/current/{name}.json"),
            "json_stdout_and_stderr",
            encoded,
        )
    } else {
        (
            format!("verification/current/{name}.log"),
            "stdout_then_stderr",
            output.clone(),
        )
    };
    let path = stage.join(&log);
    fs::write(&path, rendered)?;
    require(
        capture.stop() == Stop::Completed,
        &format!("{name} capture stopped: {:?}", capture.stop()),
    )?;
    require(
        capture
            .exit()
            .is_some_and(|exit| exit.code == Some(0) && exit.signal.is_none()),
        &format!("{name} command failed: {:?}", capture.exit()),
    )?;
    if name == "audit" {
        require(capture.stderr().is_empty(), "Audit emitted stderr")?;
    }
    Ok(Retained {
        capture,
        output,
        log,
        layout,
        sha256: digest(&path)?,
    })
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use zetesis_validation::process::{self, Invocation, Limits};

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "zetesis-proof-refresh-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::create_dir(path.join("raw")).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn pending(directory: &Path, script: &str) -> Outcome {
        // Zero cleanup allowance may leave a signalled direct child pending.
        // Require an actual owned outcome; already-reaped attempts are complete
        // and may be retried. Bound fixture work without assuming scheduler order.
        for _ in 0..8 {
            let outcome = process::invoke(
                Invocation {
                    executable: Path::new("/bin/sh"),
                    arguments: &["-c".into(), script.into()],
                    directory,
                },
                Limits {
                    timeout: Duration::from_millis(100),
                    max_output_bytes: 4096,
                    cleanup_timeout: Duration::ZERO,
                },
            )
            .unwrap();
            if outcome.cleanup_pending() {
                return outcome;
            }
            assert!(outcome.capture().exit().is_some());
        }
        panic!("bounded fixture did not yield an owned pending child");
    }

    #[test]
    fn log_write_failure_reaps_the_pending_child() {
        let directory = Directory::new();
        let outcome = pending(&directory.0, "exec /bin/sleep 5");
        let child = outcome.capture().child_id();
        let Err(error) = retain(
            outcome,
            &directory.0.join("missing"),
            &directory.0.join("raw"),
            "build",
        ) else {
            panic!("missing log directory must fail");
        };
        assert!(error.to_string().contains(&format!("child {child}")));
        let failure = error.downcast_ref::<RetentionFailure>().unwrap();
        assert_eq!(
            failure
                .cause
                .as_ref()
                .unwrap()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .kind(),
            std::io::ErrorKind::NotFound
        );
        assert!(failure.cleanup.attempted);
        assert_eq!(failure.cleanup.exit.unwrap().signal, Some(9));
        assert!(failure.cleanup.succeeded());
    }

    #[test]
    fn encoding_failure_reaps_the_pending_child() {
        let directory = Directory::new();
        fs::create_dir_all(directory.0.join("verification/current")).unwrap();
        let outcome = pending(&directory.0, "printf '\\377'; exec /bin/sleep 5");
        let child = outcome.capture().child_id();
        let Err(error) = retain(
            outcome,
            &directory.0,
            &directory.0.join("raw"),
            "record-regressions",
        ) else {
            panic!("non-UTF-8 record stream must fail");
        };
        assert!(error.to_string().contains(&format!("child {child}")));
        let failure = error.downcast_ref::<RetentionFailure>().unwrap();
        assert!(failure.cause.as_ref().unwrap().is::<std::str::Utf8Error>());
        assert_eq!(fs::read(directory.0.join("raw/stdout")).unwrap(), [255]);
        assert!(failure.cleanup.attempted);
        assert_eq!(failure.cleanup.exit.unwrap().signal, Some(9));
        assert!(failure.cleanup.succeeded());
    }

    #[test]
    fn regression_logs_preserve_both_byte_streams() {
        let directory = Directory::new();
        fs::create_dir_all(directory.0.join("verification/current")).unwrap();
        let outcome = process::invoke(
            Invocation {
                executable: Path::new("/bin/sh"),
                arguments: &[
                    "-c".into(),
                    "printf 'out\\n\\n'; printf 'err\\n\\n' >&2".into(),
                ],
                directory: &directory.0,
            },
            Limits {
                timeout: Duration::from_secs(2),
                max_output_bytes: 4096,
                cleanup_timeout: Duration::from_secs(2),
            },
        )
        .unwrap();
        let retained = retain(
            outcome,
            &directory.0,
            &directory.0.join("raw"),
            "record-regressions",
        )
        .unwrap();
        let rendered: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.0.join(retained.log)).unwrap()).unwrap();
        assert_eq!(retained.layout, "json_stdout_and_stderr");
        assert_eq!(rendered, json!({"stdout": "out\n\n", "stderr": "err\n\n"}));
        assert_eq!(retained.output, b"out\n\nerr\n\n");
    }

    fn completed(directory: &Path, script: &str) -> Outcome {
        process::invoke(
            Invocation {
                executable: Path::new("/bin/sh"),
                arguments: &["-c".into(), script.into()],
                directory,
            },
            Limits {
                timeout: Duration::from_secs(2),
                max_output_bytes: 4096,
                cleanup_timeout: Duration::from_secs(2),
            },
        )
        .unwrap()
    }

    #[test]
    fn failed_child_exit_cannot_claim_success() {
        let directory = Directory::new();
        fs::create_dir_all(directory.0.join("verification/current")).unwrap();
        let outcome = completed(&directory.0, "printf 'failure'; exit 7");
        let error = retain(outcome, &directory.0, &directory.0.join("raw"), "build")
            .err()
            .unwrap();
        assert!(error.to_string().contains("code: Some(7)"));
        assert_eq!(
            fs::read(directory.0.join("verification/current/build.log")).unwrap(),
            b"failure"
        );
    }

    #[test]
    fn audit_stderr_cannot_claim_success() {
        let directory = Directory::new();
        fs::create_dir_all(directory.0.join("verification/current")).unwrap();
        let outcome = completed(&directory.0, "printf 'warning' >&2");
        let error = retain(outcome, &directory.0, &directory.0.join("raw"), "audit")
            .err()
            .unwrap();
        assert!(error.to_string().contains("Audit emitted stderr"));
    }
}
