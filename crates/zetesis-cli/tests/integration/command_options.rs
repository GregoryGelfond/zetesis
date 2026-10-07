//! Command adapters preserve the solver's typed settings and explicit input boundary.

use clap::error::ErrorKind;
use zetesis_cli::{Backend, Grounder, Invocation, Options, StatisticsView};

fn solve(arguments: &[&str]) -> Options {
    let Invocation::Solve(options) = Invocation::try_parse_from(arguments.iter().copied()).unwrap()
    else {
        panic!("expected a solve invocation");
    };
    *options
}

#[test]
fn bare_invocation_displays_task_help() {
    let error = Invocation::try_parse_from(["zetesis"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::DisplayHelp);
    let text = error.to_string();
    for command in ["solve", "test", "devices", "help", "version"] {
        assert!(text.contains(command), "missing {command}");
    }
    assert!(!text.contains("--max-search-work"));
}

#[test]
fn explicit_solve_requires_a_source() {
    let error = Invocation::try_parse_from(["zetesis", "solve"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
}

#[test]
fn explicit_standard_input_is_preserved() {
    assert_eq!(solve(&["zetesis", "solve", "-"]).input.to_str(), Some("-"));
}

#[test]
fn legacy_options_keep_standard_input_default() {
    let options = solve(&["zetesis", "--models", "0"]);
    assert_eq!(options.input.to_str(), Some("-"));
}

#[test]
fn source_roots_preserve_authored_order() {
    let options = solve(&["zetesis", "solve", "second.lp", "first.lp", "second.lp"]);
    assert_eq!(options.input.to_str(), Some("second.lp"));
    assert_eq!(
        options.additional_inputs,
        ["first.lp", "second.lp"].map(std::path::PathBuf::from)
    );
}

#[test]
fn all_answers_maps_to_the_existing_unbounded_request() {
    assert_eq!(solve(&["zetesis", "solve", "a.lp", "--all"]).models, 0);
}

#[test]
fn answer_count_maps_to_the_existing_display_limit() {
    assert_eq!(
        solve(&["zetesis", "solve", "a.lp", "--answers", "3"]).models,
        3
    );
}

#[test]
fn answer_count_requires_a_positive_number() {
    let error =
        Invocation::try_parse_from(["zetesis", "solve", "-", "--answers", "0"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::ValueValidation);
}

#[test]
fn conflicting_answer_requests_are_rejected() {
    for flags in [
        vec!["--answers", "2", "--all"],
        vec!["--models", "0", "--answers", "2"],
        vec!["--models", "2", "--all"],
    ] {
        let error = Invocation::try_parse_from(["zetesis", "solve", "-"].into_iter().chain(flags))
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
    }
}

#[test]
fn the_backend_has_no_device_spelling() {
    let arguments = ["zetesis", "solve", "-", "--device", "cpu"];
    let error = Invocation::try_parse_from(arguments).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::UnknownArgument);
}

#[test]
fn legacy_model_count_remains_accepted() {
    assert_eq!(solve(&["zetesis", "solve", "-", "--models", "0"]).models, 0);
}

#[test]
fn ordinary_aliases_preserve_execution_settings() {
    let options = solve(&[
        "zetesis",
        "solve",
        "-",
        "--backend",
        "cpu",
        "--threads",
        "7",
        "--grounder",
        "lazy",
    ]);
    assert_eq!(options.backend, Backend::Cpu);
    assert_eq!(options.workers.get(), 7);
    assert_eq!(options.grounder, Grounder::Lazy);
}

#[test]
fn automatic_threads_match_the_default() {
    let implicit = solve(&["zetesis", "solve", "-"]);
    let explicit = solve(&["zetesis", "solve", "-", "--threads", "auto"]);
    let host = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    assert_eq!(explicit.workers, implicit.workers);
    assert_eq!(explicit.workers.get(), host);
}

#[test]
fn statistics_remain_opt_in() {
    assert!(!solve(&["zetesis", "solve", "-"]).stats);
    assert!(solve(&["zetesis", "solve", "-", "--stats"]).stats);
}

#[test]
fn human_commands_select_table_statistics() {
    assert_eq!(
        solve(&["zetesis", "solve", "-"]).statistics_view,
        StatisticsView::Human
    );
}

#[test]
fn machine_output_retains_record_statistics() {
    let options = solve(&["zetesis", "solve", "-", "--json", "--stats"]);
    assert!(options.json);
    assert_eq!(options.statistics_view, StatisticsView::Records);
}

#[test]
fn legacy_invocations_retain_record_statistics() {
    assert_eq!(
        solve(&["zetesis", "a.lp", "--stats"]).statistics_view,
        StatisticsView::Records
    );
}

#[test]
fn device_inventory_is_a_separate_task() {
    assert!(matches!(
        Invocation::try_parse_from(["zetesis", "devices"]).unwrap(),
        Invocation::Devices
    ));
}

#[test]
fn version_is_an_informational_result() {
    for request in ["version", "--version", "-V"] {
        let error = Invocation::try_parse_from(["zetesis", request]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::DisplayVersion);
        assert_eq!(
            error.to_string(),
            format!(
                "zetesis {} | Copyright (c) 2026 Gregory Gelfond | MIT License\n",
                env!("CARGO_PKG_VERSION")
            )
        );
    }
}

#[test]
fn short_solve_help_groups_everyday_controls() {
    let error = Invocation::try_parse_from(["zetesis", "help", "solve"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::DisplayHelp);
    let text = error.to_string();
    for expected in [
        "Answers:",
        "Execution:",
        "Limits:",
        "Output:",
        "--answers",
        "--all",
        "--backend",
        "--grounder",
        "--threads",
        "--memory",
    ] {
        assert!(text.contains(expected), "missing {expected}");
    }
    assert!(!text.contains("--models"));
    assert!(!text.contains("--device"));
    assert!(!text.contains("--workers"));
    assert!(!text.contains("--max-search-work"));
}

#[test]
fn old_execution_spellings_remain_solve_aliases() {
    let options = solve(&[
        "zetesis",
        "solve",
        "-",
        "--workers",
        "7",
        "--memory",
        "4GiB",
    ]);
    assert_eq!(options.workers.get(), 7);
    assert_eq!(options.memory, 4 << 30);
}

#[test]
fn advanced_help_preserves_public_resource_contracts() {
    for arguments in [
        vec!["zetesis", "help", "solve", "--advanced"],
        vec!["zetesis", "solve", "--help-all"],
        vec!["zetesis", "--help-all"],
    ] {
        let error = Invocation::try_parse_from(arguments).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::DisplayHelp);
        let text = error.to_string();
        assert!(!text.contains("--max-"));
        assert!(text.contains("--memory"));
        assert!(text.contains("not a resident-memory limit"));
        assert!(text.contains("unproved incumbents"));
    }
}

#[test]
fn unknown_help_topics_are_rejected() {
    let error = Invocation::try_parse_from(["zetesis", "help", "unknown"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidSubcommand);
}
