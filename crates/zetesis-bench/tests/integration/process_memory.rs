//! Fresh Rust helpers qualify resource scope independently of solver execution.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use zetesis_validation::process::{
    self, Invocation, Limits, Stop,
    memory::{Measurement, Unit},
};

fn run(
    script: &str,
    output_bytes: usize,
    timeout: Duration,
) -> (process::Capture, Option<Measurement>) {
    let temporary = tempfile::tempdir().unwrap();
    let record = temporary.path().join("rss.json");
    let capture = run_supervised_helper(
        temporary.path(),
        &record,
        &["/bin/sh".into(), "-c".into(), script.into()],
        output_bytes,
        timeout,
    );
    let measurement = std::fs::read(record)
        .ok()
        .map(|bytes| serde_json::from_slice(&bytes).unwrap());
    (capture, measurement)
}

fn run_supervised_helper(
    directory: &Path,
    record: &Path,
    child: &[OsString],
    output_bytes: usize,
    timeout: Duration,
) -> process::Capture {
    let mut arguments = vec!["__measure-child".into(), record.as_os_str().into()];
    arguments.extend_from_slice(child);
    let outcome = process::invoke_supervised(
        Invocation {
            executable: Path::new(env!("CARGO_BIN_EXE_zetesis-bench")),
            arguments: &arguments,
            directory,
        },
        Limits {
            timeout,
            max_output_bytes: output_bytes,
            cleanup_timeout: Duration::from_secs(1),
        },
    )
    .unwrap();
    let (capture, pending) = outcome.into_parts();
    assert!(pending.is_none(), "{capture:?}");
    capture
}

fn run_finished_helper(directory: &Path, record: &Path, child: &[OsString]) -> process::Capture {
    // These refusal fixtures cannot leave a live solver: path/spawn refusal
    // happens before a child exists, and create_new follows measure's wait for
    // its sole builtin-only shell child. Ordinary capture still bounds time,
    // output and cleanup; it does not request the extra failed-helper group
    // sweep required by the general measurement/descendant tests above/below.
    let mut arguments = vec!["__measure-child".into(), record.as_os_str().into()];
    arguments.extend_from_slice(child);
    let outcome = process::invoke(
        Invocation {
            executable: Path::new(env!("CARGO_BIN_EXE_zetesis-bench")),
            arguments: &arguments,
            directory,
        },
        Limits {
            timeout: Duration::from_secs(3),
            max_output_bytes: 1024,
            cleanup_timeout: Duration::from_secs(1),
        },
    )
    .unwrap();
    let (capture, pending) = outcome.into_parts();
    assert!(pending.is_none(), "{capture:?}");
    assert!(capture.failure().is_none(), "{capture:?}");
    assert!(capture.cleanup_failure().is_none(), "{capture:?}");
    capture
}

#[test]
fn absent_child_cannot_publish_a_resource_record() {
    let directory = tempfile::tempdir().unwrap();
    let record = directory.path().join("rss.json");
    let absent = directory.path().join("absent-child");
    let capture = run_finished_helper(directory.path(), &record, &[absent.into_os_string()]);
    assert_eq!(capture.stop(), Stop::Completed, "{capture:?}");
    assert_eq!(capture.exit().unwrap().code, Some(2));
    assert!(capture.stdout().is_empty());
    assert!(
        capture
            .stderr()
            .starts_with(b"measurement helper: child RSS operation: ")
    );
    assert!(!record.exists());
}

#[test]
fn relative_child_is_not_resolved_by_the_resource_helper() {
    let directory = tempfile::tempdir().unwrap();
    let record = directory.path().join("rss.json");
    let capture = run_finished_helper(directory.path(), &record, &["sh".into()]);
    assert_eq!(capture.stop(), Stop::Completed, "{capture:?}");
    assert_eq!(capture.exit().unwrap().code, Some(2));
    assert_eq!(
        capture.stderr(),
        b"measurement helper: child RSS executable and directory must be absolute\n"
    );
    assert!(!record.exists());
}

#[test]
fn resource_publication_preserves_an_existing_record() {
    let directory = tempfile::tempdir().unwrap();
    let record = directory.path().join("rss.json");
    std::fs::write(&record, b"previous resource evidence").unwrap();
    let capture = run_finished_helper(
        directory.path(),
        &record,
        &[
            "/bin/sh".into(),
            "-c".into(),
            "printf completed-child; exit 37".into(),
        ],
    );
    assert_eq!(capture.stop(), Stop::Completed, "{capture:?}");
    assert_eq!(capture.exit().unwrap().code, Some(2));
    assert_eq!(capture.stdout(), b"completed-child");
    assert!(capture.stderr().starts_with(b"measurement helper: "));
    assert_eq!(
        std::fs::read(record).unwrap(),
        b"previous resource evidence"
    );
}

#[test]
fn resource_record_identifies_the_solver_child() {
    let (capture, measurement) = run("printf '%s' \"$$\"", 1024, Duration::from_secs(3));
    assert_eq!(capture.stop(), Stop::Completed, "{capture:?}");
    let measurement = measurement.unwrap();
    assert_eq!(measurement.child.to_string().as_bytes(), capture.stdout());
    assert!(measurement.valid());
}

#[test]
fn helper_success_does_not_replace_a_failed_solver_exit() {
    let (capture, measurement) = run("exit 37", 1024, Duration::from_secs(3));
    assert_eq!(capture.exit().unwrap().code, Some(0));
    assert_eq!(measurement.unwrap().exit_code, Some(37));
}

#[test]
fn signalled_solver_retains_signal_evidence() {
    let (capture, measurement) = run("kill -TERM $$", 1024, Duration::from_secs(3));
    assert_eq!(capture.stop(), Stop::Completed, "{capture:?}");
    let measurement = measurement.unwrap();
    assert_eq!(measurement.signal, Some(15));
    assert_eq!(measurement.exit_code, None);
}

#[test]
fn native_peak_units_are_converted_to_bytes() {
    let (_, measurement) = run("printf child", 1024, Duration::from_secs(3));
    let measurement = measurement.unwrap();
    assert!(measurement.peak_rss_bytes > 0);
    if cfg!(target_os = "linux") {
        assert_eq!(measurement.raw_unit, Unit::Kibibytes);
        assert_eq!(measurement.peak_rss_bytes, measurement.raw_max_rss * 1024);
    } else {
        assert_eq!(measurement.raw_unit, Unit::Bytes);
        assert_eq!(measurement.peak_rss_bytes, measurement.raw_max_rss);
    }
}

#[test]
fn memory_output_limits_retain_the_solver_prefix() {
    let (capture, _) = run("printf 123456789; sleep 5", 4, Duration::from_secs(2));
    assert_eq!(capture.stop(), Stop::OutputLimit);
    assert_eq!(capture.stdout(), b"1234");
}

#[test]
fn memory_deadline_stops_a_solver_with_closed_pipes() {
    let (capture, measurement) = run("exec 1>&- 2>&-; sleep 5", 64, Duration::from_millis(80));
    assert_eq!(capture.stop(), Stop::Deadline);
    assert!(measurement.is_none());
}

#[test]
fn failed_helper_terminates_its_remaining_group_member() {
    let directory = tempfile::tempdir().unwrap();
    let arguments: Vec<OsString> = vec!["-c".into(),
        "(exec >/dev/null 2>&1; printf ready > started; sleep 0.2; printf leaked > leaked) & while [ ! -f started ]; do sleep 0.001; done; exit 7".into()];
    let outcome = process::invoke_supervised(
        Invocation {
            executable: Path::new("/bin/sh"),
            arguments: &arguments,
            directory: directory.path(),
        },
        Limits::default(),
    )
    .unwrap();
    let (capture, pending) = outcome.into_parts();
    assert!(pending.is_none());
    assert_eq!(capture.stop(), Stop::Completed, "{capture:?}");
    assert_eq!(capture.exit().unwrap().code, Some(7));
    assert!(directory.path().join("started").exists());
    std::thread::sleep(Duration::from_millis(400));
    assert!(!directory.path().join("leaked").exists());
}

fn valid_record() -> Measurement {
    Measurement {
        schema: 1,
        child: 10,
        exit_code: Some(0),
        signal: None,
        raw_max_rss: 2,
        raw_unit: Unit::Kibibytes,
        peak_rss_bytes: 2048,
    }
}

#[test]
fn contradictory_unit_conversion_is_invalid() {
    assert!(
        !Measurement {
            peak_rss_bytes: 2,
            ..valid_record()
        }
        .valid()
    );
}

#[test]
fn overflowing_unit_conversion_is_invalid() {
    assert!(
        !Measurement {
            raw_max_rss: u64::MAX,
            ..valid_record()
        }
        .valid()
    );
}

#[test]
fn contradictory_exit_evidence_is_invalid() {
    assert!(
        !Measurement {
            signal: Some(9),
            ..valid_record()
        }
        .valid()
    );
}
