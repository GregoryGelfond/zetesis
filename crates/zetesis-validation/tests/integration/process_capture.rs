//! Public process evidence contracts, using bounded shell fixtures only.

use std::ffi::OsString;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use zetesis_validation::process::{self, Capture, Invocation, Limits, StartError, Stop};

#[test]
fn prior_cancellation_refuses_spawn() {
    let result = process::invoke_with_cancellation(
        Invocation {
            executable: Path::new("/definitely-missing-cancelled-producer"),
            arguments: &[],
            directory: Path::new("/"),
        },
        Limits::default(),
        &AtomicBool::new(true),
    );
    assert!(matches!(result, Err(StartError::Cancelled)));
}

#[test]
fn cancellation_reaps_the_active_group_leader() {
    let directory = tempfile::tempdir().unwrap();
    let ready = directory.path().join("ready");
    let cancelled = AtomicBool::new(false);
    let arguments: Vec<OsString> = vec![
        "-c".into(),
        "printf prefix; : > ready; sleep 5 & wait".into(),
    ];
    let (outcome, observed_ready) = std::thread::scope(|scope| {
        let trigger = scope.spawn(|| {
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while !ready.exists() && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
            let observed = ready.exists();
            cancelled.store(true, Ordering::Relaxed);
            observed
        });
        let outcome = process::invoke_with_cancellation(
            Invocation {
                executable: Path::new("/bin/sh"),
                arguments: &arguments,
                directory: directory.path(),
            },
            Limits {
                timeout: Duration::from_secs(5),
                ..Limits::default()
            },
            &cancelled,
        );
        (outcome, trigger.join().unwrap())
    });
    let (capture, pending) = outcome.unwrap().into_parts();
    // Settle any exceptional cleanup before assertions so the fixture never
    // loses the direct-child owner on a failed assertion.
    let cleanup = pending.map(|child| child.retry(Duration::from_secs(1)));
    if let Some(pending) = cleanup.and_then(|cleanup| cleanup.pending) {
        panic!("fixture cleanup abandoned child {}", pending.abandon());
    }
    assert!(observed_ready);
    assert_eq!(capture.stop(), Stop::Cancelled);
    assert_eq!(capture.exit().unwrap().signal, Some(9));
    assert!(capture.cleanup_failure().is_none());
}

fn capture(script: &str, limit: usize, timeout: Duration) -> Capture {
    let arguments: Vec<OsString> = vec!["-c".into(), script.into()];
    let outcome = process::invoke(
        Invocation {
            executable: Path::new("/bin/sh"),
            arguments: &arguments,
            directory: Path::new("/"),
        },
        Limits {
            timeout,
            max_output_bytes: limit,
            cleanup_timeout: Duration::from_secs(1),
        },
    )
    .unwrap();
    let (capture, pending) = outcome.into_parts();
    if let Some(child) = pending {
        let retry = child.retry(Duration::from_secs(1));
        if let Some(child) = retry.pending {
            panic!("fixture cleanup abandoned child {}", child.abandon());
        }
    }
    if capture.stop() == Stop::Completed {
        assert!(capture.cleanup_failure().is_none(), "{capture:?}");
    }
    capture
}

#[test]
fn completed_capture_retains_nonzero_exit() {
    let result = capture(
        "printf out; printf err >&2; exit 37",
        6,
        Duration::from_secs(2),
    );
    assert_eq!(result.stop(), Stop::Completed);
    assert_eq!(result.exit().unwrap().code, Some(37));
    assert_eq!(result.stdout(), b"out");
    assert_eq!(result.stderr(), b"err");
}

#[test]
fn output_ceiling_counts_both_streams() {
    let result = capture("printf out; printf err >&2", 5, Duration::from_secs(2));
    assert_eq!(result.stop(), Stop::OutputLimit);
    assert_eq!(result.stdout().len() + result.stderr().len(), 5);
}

#[test]
fn exact_output_ceiling_accepts_complete_capture() {
    let result = capture("printf abcdef", 6, Duration::from_secs(2));
    assert_eq!(result.stop(), Stop::Completed);
    assert_eq!(result.stdout(), b"abcdef");
}

#[test]
fn zero_output_ceiling_accepts_empty_streams() {
    let result = capture("exit 0", 0, Duration::from_secs(2));
    assert_eq!(result.stop(), Stop::Completed);
    assert!(result.stdout().is_empty());
    assert!(result.stderr().is_empty());
}

#[test]
fn zero_output_ceiling_refuses_first_byte() {
    let result = capture("printf x", 0, Duration::from_secs(2));
    assert_eq!(result.stop(), Stop::OutputLimit);
    assert!(result.stdout().is_empty());
}

#[test]
fn raw_capture_preserves_non_utf8() {
    let result = capture("printf '\\377'", 1, Duration::from_secs(2));
    assert_eq!(result.stop(), Stop::Completed);
    assert_eq!(result.stdout(), &[255]);
    assert!(result.stdout_text().is_err());
}

#[test]
fn deadline_applies_after_both_pipes_close() {
    let result = capture(
        "exec 1>&- 2>&-; exec sleep 5",
        64,
        Duration::from_millis(100),
    );
    assert_eq!(result.stop(), Stop::Deadline);
    assert_eq!(result.exit().unwrap().signal, Some(9));
}

#[test]
fn deadline_terminates_descendants_holding_pipes() {
    let result = capture(
        "sleep 5 & printf retained; exit 17",
        64,
        Duration::from_millis(100),
    );
    assert_eq!(result.stop(), Stop::Deadline);
    assert_eq!(result.exit().unwrap().code, Some(17));
    assert_eq!(result.stdout(), b"retained");
}

#[test]
fn deadline_retains_both_stream_prefixes() {
    let result = capture(
        "printf out; printf err >&2; exec sleep 5",
        64,
        Duration::from_millis(100),
    );
    assert_eq!(result.stop(), Stop::Deadline);
    assert_eq!(result.stdout(), b"out");
    assert_eq!(result.stderr(), b"err");
}

#[test]
fn missing_executable_is_a_pre_spawn_failure() {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("absent");
    let result = process::invoke(
        Invocation {
            executable: &executable,
            arguments: &[],
            directory: directory.path(),
        },
        Limits::default(),
    );
    assert!(matches!(result, Err(StartError::Spawn(_))));
}

#[test]
fn relative_executable_is_not_resolved_by_the_library() {
    let result = process::invoke(
        Invocation {
            executable: Path::new("sh"),
            arguments: &[],
            directory: Path::new("/"),
        },
        Limits::default(),
    );
    assert!(matches!(result, Err(StartError::RelativePath)));
}

#[test]
fn completed_process_does_not_establish_semantic_completion() {
    let result = capture(
        "printf 'SATISFIABLE\\nCoverage: partial\\nModels: 0\\n'",
        128,
        Duration::from_secs(2),
    );
    assert_eq!(result.stop(), Stop::Completed);
    assert!(
        zetesis_validation::answers::native_text(
            result.stdout(),
            false,
            zetesis_validation::answers::Limits::default()
        )
        .is_err()
    );
}

#[test]
fn deadline_range_is_admitted_before_executable_start() {
    let directory = tempfile::tempdir().unwrap();
    let absent = directory.path().join("must-not-start");
    for limits in [
        Limits {
            timeout: Duration::MAX,
            ..Limits::default()
        },
        Limits {
            cleanup_timeout: Duration::MAX,
            ..Limits::default()
        },
    ] {
        let result = process::invoke(
            Invocation {
                executable: &absent,
                arguments: &[],
                directory: directory.path(),
            },
            limits,
        );
        assert!(matches!(result, Err(StartError::DeadlineOverflow)));
    }
}

#[test]
fn relative_working_directory_is_not_resolved_implicitly() {
    let result = process::invoke(
        Invocation {
            executable: Path::new("/bin/sh"),
            arguments: &[],
            directory: Path::new("."),
        },
        Limits::default(),
    );
    assert!(matches!(result, Err(StartError::RelativePath)));
}
