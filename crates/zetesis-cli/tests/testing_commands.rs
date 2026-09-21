//! Test commands consume reusable workflows and preserve typed failure boundaries.

use std::io;
use zetesis_cli::{
    Invocation,
    testing::{self, Completion, Error, TestCommand},
};
use zetesis_presentation::Layout;

fn command(arguments: &[&str]) -> TestCommand {
    let Invocation::Test(command) = Invocation::try_parse_from(arguments.iter().copied()).unwrap()
    else {
        panic!("expected a test command");
    };
    command
}

#[test]
fn test_statistics_are_opt_in() {
    let TestCommand::Corpus(options) = command(&["zetesis", "test", "corpus"]) else {
        panic!("corpus");
    };
    assert!(!options.view.stats);
    let TestCommand::Backend(options) = command(&["zetesis", "test", "backend"]) else {
        panic!("backend");
    };
    assert!(!options.view.stats);
}

#[test]
fn unsupported_backend_checks_are_explicitly_refused() {
    assert!(
        Invocation::try_parse_from(["zetesis", "test", "backend", "--device", "vulkan"]).is_err()
    );
}

#[test]
fn absent_corpus_has_a_structured_preparation_failure() {
    let directory = tempfile::tempdir().unwrap();
    let command = command(&[
        "zetesis",
        "test",
        "corpus",
        "--repo",
        directory.path().to_str().unwrap(),
        "--json",
    ]);
    let mut output = Vec::new();
    let result = testing::execute(&command, Layout::default(), &mut output, &mut Vec::new());
    assert!(matches!(result, Err(Error::Corpus(_))));
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(report["format"], "zetesis-test-failure");
    assert_eq!(report["status"], "preparation_failed");
    assert_eq!(report["kind"], "corpus_preparation");
}

struct BrokenWriter;
impl io::Write for BrokenWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("fixture output failure"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn failure_publication_preserves_the_primary_error() {
    let command = command(&[
        "zetesis",
        "test",
        "backend",
        "--timeout-seconds",
        "0",
        "--json",
    ]);
    let error = testing::execute(
        &command,
        Layout::default(),
        &mut BrokenWriter,
        &mut Vec::new(),
    )
    .unwrap_err();
    let Error::FailurePublication {
        primary,
        publication,
    } = error
    else {
        panic!("expected both errors");
    };
    assert!(matches!(
        *primary,
        Error::Backend(zetesis_validation::backend_check::Error::InvalidLimits)
    ));
    assert!(matches!(*publication, Error::Json(ref error) if error.is_io()));
}

#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn cpu_backend_checks_the_complete_known_families() {
    let command = command(&[
        "zetesis",
        "test",
        "backend",
        "--device",
        "cpu",
        "--zetesis",
        env!("CARGO_BIN_EXE_zetesis"),
        "--json",
    ]);
    let mut output = Vec::new();
    assert_eq!(
        testing::execute(&command, Layout::default(), &mut output, &mut Vec::new()).unwrap(),
        Completion::Passed
    );
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(report["passed"], true);
    assert_eq!(report["cases"].as_array().unwrap().len(), 3);
    for case in report["cases"].as_array().unwrap() {
        assert_eq!(case["decision"], "passed");
        assert_eq!(case["reference"]["family"], case["selected"]["family"]);
        assert_eq!(
            case["selected"]["observation"]["execution"]["backend"],
            "cpu"
        );
    }
    assert_eq!(
        report["cases"][0]["selected"]["family"]["models"],
        serde_json::json!([["a"], ["b"]])
    );
    assert_eq!(
        report["cases"][1]["selected"]["family"]["models"],
        serde_json::json!([[], ["a", "b", "seed"]])
    );
    assert_eq!(
        report["cases"][2]["selected"]["family"]["costs"],
        serde_json::json!([[2, 1]])
    );
}

#[test]
fn failed_backend_attempts_remain_published_nonpasses() {
    // One captured byte cannot establish this executable's full answer protocol.
    let executable = env!("CARGO_BIN_EXE_zetesis");
    let command = command(&[
        "zetesis",
        "test",
        "backend",
        "--zetesis",
        executable,
        "--json",
        "--capture-bytes",
        "1",
    ]);
    let mut output = Vec::new();
    assert_eq!(
        testing::execute(&command, Layout::default(), &mut output, &mut Vec::new()).unwrap(),
        Completion::NonPass
    );
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(report["passed"], false);
    assert!(
        report["cases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|case| case["selected"].is_null())
    );
}

#[test]
fn report_retention_failure_has_structured_output() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("existing.json");
    std::fs::write(&path, b"original").unwrap();
    let command = command(&[
        "zetesis",
        "test",
        "backend",
        "--json",
        "--report",
        path.to_str().unwrap(),
        "--zetesis",
        env!("CARGO_BIN_EXE_zetesis"),
    ]);
    let mut output = Vec::new();
    let result = testing::execute_with_cancellation(
        &command,
        Layout::default(),
        &mut output,
        &mut Vec::new(),
        &std::sync::atomic::AtomicBool::new(true),
    );
    assert!(
        matches!(result, Err(Error::Io(ref error)) if error.kind() == io::ErrorKind::AlreadyExists)
    );
    let document: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(document["format"], "zetesis-test-failure");
    assert_eq!(document["kind"], "io");
    assert_eq!(std::fs::read(path).unwrap(), b"original");
}

#[test]
fn cancelled_backend_checks_publish_a_nonpass() {
    let command = command(&[
        "zetesis",
        "test",
        "backend",
        "--json",
        "--zetesis",
        env!("CARGO_BIN_EXE_zetesis"),
    ]);
    let mut output = Vec::new();
    let result = testing::execute_with_cancellation(
        &command,
        Layout::default(),
        &mut output,
        &mut Vec::new(),
        &std::sync::atomic::AtomicBool::new(true),
    )
    .unwrap();
    assert_eq!(result, Completion::NonPass);
    let document: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(document["passed"], false);
    assert_eq!(document["cases"].as_array().unwrap().len(), 1);
    assert_eq!(document["cases"][0]["decision"], "cancelled");
    assert!(document["cases"][0]["reference"]["capture"].is_null());
    assert!(document["cases"][0]["selected"].is_null());
}
