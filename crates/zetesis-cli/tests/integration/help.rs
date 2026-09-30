//! Help visibility is presentation; all advanced arguments retain their parser.

use clap::Parser;
use zetesis_cli::{Options, Oracle};

fn help(flag: &str) -> String {
    let error = Options::try_parse_from(["zetesis", flag]).unwrap_err();
    assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
    error.to_string()
}

#[test]
fn default_help_shows_everyday_solving_options() {
    let short = help("-h");
    assert_eq!(short, help("--help"));
    for visible in [
        "FILE",
        "devices",
        "--models",
        "--backend",
        "--grounder",
        "--stats",
        "--json",
        "--color",
        "--help",
        "--help-all",
        "--version",
    ] {
        assert!(
            short.contains(visible),
            "missing everyday option: {visible}"
        );
    }
    for advanced in [
        "--oracle",
        "--workers",
        "--completion-workers",
        "--batch-size",
        "--max-search-work",
        "--max-source-bytes",
        "--max-support-bytes",
        "--max-candidate-bytes",
        "--max-observation-bytes",
    ] {
        assert!(
            !short.contains(advanced),
            "advanced option in compact help: {advanced}"
        );
    }
}

#[test]
fn full_help_exposes_the_resource_contracts() {
    let full = help("--help-all");
    for advanced in [
        "--oracle",
        "--workers",
        "--completion-workers",
        "--batch-size",
        "--max-search-work",
        "--max-search-decisions",
        "--max-source-bytes",
        "--max-support-bytes",
        "--max-candidate-bytes",
        "--max-observation-bytes",
        "--max-objective-work",
        "--max-completion-scratch-bytes",
        "--max-reduct-bytes",
        "--max-substitutions",
        "--max-ground-rules",
        "--max-batch-bytes",
    ] {
        assert!(
            full.contains(advanced),
            "missing advanced option: {advanced}"
        );
    }
    assert!(full.contains("interrupted runs may display incumbents"));
}

#[test]
fn hidden_options_keep_their_original_values() {
    let options = Options::try_parse_from([
        "zetesis",
        "--oracle",
        "countermodel",
        "--workers",
        "3",
        "--completion-workers",
        "2",
        "--batch-size",
        "7",
        "--max-search-work",
        "41",
        "--max-work",
        "0",
    ])
    .unwrap();
    assert_eq!(options.oracle, Oracle::Countermodel);
    assert_eq!(options.workers.get(), 3);
    assert_eq!(options.completion_workers.get(), 2);
    assert_eq!(options.batch_size.get(), 7);
    assert_eq!(options.max_search_work, 41);
    assert_eq!(options.max_work, 0);
}

#[test]
fn invalid_worker_counts_remain_parser_errors() {
    for flag in ["--workers", "--completion-workers", "--batch-size"] {
        let error = Options::try_parse_from(["zetesis", flag, "0"]).unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
    }
}

#[test]
fn device_help_remains_available_without_discovery() {
    let error = Options::try_parse_from(["zetesis", "devices", "--help"]).unwrap_err();
    assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
    assert!(error.to_string().contains("zetesis devices"));
}

#[test]
fn every_byte_ceiling_names_the_quantity_it_bounds() {
    // A byte ceiling is read against a resident-memory figure unless it says
    // what it counts: reserved capacity, canonical or encoded payload, or the
    // original bytes of a file. Each option's help must say which.
    let full = help("--help-all");
    let mut options = Vec::new();
    let mut entries: Vec<(&str, String)> = Vec::new();
    for line in full.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("--") {
            let name = rest
                .split(|c: char| c.is_whitespace() || c == '=')
                .next()
                .unwrap();
            options.push(name);
            entries.push((name, String::new()));
        } else if let Some((_, text)) = entries.last_mut() {
            text.push_str(trimmed);
            text.push(' ');
        }
    }
    let ceilings: Vec<_> = entries
        .iter()
        .filter(|(name, _)| name.ends_with("-bytes"))
        .collect();
    assert_eq!(ceilings.len(), 16, "{options:?}");
    for (name, text) in ceilings {
        assert!(
            ["reserved", "canonical", "encoded", "original"]
                .iter()
                .any(|quantity| text.contains(quantity)),
            "--{name} does not say which bytes it bounds: {text}"
        );
    }
}
