//! Successful formula warnings use the source palette and a separate fallible sink.

use std::{fs, io};

use clap::Parser;
use zetesis_cli::{
    ColorMode, Completion, Grounder, Options, Oracle, RunError, run_bundle_with_diagnostics,
    run_finalized_with_diagnostics, run_with_diagnostics,
};
use zetesis_cpu::Cancellation;
use zetesis_test_support::io::BoundedWriter;
use zetesis_themelios::{BundleLimits, SourceBundle};

const SOURCE: &str = "d(0..2).\np(X) :- d(X), 1/X=1.\n";
const WARNING: &str = "warning[zetesis::zero-divisor]";

fn options() -> Options {
    Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--oracle",
        "countermodel",
        "--workers",
        "1",
        "--models",
        "0",
    ])
    .unwrap()
}

fn solve(source: &str, options: &Options) -> (Vec<u8>, String) {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        source.into(),
        options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    (output, String::from_utf8(diagnostics).unwrap())
}

#[test]
fn successful_warnings_use_the_original_source() {
    for oracle in [Oracle::Auto, Oracle::Countermodel] {
        let mut options = options();
        options.oracle = oracle;
        let (output, diagnostics) = solve(SOURCE, &options);
        assert_eq!(diagnostics.matches(WARNING).count(), 1, "{diagnostics}");
        assert!(diagnostics.contains("<input>:2:"), "{diagnostics}");
        assert!(diagnostics.contains("2 | p(X) :- d(X), 1/X=1."));
        assert!(!String::from_utf8(output).unwrap().contains(WARNING));
    }
}

#[test]
fn successful_warnings_use_the_warning_palette() {
    for oracle in [Oracle::Auto, Oracle::Countermodel] {
        for color in [ColorMode::Never, ColorMode::Always] {
            let mut options = options();
            options.oracle = oracle;
            options.color = color;
            let (_, diagnostics) = solve(SOURCE, &options);
            assert_eq!(diagnostics.matches(WARNING).count(), 1, "{diagnostics}");
            if color == ColorMode::Always {
                assert!(diagnostics.contains(&format!("\u{1b}[1;33m{WARNING}")));
                assert!(!diagnostics.contains("\u{1b}[1;31m"));
            } else {
                assert!(!diagnostics.contains('\u{1b}'));
            }
        }
    }
}

#[test]
fn json_models_remain_separate_from_admission_warnings() {
    let mut options = options();
    options.json = true;
    options.color = ColorMode::Always;
    let (output, diagnostics) = solve(SOURCE, &options);
    let document: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(document["outcome"]["completion"], "exhausted");
    assert!(!document["models"].as_array().unwrap().is_empty());
    assert!(!output.contains(&0x1b));
    assert!(!String::from_utf8(output).unwrap().contains(WARNING));
    assert_eq!(diagnostics.matches(WARNING).count(), 1);
    assert!(!diagnostics.contains('\u{1b}'));
}

#[test]
fn explicit_guards_produce_no_admission_warning() {
    let (_, diagnostics) = solve("d(0..2). p(X) :- d(X), X!=0, 1/X=1.", &options());
    assert!(!diagnostics.contains("warning["), "{diagnostics}");
}

#[test]
fn warning_writer_failure_stops_before_semantic_search() {
    for (grounder, color) in [
        (Grounder::Eager, ColorMode::Never),
        (Grounder::Eager, ColorMode::Always),
        (Grounder::Lazy, ColorMode::Never),
        (Grounder::Lazy, ColorMode::Always),
    ] {
        let mut options = options();
        options.grounder = grounder;
        options.color = color;
        let (_, complete) = solve(SOURCE, &options);
        for capacity in [
            0,
            complete.find(WARNING).unwrap() + WARNING.len() / 2,
            complete.find("2 | p(X)").unwrap() + 4,
            complete.find("guard the denominator").unwrap() + 7,
        ] {
            let mut diagnostics = BoundedWriter::new(capacity);
            let mut output = Vec::new();
            let failure = run_finalized_with_diagnostics(
                SOURCE.into(),
                &options,
                &mut output,
                &mut diagnostics,
                &Cancellation::default(),
            )
            .unwrap_err();
            assert!(matches!(failure.cause.as_ref(), RunError::Output(error)
                if error.kind() == io::ErrorKind::BrokenPipe));
            assert!(failure.semantic().is_none());
            assert!(output.is_empty());
            assert_eq!(diagnostics.bytes(), &complete.as_bytes()[..capacity]);
        }
    }
}

#[test]
fn bundle_warnings_resolve_original_included_source() {
    let directory = tempfile::tempdir().unwrap();
    let entry = directory.path().join("entry.lp");
    let child = directory.path().join("child.lp");
    fs::write(&entry, "#include \"child.lp\".\n").unwrap();
    fs::write(&child, SOURCE).unwrap();
    let bundle = SourceBundle::load(&entry, BundleLimits::default()).unwrap();
    fs::write(&child, "replacement.\n").unwrap();
    let mut diagnostics = Vec::new();
    let report = run_bundle_with_diagnostics(
        bundle,
        &options(),
        &mut Vec::new(),
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    let diagnostics = String::from_utf8(diagnostics).unwrap();
    assert_eq!(diagnostics.matches(WARNING).count(), 1);
    assert!(diagnostics.contains("child.lp:2:"), "{diagnostics}");
    assert!(diagnostics.contains("2 | p(X) :- d(X), 1/X=1."));
    assert!(!diagnostics.contains("replacement"));
}
