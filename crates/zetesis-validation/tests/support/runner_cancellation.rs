//! Cancellation stops process work without reclassifying completed prefixes.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::corpus_comparison::{CaptureFailure, Decision, NativeInvocation, Producer};

#[test]
fn prior_cancellation_launches_no_corpus_children() {
    let directory = tempfile::tempdir().unwrap();
    let mut options = super::options(directory.path());
    options.clingo = directory.path().join("must-not-run-reference");
    options.zetesis = directory.path().join("must-not-run-native");
    let report = super::super::run_with_cancellation(
        &options,
        super::loaded(directory.path(), 94),
        NativeInvocation::Legacy,
        &AtomicBool::new(true),
        |_| panic!("no case may start"),
    );
    assert!(report.cancelled());
    assert!(report.cases().is_empty());
    assert!(!report.passed());
    assert_eq!(report.to_json().unwrap()["cancelled"], true);
}

#[test]
fn cancellation_between_cases_preserves_the_completed_prefix() {
    let directory = tempfile::tempdir().unwrap();
    let options = super::options(directory.path());
    let cancelled = AtomicBool::new(false);
    let report = super::super::run_with_cancellation(
        &options,
        super::loaded(directory.path(), 94),
        NativeInvocation::Legacy,
        &cancelled,
        |_| cancelled.store(true, Ordering::Relaxed),
    );
    assert!(report.cancelled());
    assert_eq!(report.cases().len(), 1);
    assert_eq!(report.cases()[0].decision(), &Decision::Passed);
    assert!(!report.passed());
}

#[test]
fn active_cancellation_retains_the_stopped_producer() {
    for producer in [Producer::Reference, Producer::Native] {
        let directory = tempfile::tempdir().unwrap();
        let mut options = super::options(directory.path());
        options.timeout_ms = 20_000;
        let waiting = super::script(
            directory.path(),
            "waiting",
            "printf partial; : > started; exec sleep 30",
        );
        match producer {
            Producer::Reference => options.clingo = waiting,
            Producer::Native => options.zetesis = waiting,
        }
        let cancelled = AtomicBool::new(false);
        let (report, observed_start) = std::thread::scope(|scope| {
            let marker = directory.path().join("started");
            let flag = &cancelled;
            let stop = scope.spawn(move || {
                let deadline = std::time::Instant::now() + super::FIXTURE_LIVENESS;
                while !marker.exists() && std::time::Instant::now() < deadline {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                let observed = marker.exists();
                flag.store(true, Ordering::Relaxed);
                observed
            });
            let report = super::super::run_with_cancellation(
                &options,
                super::loaded(directory.path(), 94),
                NativeInvocation::Legacy,
                &cancelled,
                |_| {},
            );
            (report, stop.join().unwrap())
        });
        assert!(observed_start, "fixture never entered its active wait");
        assert!(report.cancelled());
        assert!(!report.passed());
        assert_eq!(report.cases().len(), 1);
        assert_eq!(
            report.cases()[0].decision(),
            &Decision::CaptureFailed(producer, CaptureFailure::Cancelled)
        );
        let view = report.to_json().unwrap();
        let field = match producer {
            Producer::Reference => "reference_process",
            Producer::Native => "native_process",
        };
        let capture = &view["cases"][0][field];
        assert_eq!(capture["status"], "cancelled");
        // The marker establishes the producer's write, not pipe-read admission.
        // Cancellation stops polling before another read; an empty admitted
        // prefix is valid even when the pipe still holds the complete payload.
        assert!("partial".starts_with(capture["stdout"].as_str().unwrap()));
        assert_eq!(capture["stderr"], "");
        assert!(capture["exit_code"].is_null());
        assert_eq!(capture["exit_signal"], 9);
        assert!(capture["capture_failure"].is_null());
        assert!(capture["pending_child_id"].is_null());
        assert!(capture["cleanup_failure"].is_null());
        if producer == Producer::Reference {
            assert!(view["cases"][0].get("native_process").is_none());
        } else {
            assert_eq!(view["cases"][0]["reference_answer"]["model_count"], 1);
        }
    }
}

#[test]
fn cancelled_capture_start_has_no_synthetic_output() {
    let error = crate::corpus_comparison::capture::invoke_with_cancellation(
        std::path::Path::new("/must-not-resolve"),
        &[],
        std::path::Path::new("/"),
        super::FIXTURE_LIVENESS,
        128,
        &AtomicBool::new(true),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        crate::corpus_comparison::decision::InvocationFailure::Cancelled
    ));
}
