//! Human reports consume actual workflow records, including refused attempts.

use std::io;
use std::num::NonZeroUsize;
use std::sync::atomic::AtomicBool;

use zetesis_cli::testing::{self, Completion, Error};
use zetesis_presentation::{ColorMode, Layout};
use zetesis_test_support::repository;

fn backend(stats: bool) -> testing::TestCommand {
    let mut arguments = vec![
        "zetesis",
        "test",
        "backend",
        "--backend",
        "cpu",
        "--zetesis",
        env!("CARGO_BIN_EXE_zetesis"),
    ];
    if stats {
        arguments.push("--stats");
    }
    super::command(&arguments)
}

fn cancelled_report(stats: bool, layout: Layout) -> String {
    let mut output = Vec::new();
    let result = testing::execute_with_cancellation(
        &backend(stats),
        layout,
        &mut output,
        &mut Vec::new(),
        &AtomicBool::new(true),
    )
    .unwrap();
    assert_eq!(result, Completion::NonPass);
    String::from_utf8(output).unwrap()
}

#[test]
fn backend_report_preserves_recorded_case_identity() {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let result = testing::execute(
        &backend(false),
        Layout::default(),
        &mut output,
        &mut diagnostics,
    )
    .unwrap();
    assert_eq!(result, Completion::Passed);
    assert!(diagnostics.is_empty());
    let text = String::from_utf8(output).unwrap();
    for fixture in [
        "tight-choice-hidden-atoms",
        "seeded-positive-cycle",
        "optimal-ties",
    ] {
        assert!(text.contains(fixture), "{text}");
    }
    assert!(text.contains("Cpu / TightSupport"), "{text}");
    assert!(text.contains("Cpu / Countermodel"), "{text}");
}

#[test]
fn requested_backend_statistics_report_captured_elapsed_values() {
    let mut output = Vec::new();
    assert_eq!(
        testing::execute(
            &backend(true),
            Layout::default(),
            &mut output,
            &mut Vec::new()
        )
        .unwrap(),
        Completion::Passed
    );
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("Selected ms"), "{text}");
    assert!(text.contains("Captured process elapsed time"), "{text}");
    assert!(!text.contains("unavailable"), "{text}");
}

#[test]
fn backend_elapsed_statistics_are_hidden_by_default() {
    let text = cancelled_report(false, Layout::default());
    assert!(!text.contains("Captured process elapsed time"), "{text}");
    assert!(!text.contains("Selected ms"), "{text}");
}

#[test]
fn default_backend_layout_is_plain() {
    let text = cancelled_report(false, Layout::default());
    assert!(!text.contains('\u{1b}'), "{text}");
}

#[test]
fn explicit_backend_color_styles_the_report() {
    let text = cancelled_report(
        false,
        Layout::new(NonZeroUsize::new(32).unwrap(), ColorMode::Always),
    );
    assert!(text.contains('\u{1b}'), "{text}");
}

#[test]
fn backend_report_limits_its_qualification_claim() {
    let text = cancelled_report(false, Layout::default());
    assert!(text.contains("not full physical qualification"), "{text}");
}

#[test]
fn unlaunched_backend_case_is_recorded_without_a_completed_case_claim() {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let layout = Layout::new(NonZeroUsize::new(30).unwrap(), ColorMode::Never);
    let result = testing::execute_with_cancellation(
        &backend(true),
        layout,
        &mut output,
        &mut diagnostics,
        &AtomicBool::new(true),
    )
    .unwrap();
    assert_eq!(result, Completion::NonPass);
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("Cases recorded:\n1\n"), "{text}");
    assert!(text.contains("Cases required:\n3\n"), "{text}");
    assert!(text.contains("All passed:\nfalse\n"), "{text}");
    assert!(text.contains("cancelled"), "{text}");
    assert!(text.contains("Selected ms:\nunavailable\n"), "{text}");
    assert!(!text.contains("Cases completed"));
    assert!(!diagnostics.is_empty());
}

#[test]
fn corpus_report_distinguishes_captured_reference_time_from_unavailable_native_time() {
    let repo = repository::root();
    let directory = tempfile::tempdir().unwrap();
    let report = directory.path().join("report.json");
    // /usr/bin/false is a controlled refusal producer, not a reference solver.
    // The reusable workflow still verifies its complete source roster and records
    // every refused reference; no native execution can be qualified from it.
    let command = super::command(&[
        "zetesis",
        "test",
        "corpus",
        "--repo",
        repo.to_str().unwrap(),
        "--clingo",
        "/usr/bin/false",
        "--stats",
        "--report",
        report.to_str().unwrap(),
    ]);
    let mut output = Vec::new();
    let result =
        testing::execute(&command, Layout::default(), &mut output, &mut Vec::new()).unwrap();
    assert_eq!(result, Completion::NonPass);
    let recorded: serde_json::Value =
        serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert_eq!(recorded["case_count"], 94);
    assert_eq!(recorded["status_counts"]["reference_error"], 94);
    assert!(
        recorded["cases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|case| case["reference_process"]["elapsed_ms"].is_number()
                && case.get("native_process").is_none())
    );
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("Cases recorded"), "{text}");
    assert!(text.contains("94"), "{text}");
    assert!(text.contains("reference_error"), "{text}");
    assert!(text.contains("unavailable"), "{text}");
    assert!(text.contains("clingo ms"), "{text}");
    assert!(text.contains("zetesis ms"), "{text}");
    assert!(!text.contains('\u{1b}'));
}

#[test]
fn human_publication_preserves_a_writer_failure_without_appending_json() {
    struct Failed {
        prefix: Vec<u8>,
    }
    impl io::Write for Failed {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            self.prefix.push(b'!');
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "human fixture sink",
            ))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut output = Failed { prefix: Vec::new() };
    let error = testing::execute_with_cancellation(
        &backend(false),
        Layout::default(),
        &mut output,
        &mut Vec::new(),
        &AtomicBool::new(true),
    )
    .unwrap_err();
    assert!(matches!(error, Error::Io(ref error) if error.kind()==io::ErrorKind::BrokenPipe));
    assert_eq!(output.prefix, b"!");
}
