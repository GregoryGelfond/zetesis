//! Process status distinguishes complete comparison, applicability and command errors.

use std::process::{Command, Output};

fn command(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_zetesis-bench"))
        .args([
            "table",
            "--rows",
            "1",
            "--queries",
            "1",
            "--workers",
            "1",
            "--warmups",
            "0",
            "--repetitions",
            "1",
        ])
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn complete_table_comparison_exits_successfully() {
    let result = command(&[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let records: Vec<serde_json::Value> = String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.last().unwrap()["event"], "complete");
    assert_eq!(records.last().unwrap()["passed"], true);
    assert_eq!(
        records
            .iter()
            .filter(|record| record["event"] == "batch")
            .count(),
        6
    );
}

#[test]
fn table_applicability_refusal_has_a_distinct_exit() {
    let result = command(&["--max-table-bytes", "0"]);
    assert_eq!(result.status.code(), Some(1));
    let records: Vec<serde_json::Value> = String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(
        records
            .iter()
            .any(|record| record["event"] == "preparation-refused")
    );
    let refusal = records
        .iter()
        .find(|record| record["event"] == "preparation-refused")
        .unwrap();
    assert_eq!(refusal["failure"]["cause"]["kind"], "limit");
    assert_eq!(refusal["failure"]["cause"]["resource"], "bytes");
    assert_eq!(refusal["failure"]["cause"]["limit"], 0);
    assert_eq!(records.last().unwrap()["passed"], false);
    let preparation = records
        .iter()
        .find(|record| record["event"] == "subject")
        .unwrap();
    assert!(preparation["preparation"].get("table").is_none());
}

#[test]
fn invalid_table_command_publishes_no_samples() {
    let result = command(&["--max-table-work", "100000001"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}
