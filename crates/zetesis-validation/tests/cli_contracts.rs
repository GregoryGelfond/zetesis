//! Process-level failures cannot masquerade as a qualified corpus campaign.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_zetesis-validate"));
    command
        .arg("--repo")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));
    command
}

#[test]
fn invalid_process_limits_and_missing_corpus_are_setup_errors() {
    for (flag, value) in [("--timeout-ms", "0"), ("--max-output-bytes", "0")] {
        let output = command().args([flag, value]).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("ceilings must be positive")
        );
    }
    let directory = tempfile::tempdir().unwrap();
    let output = command()
        .arg("--corpus")
        .arg(directory.path().join("missing"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("corpus ")
    );
}

fn failed_campaign(reference_only: bool, report: Option<&std::path::Path>) -> std::process::Output {
    let directory = tempfile::tempdir().unwrap();
    let mut command = command();
    command
        .arg("--clingo")
        .arg(directory.path().join("absent-reference"));
    command
        .arg("--zetesis")
        .arg(directory.path().join("absent-native"));
    if reference_only {
        command.arg("--reference-only");
    }
    if let Some(path) = report {
        command.arg("--report").arg(path);
    }
    command.output().unwrap()
}

fn check_failed_report(report: &Value, mode: &str) {
    assert_eq!(report["case_count"], 94);
    assert_eq!(report["native_completion_workers"], 1);
    assert_eq!(report["native_max_completion_scratch_bytes"], 268_435_456);
    assert_eq!(report["mode"], mode);
    assert_eq!(report["requested_mode_passed"], false);
    assert_eq!(report["full_native_target_passed"], false);
    assert_eq!(report["status_counts"]["reference_invocation_error"], 94);
    assert!(
        report["cases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|case| case.get("native_process").is_none())
    );
}

#[test]
fn failed_campaign_keeps_machine_readable_evidence_and_failure_exit() {
    let output = failed_campaign(false, None);
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    check_failed_report(&report, "native_full_target");
    assert!(output.stdout.ends_with(b"\n"));

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("failure.json");
    let output = failed_campaign(true, Some(&path));
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let report: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    check_failed_report(&report, "reference_only");
}

#[test]
fn report_write_failure_is_distinct_from_an_incomplete_campaign() {
    let directory = tempfile::tempdir().unwrap();
    let output = failed_campaign(false, Some(directory.path()));
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let diagnostics = String::from_utf8(output.stderr).unwrap();
    assert!(diagnostics.contains("zetesis-validate: report "));
    assert!(directory.path().is_dir());
}

#[test]
fn native_hardware_options_are_user_visible_and_reject_invalid_values() {
    let help = command().arg("--help").output().unwrap();
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    for flag in [
        "--native-backend",
        "--native-batch-size",
        "--native-stats",
        "--native-completion-workers",
        "--native-max-completion-scratch-bytes",
    ] {
        assert!(help.contains(flag));
    }
    for (flag, value) in [
        ("--native-batch-size", "0"),
        ("--native-backend", "cuda"),
        ("--native-completion-workers", "0"),
        ("--native-max-completion-scratch-bytes", "-1"),
        (
            "--native-max-completion-scratch-bytes",
            "18446744073709551616",
        ),
    ] {
        let output = command().args([flag, value]).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains(if value == "-1" {
                    "unexpected argument"
                } else {
                    "invalid value"
                })
        );
    }
    let directory = tempfile::tempdir().unwrap();
    let output = command()
        .args([
            "--native-backend",
            "metal",
            "--native-oracle",
            "countermodel",
            "--native-batch-size",
            "32",
            "--native-completion-workers",
            "4",
            "--native-max-completion-scratch-bytes",
            "0",
        ])
        .arg("--clingo")
        .arg(directory.path().join("absent-reference"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["native_backend"], "metal");
    assert_eq!(report["native_batch_size"], 32);
    assert_eq!(report["native_completion_workers"], 4);
    assert_eq!(report["native_max_completion_scratch_bytes"], 0);
    assert_eq!(report["physical_formula_route_required"], true);
    assert_eq!(report["effective_native_stats"], true);
    assert_eq!(report["full_physical_formula_route_passed"], false);
    assert_eq!(report["full_native_answer_parity_passed"], false);
}

fn corpus_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_zetesis-corpus"))
}

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn default_campaign_reports_clean_source_provenance() {
    let output = failed_campaign(true, None);
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["corpus_view"], "annotation_cleaned");
    assert_eq!(
        report["manifest_sha256"],
        zetesis_validation::examples::MANIFEST_SHA256
    );
    let expected = zetesis_validation::examples::load(
        &repository().join("examples/kr-domains"),
        zetesis_validation::examples::Limits::default(),
    )
    .unwrap();
    for (actual, case) in report["cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected.cases())
    {
        assert_eq!(actual["sha256"], case.source_sha256());
        assert_eq!(actual["original_sha256"], case.original_sha256());
        assert_eq!(
            actual["example_contract"],
            serde_json::to_value(case.contract()).unwrap()
        );
        assert!(actual.get("reference_answer").is_none());
    }
}

#[test]
fn historical_override_reports_original_source_provenance() {
    let directory = tempfile::tempdir().unwrap();
    let output = command()
        .arg("--corpus")
        .arg(repository().join("validation/corpus/kr-domains"))
        .arg("--clingo")
        .arg(directory.path().join("absent-reference"))
        .arg("--reference-only")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["corpus_view"], "original");
    assert_eq!(
        report["manifest_sha256"],
        "a99dafc272fb0047c01f984e27bf22943f2aa5f9c8acf04e4ed1de6ac1a3fe88"
    );
    for case in report["cases"].as_array().unwrap() {
        assert!(case.get("original_sha256").is_none());
        assert!(case.get("example_contract").is_none());
    }
}

#[test]
fn example_verification_does_not_claim_solver_execution() {
    let output = corpus_command()
        .arg("verify-examples")
        .arg(repository().join("examples/kr-domains"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["integrity"], "verified");
    assert_eq!(report["semantic_solver_run"], false);
    assert_eq!(report["originals_verified"], false);
    assert_eq!(report["files"], 108);
    assert_eq!(report["cases"], 94);
    assert_eq!(
        report["manifest_sha256"],
        zetesis_validation::examples::MANIFEST_SHA256
    );
}

#[test]
fn original_audit_is_explicit_in_the_integrity_report() {
    let output = corpus_command()
        .arg("verify-examples")
        .arg(repository().join("examples/kr-domains"))
        .arg("--originals")
        .arg(repository().join("validation/corpus/kr-domains"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["originals_verified"], true);
    assert_eq!(report["semantic_solver_run"], false);
}

#[test]
fn failed_original_audit_emits_no_verified_report() {
    let directory = tempfile::tempdir().unwrap();
    let output = corpus_command()
        .arg("verify-examples")
        .arg(repository().join("examples/kr-domains"))
        .arg("--originals")
        .arg(directory.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("LICENSE")
    );
}

#[test]
fn changed_example_manifest_emits_no_verified_report() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("manifest.json"), b"{}").unwrap();
    let output = corpus_command()
        .arg("verify-examples")
        .arg(directory.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("SHA-256")
    );
}
