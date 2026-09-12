//! Typed solve metadata preserves its text and all reporting failure boundaries.

use clap::Parser;
use std::{
    io,
    process::{Command, Stdio},
};
use zetesis_cli::{
    ColorMode, Completion, Options, RunError, run_bundle_finalized_with_diagnostics,
    run_finalized_with_diagnostics,
};
use zetesis_cpu::Control;
use zetesis_themelios::{BundleLimits, SourceBundle};

#[path = "support/bounded_writer.rs"]
mod bounded_writer;
use bounded_writer::BoundedWriter;

fn options(mode: ColorMode) -> Options {
    let mut options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "auto",
        "--grounder",
        "lazy",
        "--workers",
        "1",
        // Keep formatting and writer-boundary checks independent of hardware.
        "--batch-size",
        "16",
        "--models",
        "0",
    ])
    .unwrap();
    options.color = mode;
    options
}

fn bundle() -> SourceBundle {
    SourceBundle::load(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/network-repair.lp"
        ),
        BundleLimits::default(),
    )
    .unwrap()
}

fn diagnostics(mode: ColorMode, json: bool) -> (Vec<u8>, String) {
    let mut options = options(mode);
    options.json = json;
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_bundle_finalized_with_diagnostics(
        bundle(),
        &options,
        &mut output,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap();
    assert_eq!(report.semantic().completion(), Some(Completion::Exhausted));
    (output, String::from_utf8(diagnostics).unwrap())
}

#[test]
fn plain_metadata_describes_the_execution_policy() {
    let automatic = if cfg!(feature = "gpu") {
        "GPU discovery deferred; the first seed stays CPU. Later batches of at least 32 candidates may use a physical GPU with lazy grounding (provisional heuristic)."
    } else {
        "GPU support was not compiled; using CPU without device discovery."
    };
    let expected = format!(
        "Source: 1 original file ({} bytes)\n\
         Oracle: reduct closure\n\
         Grounding: requested=lazy, effective=lazy (source joins; no complete ground-rule store)\n\
         Backend: cpu (lazy source joins, 1 workers)\n\
         Auto: {automatic}\n",
        bundle().total_bytes()
    );
    assert_eq!(diagnostics(ColorMode::Never, false).1, expected);
}

#[test]
fn styled_metadata_retains_its_exact_plain_text() {
    let (_, plain) = diagnostics(ColorMode::Never, false);
    let (_, styled) = diagnostics(ColorMode::Always, false);
    for label in ["Source", "Oracle", "Grounding", "Backend", "Auto"] {
        assert!(styled.contains(&format!("\u{1b}[34m{label}:\u{1b}[3;90m ")));
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
        diagnostics(ColorMode::Auto, false),
        diagnostics(ColorMode::Never, false)
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
            &options(ColorMode::Always),
            &mut output,
            &mut prefix,
            &Control::default(),
        )
        .unwrap_err();
        assert!(matches!(*failure.cause, RunError::Output(ref error)
            if error.kind() == io::ErrorKind::BrokenPipe));
        assert_eq!(prefix.bytes(), &complete.as_bytes()[..limit]);
        assert!(output.is_empty());
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
    let mut settings = options(ColorMode::Always);
    let mut initial = Vec::new();
    run_finalized_with_diagnostics(
        "a.".into(),
        &settings,
        &mut io::sink(),
        &mut initial,
        &Control::default(),
    )
    .unwrap();
    settings.stats = true;
    let mut diagnostics = BoundedWriter::new(initial.len());
    let mut output = Vec::new();
    let failure = run_finalized_with_diagnostics(
        "a.".into(),
        &settings,
        &mut output,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(*failure.cause, RunError::Output(ref error)
        if error.kind() == io::ErrorKind::BrokenPipe));
    assert_eq!(diagnostics.bytes(), initial);
    let semantic = failure.semantic().expect("completed search evidence");
    assert_eq!(semantic.completion(), Some(Completion::Exhausted));
    assert_eq!(semantic.verified_models(), 1);
    assert_eq!(failure.publication().unwrap().models(), 1);
}

#[test]
fn redirected_process_diagnostics_remain_plain() {
    for (no_color, term) in [("", "xterm-256color"), ("1", "xterm"), ("", "dumb")] {
        let result = Command::new(env!("CARGO_BIN_EXE_zetesis"))
            .args(["--backend", "auto", "--grounder", "lazy", "--models", "0"])
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../examples/network-repair.lp"
            ))
            .env("NO_COLOR", no_color)
            .env("TERM", term)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(result.status.success());
        assert!(result.stderr.starts_with(b"Source: "));
        assert!(!result.stdout.contains(&0x1b));
        assert!(!result.stderr.contains(&0x1b));
    }
}
