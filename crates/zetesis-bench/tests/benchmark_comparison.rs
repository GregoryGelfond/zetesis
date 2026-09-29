//! Retained campaign comparisons use the public controller without launching solvers.

use clap::Parser as _;
use std::{io, num::NonZeroUsize, path::Path};
use zetesis_bench::{self as benchmark, Cli, Command as BenchCommand, Completion, Error};
use zetesis_presentation::{ColorMode, Layout};
use zetesis_validation::performance::series::{ReadError, ViewError};

#[path = "support/benchmark_reports.rs"]
mod reports;
#[path = "support/bounded_writer.rs"]
mod bounded_writer;

fn command(directory: &Path, records: &[serde_json::Value; 2], retain: bool) -> BenchCommand {
    with_options(directory, records, retain, &[])
}

fn with_options(
    directory: &Path,
    records: &[serde_json::Value; 2],
    retain: bool,
    options: &[&str],
) -> BenchCommand {
    let mut arguments = vec!["zetesis-bench".to_owned(), "compare".into()];
    for (label, record) in ["before", "after"].into_iter().zip(records) {
        let path = directory.join(format!("{label}.json"));
        std::fs::write(&path, serde_json::to_vec(record).unwrap()).unwrap();
        arguments.push(format!("{label}={}", path.display()));
    }
    if retain {
        arguments.extend([
            "--output".into(),
            directory
                .join("comparison.json")
                .to_str()
                .unwrap()
                .to_owned(),
        ]);
    }
    arguments.extend(options.iter().map(|&option| option.to_owned()));
    Cli::try_parse_from(arguments).unwrap().command
}

fn layout() -> Layout {
    Layout::new(NonZeroUsize::new(1024).unwrap(), ColorMode::Never)
}

fn execute(command: &BenchCommand, output: &mut impl io::Write) -> Result<Completion, Error> {
    let mut diagnostics = Vec::new();
    let result = benchmark::execute(command, layout(), output, &mut diagnostics);
    assert!(
        diagnostics.is_empty(),
        "saved reports need no campaign diagnostics"
    );
    result
}

#[test]
fn default_comparison_preserves_nonpassing_campaigns() {
    let directory = tempfile::tempdir().unwrap();
    let command = command(directory.path(), &reports::reports(), false);
    assert!(!command.json());
    assert_eq!(command.color(), ColorMode::Auto);
    let mut output = Vec::new();
    assert_eq!(execute(&command, &mut output).unwrap(), Completion::Passed);
    let text = String::from_utf8(output).unwrap();
    let rows: Vec<_> = text
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    let observations: Vec<_> = rows
        .iter()
        .filter(|row| {
            row.starts_with("before generated/choice-2")
                || row.starts_with("after generated/choice-2")
        })
        .map(String::as_str)
        .collect();
    assert_eq!(
        observations,
        [
            "before generated/choice-2 1: cpu/auto (1 threads) 2.000 5.000 3.500 5.000 pass",
            "after generated/choice-2 1: cpu/auto (1 threads) 20.000 8.000 — — pass",
            "before generated/choice-2 2: cpu/auto (4 threads) 3.000 5.000 — 5.000 pass",
            "after generated/choice-2 2: cpu/auto (4 threads) 21.000 8.000 — — pass",
        ]
    );
    assert!(rows.iter().any(|row| row == "before generated/cycle-2 1: cpu/auto (1 threads) — 15.000 — — blocked by timeout: 2, timeout: 1 timed: not_attempted: reason unavailable in retained sample; blocked by sample 10 (timed, timeout): reason unavailable in retained sample ×2; timed: timeout: reason unavailable in retained sample ×1"));
    assert!(rows.contains(&format!(
        "before false true {} {}",
        "11".repeat(32),
        "33".repeat(32)
    )));
    assert!(rows.contains(&format!(
        "after true true {} {}",
        "22".repeat(32),
        "33".repeat(32)
    )));
    assert!(!text.contains('\u{1b}'));
}

#[test]
fn a_clingo_free_report_shows_clingo_as_not_run() {
    let [before, mut after] = reports::reports();
    // As a clingo-free campaign writes it: no reference samples, no clingo
    // seal, and the policy recorded as clingo-free.
    after["report"]["plan"]["reference_policy"] = serde_json::json!("clingo_free");
    after["report"]["before"].as_array_mut().unwrap().remove(1);
    after["report"]["samples"]
        .as_array_mut()
        .unwrap()
        .retain(|sample| sample["slot"]["producer"]["solver"] != "reference");
    let directory = tempfile::tempdir().unwrap();
    let command = command(directory.path(), &[before, after], false);
    let mut output = Vec::new();
    execute(&command, &mut output).unwrap();
    let rows: Vec<_> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    // Clingo's time and memory columns, apart from a missing value's dash.
    assert!(rows.iter().any(|row| row
        == "after generated/choice-2 1: cpu/auto (1 threads) 20.000 not run — not run pass"));
    assert!(
        rows.iter()
            .any(|row| row.starts_with("before generated/choice-2 1:") && row.contains(" 5.000 "))
    );
    // The identity table: the clingo-free campaign sealed no clingo.
    assert!(rows.contains(&format!("after true true {} not run", "22".repeat(32))));
}

#[test]
fn human_comparison_retains_the_typed_report() {
    let directory = tempfile::tempdir().unwrap();
    let command = command(directory.path(), &reports::reports(), true);
    let BenchCommand::Compare(options) = &command else {
        panic!("expected comparison")
    };
    let expected = benchmark::compare(options).unwrap();
    let mut output = Vec::new();
    execute(&command, &mut output).unwrap();
    let retained: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.path().join("comparison.json")).unwrap())
            .unwrap();
    assert_eq!(retained, serde_json::to_value(expected).unwrap());
    assert!(
        String::from_utf8(output)
            .unwrap()
            .starts_with("Corpus benchmark — timed medians\n")
    );
}

#[test]
fn comparison_does_not_overwrite_retained_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let command = command(directory.path(), &reports::reports(), true);
    let path = directory.path().join("comparison.json");
    let evidence = b"an earlier report must remain byte-exact\n";
    std::fs::write(&path, evidence).unwrap();
    let mut output = Vec::new();
    let error = execute(&command, &mut output).unwrap_err();
    assert!(matches!(error, Error::Io(ref error) if error.kind() == io::ErrorKind::AlreadyExists));
    assert!(output.is_empty());
    assert_eq!(std::fs::read(path).unwrap(), evidence);
}

#[test]
fn human_writer_failure_preserves_retained_comparison() {
    let directory = tempfile::tempdir().unwrap();
    let command = command(directory.path(), &reports::reports(), true);
    let BenchCommand::Compare(options) = &command else {
        panic!("expected comparison")
    };
    let expected = benchmark::compare(options).unwrap();
    let mut output = bounded_writer::BoundedWriter::new(64);
    let error = execute(&command, &mut output).unwrap_err();
    assert!(matches!(error, Error::Io(ref error) if error.kind() == io::ErrorKind::BrokenPipe));
    let retained: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.path().join("comparison.json")).unwrap())
            .unwrap();
    assert_eq!(retained, serde_json::to_value(expected).unwrap());
    assert_eq!(output.bytes().len(), 64);
    assert!(output.bytes().starts_with(b"Corpus benchmark"));
    assert!(!String::from_utf8_lossy(output.bytes()).contains("zetesis-benchmark-failure"));
}

#[test]
fn incompatible_workloads_refuse_before_publication() {
    let directory = tempfile::tempdir().unwrap();
    let mut records = reports::reports();
    records[1]["report"]["cases"][1] = serde_json::json!("generated/a-different-cycle.lp");
    let command = command(directory.path(), &records, true);
    let mut output = Vec::new();
    assert!(matches!(
        execute(&command, &mut output),
        Err(Error::Comparison(ReadError::Compare(ViewError::Cells { label }))) if label == "after"
    ));
    assert!(output.is_empty());
    assert!(!directory.path().join("comparison.json").exists());
}

// Each report sealed with clingo stands against it on every profile: a table
// of the cells where both passed, fastest ratio first, headed by the count of
// cells zetesis decided faster.
#[test]
fn the_tables_show_each_report_against_clingo() {
    let directory = tempfile::tempdir().unwrap();
    let command = command(directory.path(), &reports::reports(), false);
    let mut output = Vec::new();
    execute(&command, &mut output).unwrap();
    let rows: Vec<_> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    let heading = rows
        .iter()
        .position(|row| row.starts_with("Against clingo — report before, profile 1: cpu/auto (1 threads), search default; faster on 1 of 1 cells where both passed"))
        .unwrap_or_else(|| panic!("{rows:#?}"));
    assert!(rows[heading + 1].starts_with("Cell zetesis ms clingo ms zetesis/clingo"));
    assert!(rows[heading + 2].starts_with("generated/choice-2 2.000 5.000 0.400 "));
    let heading = rows
        .iter()
        .position(|row| row.starts_with("Against clingo — report after, profile 2: cpu/auto (4 threads), search default; faster on 0 of 2 cells where both passed"))
        .unwrap_or_else(|| panic!("{rows:#?}"));
    assert!(rows[heading + 2].starts_with("generated/cycle-2 31.000 18.000 1.722 "));
    assert!(rows[heading + 3].starts_with("generated/choice-2 21.000 8.000 2.625 "));
}

#[test]
fn markdown_is_the_series_view_of_the_comparison() {
    let directory = tempfile::tempdir().unwrap();
    let command = with_options(
        directory.path(),
        &reports::reports(),
        false,
        &["--markdown"],
    );
    let BenchCommand::Compare(options) = &command else {
        panic!("expected comparison")
    };
    let expected = benchmark::compare(options).unwrap().markdown();
    let mut output = Vec::new();
    execute(&command, &mut output).unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), expected);
}
