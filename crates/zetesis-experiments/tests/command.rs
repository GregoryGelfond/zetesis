//! Installed-command evidence must retain the library's completion boundary.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::path::Path;
use std::time::Duration;
use zetesis_validation::process::{self, Capture, Invocation, Limits, Stop};

fn invoke(arguments: &[&str]) -> Capture {
    let arguments = arguments
        .iter()
        .map(std::ffi::OsString::from)
        .collect::<Vec<_>>();
    let outcome = process::invoke(
        Invocation {
            executable: Path::new(env!("CARGO_BIN_EXE_zetesis-experiments")),
            arguments: &arguments,
            directory: Path::new(env!("CARGO_MANIFEST_DIR")),
        },
        Limits {
            timeout: Duration::from_secs(10),
            cleanup_timeout: Duration::from_secs(1),
            max_output_bytes: 1_048_576,
        },
    )
    .unwrap();
    let (capture, pending) = outcome.into_parts();
    if let Some(child) = pending {
        let cleanup = child.retry(Duration::from_secs(1));
        if let Some(child) = cleanup.pending {
            panic!("benchmark test left unreaped child {}", child.abandon());
        }
        panic!(
            "benchmark test required cleanup retry: {:?}",
            cleanup.failure
        );
    }
    assert_eq!(capture.stop(), Stop::Completed);
    assert!(capture.failure().is_none());
    assert!(capture.cleanup_failure().is_none());
    capture
}

#[test]
fn formula_command_publishes_only_qualified_membership() {
    let capture = invoke(&[
        "formula",
        "--backend",
        "cpu",
        "--atoms",
        "1",
        "--batches",
        "1",
        "--families",
        "choices",
        "--repetitions",
        "1",
        "--cpu-workers",
        "1",
    ]);
    assert_eq!(capture.exit().unwrap().code, Some(0));
    assert!(capture.stderr().is_empty());
    let report = capture.stdout_text().unwrap();
    assert!(report.contains("source_grounding=excluded outer_search=excluded"));
    assert!(report.contains("GPU_execution=not_requested"));
    assert!(report.ends_with(
        "# status=PASS scope=synthetic-membership all_completed_results_match_native=true\n"
    ));
}

#[test]
fn lazy_command_publishes_complete_checked_batches() {
    let capture = invoke(&[
        "lazy",
        "--backend",
        "cpu",
        "--widths",
        "1",
        "--batches",
        "1",
        "--families",
        "sparse",
        "--warmups",
        "0",
        "--repetitions",
        "1",
    ]);
    assert_eq!(capture.exit().unwrap().code, Some(0));
    assert!(capture.stderr().is_empty());
    let records = capture
        .stdout_text()
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    let complete = records.last().unwrap();
    assert_eq!(complete["event"], "complete");
    assert_eq!(complete["samples"], 8);
}

#[test]
fn lazy_command_cannot_hide_incomplete_reference_work() {
    let capture = invoke(&[
        "lazy",
        "--backend",
        "cpu",
        "--widths",
        "1",
        "--batches",
        "1",
        "--families",
        "sparse",
        "--max-work",
        "0",
    ]);
    assert_eq!(capture.exit().unwrap().code, Some(2));
    assert!(!capture.stderr().is_empty());
    let output = capture.stdout_text().unwrap();
    assert!(!output.contains("\"event\":\"complete\""));
}

#[test]
fn static_command_publishes_only_qualified_membership() {
    let capture = invoke(&[
        "--backend",
        "cpu",
        "--atoms",
        "1",
        "--batches",
        "1",
        "--families",
        "wide",
        "--repetitions",
        "1",
        "--workers",
        "1",
    ]);
    assert_eq!(capture.exit().unwrap().code, Some(0));
    assert!(capture.stderr().is_empty());
    let report = capture.stdout_text().unwrap();
    assert!(report.contains("GPU_execution=not_requested"));
    assert!(report.contains("# status=PASS"));
}

#[test]
fn grounding_command_retains_complete_model_evidence() {
    let capture = invoke(&[
        "grounding",
        "tests/fixtures/grounding/identity.lp",
        "--repetitions",
        "1",
    ]);
    assert_eq!(capture.exit().unwrap().code, Some(0));
    assert!(capture.stderr().is_empty());
    let report: serde_json::Value = serde_json::from_slice(capture.stdout()).unwrap();
    assert_eq!(report["complete"], true);
    assert_eq!(
        report["qualification"]["interpretations"],
        serde_json::json!([[0, 1], [0, 1, 2]])
    );
}

#[test]
fn projection_command_refuses_cpu_only_selection() {
    let capture = invoke(&["formula-projection", "--backend", "cpu"]);
    assert_eq!(capture.exit().unwrap().code, Some(2));
    assert!(capture.stdout().is_empty());
    assert!(
        capture
            .stderr_text()
            .unwrap()
            .contains("formula-projection requires --backend metal or vulkan")
    );
}

#[test]
fn incomplete_formula_command_has_no_pass_marker() {
    let capture = invoke(&[
        "formula",
        "--backend",
        "cpu",
        "--atoms",
        "1",
        "--batches",
        "1",
        "--families",
        "choices",
        "--repetitions",
        "1",
        "--cpu-workers",
        "1",
        "--max-work",
        "0",
    ]);
    assert_eq!(capture.exit().unwrap().code, Some(2));
    assert!(!capture.stdout_text().unwrap().contains("status=PASS"));
    assert!(!capture.stderr().is_empty());
}

#[test]
fn refused_grounding_command_retains_typed_failure() {
    let capture = invoke(&[
        "grounding",
        "tests/fixtures/grounding/objective.lp",
        "--repetitions",
        "1",
    ]);
    assert_eq!(capture.exit().unwrap().code, Some(2));
    let report: serde_json::Value = serde_json::from_slice(capture.stdout()).unwrap();
    assert_eq!(report["complete"], false);
    assert_eq!(report["failure"]["code"], "objective_unsupported");
    assert!(
        capture
            .stderr_text()
            .unwrap()
            .contains("requires objective-free source")
    );
}

#[test]
fn missing_grounding_source_remains_a_process_error() {
    let capture = invoke(&["grounding", "tests/fixtures/grounding/missing-source.lp"]);
    assert_eq!(capture.exit().unwrap().code, Some(2));
    let report: serde_json::Value = serde_json::from_slice(capture.stdout()).unwrap();
    assert_eq!(report["complete"], false);
    assert_eq!(report["failure"]["code"], "source");
    assert!(capture.stderr_text().unwrap().contains("missing-source.lp"));
}

#[test]
fn tight_command_publishes_complete_occurrence_evidence() {
    let capture = invoke(&[
        "tight",
        "--backend",
        "cpu",
        "--atoms",
        "4",
        "--batches",
        "3",
        "--families",
        "normal",
        "--warmups",
        "0",
        "--repetitions",
        "1",
        "--workers",
        "1",
    ]);
    assert_eq!(capture.exit().unwrap().code, Some(0));
    assert!(capture.stderr().is_empty());
    let records = capture
        .stdout_text()
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(records.last().unwrap()["samples"], 4);
    let samples = records
        .iter()
        .filter(|record| record["event"] == "sample")
        .collect::<Vec<_>>();
    assert_eq!(samples.len(), 4);
    for sample in samples {
        assert_eq!(sample["sample"]["outcomes"].as_array().unwrap().len(), 3);
    }
}

#[test]
fn tight_command_preserves_failed_reference_exit() {
    let capture = invoke(&[
        "tight",
        "--backend",
        "cpu",
        "--atoms",
        "4",
        "--batches",
        "1",
        "--max-work",
        "0",
    ]);
    assert_eq!(capture.exit().unwrap().code, Some(2));
    assert!(!capture.stderr().is_empty());
    let records = capture
        .stdout_text()
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert!(!records.iter().any(|record| record["event"] == "complete"));
}
