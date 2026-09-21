//! The installed command protocol uses the same public entry as ordinary users.

use std::io::Write;
use std::process::{Command, Output, Stdio};

fn solve(source: &str, arguments: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .args(["solve", "-", "--device", "cpu", "--threads", "1", "--all"])
        .args(arguments)
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn human_statistics_separate_eager_stages() {
    let result = solve("a.", &["--grounder", "eager", "--stats"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let diagnostics = String::from_utf8(result.stderr).unwrap();
    for label in [
        "Statistics",
        "Time (ms)",
        "Source preparation",
        "Grounding",
        "Solving",
        "host elapsed",
    ] {
        assert!(diagnostics.contains(label), "{label}: {diagnostics}");
    }
    assert!(!diagnostics.contains("Stage timings:"));
    assert!(!diagnostics.contains('\u{1b}'));
    assert!(
        String::from_utf8(result.stdout)
            .unwrap()
            .contains("Answer: 1\na\n")
    );
}

#[test]
fn lazy_statistics_name_interleaved_grounding() {
    let result = solve("{a}.", &["--grounder", "lazy", "--stats"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("interleaved with solving")
    );
}

#[test]
fn requested_json_contains_machine_statistics() {
    let result = solve(
        "{hidden}. visible. #show visible/0.",
        &["--json", "--stats", "--color", "always"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!result.stdout.contains(&0x1b));
    let document: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(document["outcome"]["coverage"], "exhausted");
    assert_eq!(document["models"].as_array().unwrap().len(), 2);
    assert!(!document["statistics"]["phase_timings"].is_null());
    let answers = zetesis_validation::answers::native_json::parse(
        &result.stdout,
        zetesis_validation::answers::native_json::Limits::default(),
    )
    .unwrap();
    assert_ne!(
        answers.records()[0].full_model(),
        answers.records()[1].full_model()
    );
}

#[test]
fn solve_statistics_are_absent_by_default() {
    let result = solve("a.", &["--json"]);
    assert!(result.status.success());
    let document: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(document["statistics"]["phase_timings"].is_null());
    assert!(
        !String::from_utf8(result.stderr)
            .unwrap()
            .contains("Statistics")
    );
}

#[test]
fn solve_does_not_accept_benchmark_options() {
    let result = Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .args(["solve", "-", "--repetitions", "3"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("unexpected argument")
    );
}

#[test]
fn json_argument_errors_leave_stdout_empty() {
    for arguments in [
        vec!["solve", "--json"],
        vec!["test", "backend", "--device", "unknown", "--json"],
        vec!["bench", "corpus", "--json"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_zetesis"))
            .args(&arguments)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2), "{arguments:?}");
        assert!(result.stdout.is_empty(), "{arguments:?}");
        assert!(!result.stderr.is_empty(), "{arguments:?}");
    }
}

#[test]
fn solve_statistics_use_the_requested_terminal_width() {
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), "a.").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .args(["solve", "--device", "cpu", "--threads", "1", "--stats"])
        .arg(input.path())
        .env("COLUMNS", "32")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(result.status.success());
    let diagnostics = String::from_utf8(result.stderr).unwrap();
    assert!(
        diagnostics.contains("Stage:\nSource preparation\n"),
        "{diagnostics}"
    );
    assert!(diagnostics.contains("Time (ms):\n"));
}
