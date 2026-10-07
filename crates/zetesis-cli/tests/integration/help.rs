//! Both help views expose the shared public resource controls.

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
        "--threads",
        "--memory",
        "--time-limit",
        "--help",
        "--version",
    ] {
        assert!(
            short.contains(visible),
            "missing everyday option: {visible}"
        );
    }
    assert!(!short.contains("--oracle"));
}

#[test]
fn full_help_retains_algorithm_choices() {
    let full = help("--help-all");
    for choice in [
        "--oracle",
        "--search",
        "--source-batching",
        "--formula-joins",
    ] {
        assert!(full.contains(choice), "missing algorithm choice: {choice}");
    }
    assert!(full.contains("interrupted runs may display incumbents"));
}

#[test]
fn resource_help_has_no_stage_controls() {
    for text in [help("--help"), help("--help-all")] {
        for removed in [
            "--max-",
            "--completion-workers",
            "--batch-size",
            "--gpu-formula-",
        ] {
            assert!(!text.contains(removed), "removed control: {removed}");
        }
    }
    assert!(help("--help-all").contains("not a resident-memory limit"));
}

#[test]
fn compatibility_aliases_preserve_public_resources() {
    let options = Options::try_parse_from([
        "zetesis",
        "--oracle",
        "countermodel",
        "--workers",
        "3",
        "--memory-budget",
        "4GiB",
    ])
    .unwrap();
    assert_eq!(options.oracle, Oracle::Countermodel);
    assert_eq!(options.workers.get(), 3);
    assert_eq!(options.memory, 4 << 30);
}

#[test]
fn invalid_thread_counts_remain_parser_errors() {
    for flag in ["--threads", "--workers"] {
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
