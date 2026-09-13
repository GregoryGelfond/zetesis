//! Real shell orchestration with synthetic compilers and physical-test output.
#![cfg(all(
    any(target_os = "linux", target_os = "macos"),
    feature = "test-fixtures"
))]
#[path = "support/process.rs"]
pub mod subprocess;
use subprocess::{Command, Output};
#[path = "support/coverage_fixture.rs"]
mod fixture;
use fixture::{Fixture, groups};
use std::fs;

#[test]
fn portable_profiles_keep_the_exact_separate_schedule() {
    let f = Fixture::new();
    let result = f.coverage("gate", false, &[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    f.assert_complete_schedule("gate", false, "91");
}
#[test]
fn physical_profiles_keep_the_exact_separate_schedule() {
    let f = Fixture::new();
    let result = f.coverage("gate", true, &[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    f.assert_complete_schedule("gate", true, "91");
}
#[test]
fn baseline_measurements_never_apply_a_floor() {
    for metal in [false, true] {
        let f = Fixture::new();
        f.write("scripts/coverage-floor.txt", b"UNMEASURED\n");
        let result = f.coverage("baseline", metal, &[]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        f.assert_complete_schedule("baseline", metal, "UNMEASURED");
    }
}
#[test]
fn incomplete_tool_versions_cannot_reuse_success() {
    for (key, value) in [
        ("COVERAGE_TEST_CARGO_VERSION", "cargo-llvm-cov 0.8.6"),
        ("COVERAGE_TEST_LLVM_VERSION", "22.1.5"),
    ] {
        let f = Fixture::new();
        let result = f.coverage("gate", false, &[(key, value)]);
        f.assert_preflight_failure(&result);
    }
}
#[test]
fn invalid_floor_cannot_reuse_success() {
    for (mode, floor) in [("gate", "UNMEASURED"), ("baseline", "NaN")] {
        let f = Fixture::new();
        f.write("scripts/coverage-floor.txt", floor.as_bytes());
        let result = f.coverage(mode, false, &[]);
        f.assert_preflight_failure(&result);
    }
}
#[test]
fn llvm_overrides_require_both_tools() {
    for key in ["LLVM_COV", "LLVM_PROFDATA"] {
        let f = Fixture::new();
        let result = f.coverage("gate", false, &[(key, "/missing")]);
        f.assert_preflight_failure(&result);
    }
}
#[test]
fn paired_llvm_overrides_preserve_schedule() {
    let f = Fixture::new();
    let cov = f.root().join("sysroot/lib/rustlib/fake-host/bin/llvm-cov");
    let prof = f
        .root()
        .join("sysroot/lib/rustlib/fake-host/bin/llvm-profdata");
    let result = f.coverage(
        "gate",
        false,
        &[
            ("LLVM_COV", cov.to_str().unwrap()),
            ("LLVM_PROFDATA", prof.to_str().unwrap()),
        ],
    );
    assert!(result.status.success());
    f.assert_complete_schedule("gate", false, "91");
}
#[test]
fn bundled_llvm_versions_preserve_schedule() {
    let f = Fixture::new();
    let result = f.coverage(
        "gate",
        false,
        &[("COVERAGE_TEST_LLVM_VERSION", "22.1.6-rust-1.97.1-stable")],
    );
    assert!(result.status.success());
    f.assert_complete_schedule("gate", false, "91");
}
#[test]
fn omitted_physical_group_prevents_instrumentation() {
    for group in [
        "lazy",
        "relation",
        "relation-measurement",
        "context",
        "solve-context",
        "session-resources",
        "language-consumers",
        "static",
    ] {
        let f = Fixture::new();
        let row = groups()
            .into_iter()
            .find(|fields| fields[0] == group)
            .unwrap()
            .join("|");
        // Remove the record while preserving the shell literal's closing quote,
        // including when the omitted group is the final table row.
        let original = f.read("scripts/coverage.sh");
        let script = original.replacen(&format!("\n{row}"), "", 1);
        assert_ne!(script, original);
        f.write("scripts/coverage.sh", script.as_bytes());
        let result = f.coverage("gate", true, &[]);
        f.assert_preflight_failure(&result);
    }
}
#[test]
fn every_physical_group_rejects_zero_matches() {
    for fields in groups() {
        let f = Fixture::new();
        let result = f.coverage(
            "gate",
            true,
            &[
                ("COVERAGE_TEST_PHYSICAL", "empty"),
                ("COVERAGE_TEST_PHYSICAL_GROUP", fields[0]),
            ],
        );
        assert!(!result.status.success());
        f.assert_physical_failure(fields[0]);
    }
}
#[test]
fn altered_physical_records_prevent_completion() {
    for mode in [
        "missing",
        "wrong-name",
        "duplicate-name",
        "ignored",
        "unreported",
        "missing-outcome",
        "failed-outcome",
        "extra-summary",
        "failed-summary",
    ] {
        let f = Fixture::new();
        let result = f.coverage("gate", true, &[("COVERAGE_TEST_PHYSICAL", mode)]);
        assert!(!result.status.success(), "{mode}");
        f.assert_physical_failure("wgpu-lib");
    }
}
#[test]
fn failed_physical_execution_preserves_its_exit_code() {
    for group in [
        "wgpu-lib",
        "aggregate-measurement",
        "relation",
        "relation-measurement",
        "solve-context",
        "session-resources",
        "language-consumers",
        "static",
    ] {
        let f = Fixture::new();
        let result = f.coverage(
            "gate",
            true,
            &[
                ("COVERAGE_TEST_PHYSICAL", "failed"),
                ("COVERAGE_TEST_PHYSICAL_GROUP", group),
            ],
        );
        assert_eq!(result.status.code(), Some(37));
        f.assert_physical_failure(group);
    }
}
#[test]
fn every_instrumentation_failure_keeps_status_incomplete() {
    for phase in [
        "clean",
        "test",
        "json",
        "html",
        "build-cli-cpu:test",
        "gate",
        "build-cli-cpu:gate",
    ] {
        let f = Fixture::new();
        let result = f.coverage("gate", false, &[("COVERAGE_TEST_FAIL", phase)]);
        assert_eq!(result.status.code(), Some(37));
        f.assert_status("incomplete");
    }
}
#[test]
fn final_output_failure_keeps_coverage_incomplete() {
    for mode in ["gate", "baseline"] {
        let f = Fixture::new();
        let script = f.read("scripts/coverage.sh");
        let completion =
            "printf 'Coverage %s completed; reports: %s\\n' \"$mode\" \"$coverage_dir\"";
        assert_eq!(script.matches(completion).count(), 1);
        // Earlier tool output succeeds; only the final progress write loses stdout.
        f.write(
            "scripts/coverage.sh",
            script
                .replacen(completion, &format!("exec 1>&-\n{completion}"), 1)
                .as_bytes(),
        );
        let result = f.coverage(mode, false, &[]);
        assert!(!result.status.success());
        f.assert_status("incomplete");
        for profile in ["workspace", "cli-cpu"] {
            for report in ["coverage.json", "html/index.html"] {
                assert!(
                    f.root()
                        .join(format!("target/coverage/{profile}/{report}"))
                        .is_file()
                );
            }
        }
    }
}
#[test]
fn concurrent_run_preserves_the_existing_owner() {
    let f = Fixture::new();
    fs::create_dir(f.root().join("target/coverage/.lock")).unwrap();
    let result = f.coverage("gate", false, &[]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(f.read("target/coverage/status.txt"), "gate-passed\n");
    assert!(f.root().join("target/coverage/.lock").is_dir());
    assert!(f.calls().is_empty());
}
#[test]
fn invalid_coverage_arguments_are_refused() {
    for args in [
        vec!["bad"],
        vec!["gate", "--bad"],
        vec!["gate", ""],
        vec!["gate", "--metal", "extra"],
    ] {
        let f = Fixture::new();
        let result = f.command("scripts/coverage.sh").args(args).bounded_output();
        assert_eq!(result.status.code(), Some(2));
    }
}
#[test]
fn invalid_check_arguments_are_refused() {
    for args in [
        vec!["portable", "--metal"],
        vec!["full", "--metal"],
        vec!["coverage", "--bad"],
        vec!["coverage", ""],
        vec!["coverage", "--metal", "extra"],
    ] {
        let f = Fixture::new();
        let result = f.command("scripts/check.sh").args(args).bounded_output();
        assert_eq!(result.status.code(), Some(2));
    }
}
#[test]
fn optional_coverage_gate_forwards_failure() {
    let f = Fixture::new();
    f.write(
        "scripts/coverage.sh",
        b"#!/bin/sh\nprintf '<%s>\\n' \"$@\"\nexit 37\n",
    );
    let result = f
        .command("scripts/check.sh")
        .args(["coverage", "--metal"])
        .bounded_output();
    assert_eq!(result.status.code(), Some(37));
    assert_eq!(result.stdout, b"<gate>\n<--metal>\n");
}
#[test]
fn record_gate_follows_actual_lean_commands() {
    for mode in ["proofs", "full"] {
        for failure in ["", "build", "audit", "record"] {
            let f = Fixture::new();
            f.tool("bin/maintenance", "maintenance");
            f.tool("scripts/coverage.sh", "coverage.sh");
            f.tool("scripts/validate.sh", "validate.sh");
            let result = f
                .command("scripts/check.sh")
                .arg(mode)
                .env("ZETESIS_MAINTENANCE", f.root().join("bin/maintenance"))
                .env("CHECK_TEST_TRACE", f.root().join("trace"))
                .env("CHECK_TEST_FAILURE", failure)
                .bounded_output();
            assert_eq!(
                result.status.code(),
                Some(if failure.is_empty() { 0 } else { 23 }),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let trace = f.read("trace");
            let lines: Vec<_> = trace.lines().collect();
            let gate = "maintenance proof-record";
            if matches!(failure, "build" | "audit") {
                assert!(!lines.contains(&gate));
            } else {
                let index = lines.iter().position(|line| *line == gate).unwrap();
                assert_eq!(lines.iter().filter(|line| **line == gate).count(), 1);
                assert_eq!(
                    lines[index - 1],
                    "lake env lean -DautoImplicit=false -DwarningAsError=true Audit.lean"
                );
                if mode == "full" && failure.is_empty() {
                    assert!(
                        lines
                            .iter()
                            .position(|line| line.starts_with("coverage.sh"))
                            .unwrap()
                            > index
                    );
                } else {
                    assert!(!lines.iter().any(|line| line.starts_with("coverage.sh")));
                }
            }
        }
    }
}

#[test]
fn book_checks_retain_the_repository_toolchain() {
    let fixture = Fixture::new();
    let result = fixture
        .command("scripts/check.sh")
        .arg("book")
        .env_remove("RUSTUP_TOOLCHAIN")
        .env("CHECK_TEST_BOOK_TOOLCHAIN", "1.97.1-fake-host")
        .env("CHECK_TEST_TRACE", fixture.root().join("trace"))
        .bounded_output();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(fixture.read("trace").contains("mdbook test --library-path"));
}

#[test]
fn portable_test_campaigns_collect_target_failures() {
    let fixture = Fixture::new();
    let result = fixture
        .command("scripts/check.sh")
        .arg("portable")
        .env("CHECK_TEST_TRACE", fixture.root().join("trace"))
        .bounded_output();
    assert!(result.status.success());
    let trace = fixture.read("trace");
    let campaigns: Vec<_> = trace
        .lines()
        .filter(|line| line.starts_with("cargo test "))
        .collect();
    assert_eq!(campaigns.len(), 8);
    assert!(campaigns.iter().all(|line| line.contains("--no-fail-fast")));
}

fn workflow_block() -> String {
    let workflow = include_str!("../../../.github/workflows/checks.yml");
    let lines: Vec<_> = workflow.lines().collect();
    let matches: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim() == "- name: Preserve the committed floor")
        .collect();
    assert_eq!(matches.len(), 1);
    let index = matches[0].0 + 1;
    assert_eq!(lines[index].trim(), "run: |");
    let mut block = String::new();
    for line in &lines[index + 1..] {
        if !line.trim().is_empty() && !line.starts_with("          ") {
            break;
        }
        block.push_str(line.strip_prefix("          ").unwrap_or(line));
        block.push('\n');
    }
    assert!(!block.trim().is_empty());
    block
}
fn git(f: &Fixture, arguments: &[&str]) -> Output {
    let hooks = f.root().join("empty-hooks");
    fs::create_dir_all(&hooks).unwrap();
    let result = Command::new("git")
        .current_dir(f.root())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args([
            "-c",
            &format!("core.hooksPath={}", hooks.display()),
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.name=Coverage Test",
            "-c",
            "user.email=coverage@example.invalid",
        ])
        .args(arguments)
        .bounded_output();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    result
}
#[test]
fn workflow_ratchet_preserves_the_committed_floor() {
    let block = workflow_block();
    for (previous, current, success) in [
        (None, "91", true),
        (Some("UNMEASURED"), "91", true),
        (Some("91"), "91", true),
        (Some("91"), "92", true),
        (Some("91"), "90", false),
        (Some("91"), "UNMEASURED", false),
        (Some("91"), "garbage", false),
    ] {
        let f = Fixture::new();
        git(&f, &["init", "-q", "--template="]);
        f.write("initial", b"local fixture\n");
        if let Some(floor) = previous {
            f.write("scripts/coverage-floor.txt", floor.as_bytes());
        } else {
            fs::remove_file(f.root().join("scripts/coverage-floor.txt")).unwrap();
        }
        git(&f, &["add", "initial", "scripts"]);
        git(&f, &["commit", "-qm", "local coverage-floor fixture"]);
        let commit = String::from_utf8(git(&f, &["rev-parse", "HEAD"]).stdout).unwrap();
        f.write("scripts/coverage-floor.txt", current.as_bytes());
        for event in [
            serde_json::json!({"before":commit.trim()}),
            serde_json::json!({"pull_request":{"base":{"sha":commit.trim()}}}),
        ] {
            f.write("event.json", event.to_string().as_bytes());
            let result = f
                .command("-c")
                .arg(&block)
                .env("GITHUB_EVENT_PATH", f.root().join("event.json"))
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .bounded_output();
            assert_eq!(
                result.status.success(),
                success,
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(f.read("scripts/coverage-floor.txt"), current);
        }
    }
}
#[test]
fn unreadable_existing_history_cannot_count_as_adoption() {
    let f = Fixture::new();
    f.write(
        "event.json",
        serde_json::json!({"before":"a".repeat(40)})
            .to_string()
            .as_bytes(),
    );
    f.write("bin/git",b"#!/bin/sh\ncase \"$1\" in show) exit 17;; ls-tree) printf 'tracked floor\\n';; *) exit 19;; esac\n");
    f.executable("bin/git");
    let result = f
        .command("scripts/coverage-ratchet.sh")
        .env("GITHUB_EVENT_PATH", f.root().join("event.json"))
        .bounded_output();
    assert!(!result.status.success());
}
#[test]
fn failed_history_lookup_cannot_count_as_adoption() {
    let f = Fixture::new();
    f.write(
        "event.json",
        serde_json::json!({"before":"a".repeat(40)})
            .to_string()
            .as_bytes(),
    );
    f.write("bin/git", b"#!/bin/sh\nexit 17\n");
    f.executable("bin/git");
    let result = f
        .command("scripts/coverage-ratchet.sh")
        .env("GITHUB_EVENT_PATH", f.root().join("event.json"))
        .bounded_output();
    assert_eq!(result.status.code(), Some(17));
}
#[test]
fn mock_report_parser_refuses_unsupported_feature_flags() {
    for arguments in [
        vec!["--all-features"],
        vec!["--no-default-features"],
        vec!["--features", "gpu"],
        vec!["--workspace"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_zetesis-maintenance-fixture"))
            .args(["cargo", "+1.97.1", "llvm-cov", "report"])
            .args(arguments)
            .bounded_output();
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("invalid mock report option"));
    }
}

#[test]
fn oracle_campaigns_continue_after_independent_failures() {
    for failed in ["", "arithmetic_validation", "arithmetic_validation,formula"] {
        let fixture = Fixture::new();
        let result = fixture
            .command("scripts/check.sh")
            .arg("oracle")
            .env("CHECK_TEST_TRACE", fixture.root().join("trace"))
            .env("CHECK_TEST_ORACLE_FAILURE", failed)
            .bounded_output();
        assert_eq!(
            result.status.code(),
            Some(if failed.is_empty() { 0 } else { 23 })
        );
        let trace = fixture.read("trace");
        let campaigns: Vec<_> = trace
            .lines()
            .filter(|line| line.starts_with("cargo test "))
            .collect();
        assert_eq!(campaigns.len(), 13);
        assert!(campaigns.iter().all(|line| line.contains("--no-fail-fast")));
        let records: Vec<_> = fs::read_dir(fixture.root().join("target/oracle-checks"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(records.len(), 1);
        for index in 1..=13 {
            let expected =
                if (index == 1 && !failed.is_empty()) || (index == 13 && failed.contains(',')) {
                    "23\n"
                } else {
                    "0\n"
                };
            assert_eq!(
                fs::read_to_string(records[0].join(format!("{index}.exit"))).unwrap(),
                expected
            );
            let arguments = fs::read_to_string(records[0].join(format!("{index}.argv"))).unwrap();
            assert_eq!(
                arguments.lines().collect::<Vec<_>>().join(" "),
                campaigns[index - 1]
            );
        }
        assert_eq!(
            fs::read_to_string(records[0].join("status.txt")).unwrap(),
            if failed.is_empty() {
                "passed\n"
            } else {
                "failed\n"
            }
        );
    }
}

#[test]
fn oracle_setup_refusal_prevents_test_execution() {
    for defect in [
        "missing",
        "relative",
        "foreign",
        "absent-path",
        "nonexecutable",
        "version",
        "execution",
    ] {
        let fixture = Fixture::new();
        let mut command = fixture.command("scripts/check.sh");
        command
            .arg("oracle")
            .env("CHECK_TEST_TRACE", fixture.root().join("trace"));
        match defect {
            "missing" => {
                command.env_remove("CLINGO");
            }
            "relative" => {
                command.env("CLINGO", "bin/clingo");
            }
            "foreign" => {
                fixture.tool("other/clingo", "clingo");
                command.env("CLINGO", fixture.root().join("other/clingo"));
            }
            "absent-path" => {
                fixture.tool("other/clingo", "clingo");
                fs::remove_file(fixture.root().join("bin/clingo")).unwrap();
                command.env("PATH", fixture.root().join("bin"));
                command.env("CLINGO", fixture.root().join("other/clingo"));
            }
            "nonexecutable" => {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(
                    fixture.root().join("bin/clingo"),
                    fs::Permissions::from_mode(0o600),
                )
                .unwrap();
            }
            "version" => {
                command.env("CHECK_TEST_CLINGO_VERSION", "5.8.1");
            }
            "execution" => {
                command.env("CHECK_TEST_CLINGO_FAILURE", "version");
            }
            _ => unreachable!(),
        }
        let result = command.bounded_output();
        assert_eq!(result.status.code(), Some(2), "{defect}");
        let trace = fs::read_to_string(fixture.root().join("trace")).unwrap_or_default();
        assert!(!trace.contains("cargo test "), "{defect}: {trace}");
        assert!(!fixture.root().join("target/oracle-checks").exists());
    }
}
