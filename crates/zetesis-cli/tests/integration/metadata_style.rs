//! Typed solve metadata preserves its text and all reporting failure boundaries.

use clap::Parser;
use std::{
    io,
    process::{Command, Stdio},
};
use zetesis_algebra::Cancellation;
use zetesis_cli::{
    ColorMode, Completion, Options, RunError, StatisticsView,
    run_bundle_finalized_with_diagnostics, run_finalized_with_diagnostics,
};
use zetesis_test_support::repository;
use zetesis_themelios::{BundleLimits, SourceBundle};

use zetesis_test_support::io::{BoundedWriter, FailAt};

fn options(mode: ColorMode) -> Options {
    let mut options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--grounder",
        "lazy",
        "--workers",
        "1",
        // Keep formatting and writer-boundary checks independent of hardware.
        "--models",
        "0",
    ])
    .unwrap();
    options.color = mode;
    options
}

fn record_options(mode: ColorMode) -> Options {
    let mut options = options(mode);
    options.stats = true;
    options.statistics_view = StatisticsView::Records;
    options
}

fn setup_metadata(text: &str) -> &str {
    let backend = text.find("Backend:").expect("selected CPU backend");
    let end = backend + text[backend..].find('\n').unwrap() + 1;
    &text[..end]
}

fn bundle() -> SourceBundle {
    SourceBundle::load(
        repository::examples().join("network-repair.lp"),
        BundleLimits::default(),
    )
    .unwrap()
}

fn diagnostics(mode: ColorMode, json: bool) -> (Vec<u8>, String) {
    let mut options = if json {
        options(mode)
    } else {
        record_options(mode)
    };
    options.json = json;
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_bundle_finalized_with_diagnostics(
        bundle(),
        &options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(report.semantic().completion(), Some(Completion::Exhausted));
    let diagnostics = String::from_utf8(diagnostics).unwrap();
    // The metadata prefix ends before solving and variable statistics output.
    let diagnostics = if json {
        diagnostics
    } else {
        setup_metadata(&diagnostics).to_owned()
    };
    (output, diagnostics)
}

#[test]
fn plain_metadata_describes_the_execution_policy() {
    let expected = format!(
        "Source: 1 original file ({} bytes)\n\
         Comparisons: with_form=0; long_operand=0; wide_slot=0; constructor=0; nonnumeric_constant=0; argument_position=0\n\
         Grounding: requested=lazy, effective=hybrid (eager retained core; streamed constraints: 0 templates, 0 admitted instances; grounded in the core: 1 templates, 1 instances)\n\
         Retained core formula: 12 atoms, 74 nodes, 27 roots; 122 operand occurrences; 0 constraints asked by key; streamed constraints checked separately\n\
         Core membership: checked tight support certificate; original constraints still pending\n\
         Backend: cpu; retained-core oracle: Ferraris reduct membership; core grounding: eager\n",
        bundle().total_bytes()
    );
    assert_eq!(diagnostics(ColorMode::Never, false).1, expected);
}

#[test]
fn styled_metadata_retains_its_exact_plain_text() {
    let (_, plain) = diagnostics(ColorMode::Never, false);
    let (_, styled) = diagnostics(ColorMode::Always, false);
    // Every styled metadata label; formula and membership descriptions are
    // plain lines on every route.
    for label in ["Source", "Comparisons", "Grounding", "Backend"] {
        assert!(
            styled.contains(&format!("\u{1b}[34m{label}:\u{1b}[3;90m ")),
            "{label}: {styled:?}"
        );
    }
    let decoded = styled
        .replace("\u{1b}[34m", "")
        .replace("\u{1b}[3;90m", "")
        .replace("\u{1b}[0m", "");
    assert_eq!(decoded, plain);
    assert!(!decoded.contains('\u{1b}'));
}

#[test]
fn automatic_library_metadata_is_plain() {
    assert_eq!(
        diagnostics(ColorMode::Auto, false).1,
        diagnostics(ColorMode::Never, false).1
    );
}

#[test]
fn json_ignores_metadata_color_requests() {
    let plain = diagnostics(ColorMode::Never, true);
    let styled = diagnostics(ColorMode::Always, true);
    assert_eq!(styled, plain);
    assert!(!styled.0.contains(&0x1b));
    assert!(!styled.1.contains('\u{1b}'));
    let document: serde_json::Value = serde_json::from_slice(&styled.0).unwrap();
    assert_eq!(document["outcome"]["completion"], "exhausted");
}

#[test]
fn metadata_prefix_failures_cannot_publish_a_model() {
    let (_, complete) = diagnostics(ColorMode::Always, false);
    for limit in 0..complete.len() {
        let mut prefix = BoundedWriter::new(limit);
        let mut output = Vec::new();
        let failure = run_bundle_finalized_with_diagnostics(
            bundle(),
            &record_options(ColorMode::Always),
            &mut output,
            &mut prefix,
            &Cancellation::default(),
        )
        .unwrap_err();
        assert!(matches!(*failure.cause, RunError::Output(ref error)
            if error.kind() == io::ErrorKind::BrokenPipe));
        assert_eq!(prefix.bytes(), &complete.as_bytes()[..limit]);
        assert!(!std::str::from_utf8(&output).unwrap().contains("Answer:"));
        assert_eq!(
            failure
                .publication()
                .map_or(0, zetesis_cli::Publication::models),
            0
        );
        assert_eq!(
            failure
                .semantic()
                .map_or(0, zetesis_cli::SemanticOutcome::verified_models),
            0
        );
    }
}

#[test]
fn later_diagnostic_failure_retains_exhaustion() {
    let settings = record_options(ColorMode::Always);
    let mut diagnostics = FailAt::new(b"Statistics:");
    let mut output = Vec::new();
    let failure = run_finalized_with_diagnostics(
        "a.".into(),
        &settings,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(*failure.cause, RunError::Output(ref error)
        if error.kind() == io::ErrorKind::BrokenPipe));
    let semantic = failure.semantic().expect("completed search evidence");
    assert_eq!(semantic.completion(), Some(Completion::Exhausted));
    assert_eq!(semantic.verified_models(), 1);
    assert_eq!(failure.publication().unwrap().models(), 1);
}

#[test]
fn redirected_process_output_remains_plain() {
    for (no_color, term) in [("", "xterm-256color"), ("1", "xterm"), ("", "dumb")] {
        let result = Command::new(env!("CARGO_BIN_EXE_zetesis"))
            .args(["--backend", "cpu", "--grounder", "lazy", "--models", "0"])
            .arg(repository::examples().join("network-repair.lp"))
            .env("NO_COLOR", no_color)
            .env("TERM", term)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(result.status.success());
        assert!(result.stdout.starts_with(b"zetesis "));
        assert!(result.stderr.is_empty());
        assert!(!result.stdout.contains(&0x1b));
        assert!(!result.stderr.contains(&0x1b));
    }
}
