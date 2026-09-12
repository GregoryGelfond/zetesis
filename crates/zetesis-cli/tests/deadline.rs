//! Process deadlines preserve the solver's incomplete-result contract.

use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(limit: Option<&str>, json: bool, oracle: &str) -> Output {
    let mut source = tempfile::NamedTempFile::new().unwrap();
    writeln!(source, "{{a;b}}.").unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_zetesis"));
    command.arg(source.path()).args([
        "--backend",
        "cpu",
        "--oracle",
        oracle,
        "--color",
        "never",
        "--stats",
        "--models",
        "0",
    ]);
    if let Some(limit) = limit {
        command.args(["--time-limit", limit]);
    }
    if json {
        command.arg("--json");
    }
    command.stdin(Stdio::null()).output().unwrap()
}

#[test]
fn expired_process_deadlines_report_incomplete() {
    for oracle in ["auto", "countermodel"] {
        for json in [false, true] {
            let result = run(Some("0"), json, oracle);
            let diagnostics = String::from_utf8(result.stderr).unwrap();
            assert_eq!(result.status.code(), Some(3), "{diagnostics}");
            assert!(
                diagnostics.contains("requested process time limit: 0 s"),
                "{diagnostics}"
            );
            if json {
                let document: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
                assert_eq!(document["outcome"]["completion"], "interrupted");
                assert_eq!(document["outcome"]["coverage"], "partial");
                assert_eq!(document["outcome"]["status"], "incomplete");
                assert!(document["models"].as_array().unwrap().is_empty());
                assert!(
                    String::from_utf8(result.stdout)
                        .unwrap()
                        .contains("deadline")
                );
            } else {
                let text = String::from_utf8(result.stdout).unwrap();
                assert!(text.contains("INCOMPLETE"), "{text}");
                assert!(!text.contains("UNSATISFIABLE"), "{text}");
                assert!(!text.contains("Answer:"), "{text}");
                assert!(
                    text.contains("deadline") || diagnostics.contains("deadline"),
                    "{text}\n{diagnostics}"
                );
            }
        }
    }
}

#[test]
fn absent_process_deadlines_allow_complete_enumeration() {
    let result = run(None, true, "countermodel");
    assert_eq!(result.status.code(), Some(0));
    let document: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(document["outcome"]["completion"], "exhausted");
    assert_eq!(document["models"].as_array().unwrap().len(), 4);
}

#[test]
fn unexpired_process_deadlines_allow_complete_enumeration() {
    let result = run(Some("60"), true, "countermodel");
    assert_eq!(result.status.code(), Some(0));
    let document: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(document["outcome"]["completion"], "exhausted");
    assert_eq!(document["models"].as_array().unwrap().len(), 4);
}

#[test]
fn unrepresentable_process_deadlines_are_refused() {
    let result = run(Some("18446744073709551615"), true, "countermodel");
    assert_eq!(result.status.code(), Some(2));
    let diagnostics = String::from_utf8(result.stderr).unwrap();
    assert!(
        diagnostics.contains("time limit exceeds the platform clock range"),
        "{diagnostics}"
    );
    let document: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(document["models"].as_array().unwrap().is_empty());
}

#[test]
fn process_deadline_syntax_requires_whole_seconds() {
    for value in ["-1", "NaN", "1.5", "18446744073709551616"] {
        let result = run(Some(value), false, "countermodel");
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}
