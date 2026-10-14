use super::{Clingo, RunOptions, civil, clingo, utc};
use crate::{Cli, Command, Error};
use clap::Parser;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

fn options(arguments: &[&str]) -> RunOptions {
    let cli = Cli::try_parse_from(["zetesis-bench", "run"].iter().chain(arguments)).unwrap();
    let Command::Run(options) = cli.command else {
        panic!("a run command");
    };
    *options
}

/// A directory holding one runnable file named `clingo`.
fn search_path() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let clingo = directory.path().join("clingo");
    std::fs::write(&clingo, "#!/bin/sh\n").unwrap();
    // Only Unix records whether a file may be executed.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&clingo, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    (directory, clingo)
}

#[test]
fn utc_names_the_epoch() {
    assert_eq!(utc(0), "19700101T000000Z");
}

#[test]
fn utc_keeps_the_leap_day_of_a_leap_century() {
    assert_eq!(utc(951_868_799), "20000229T235959Z");
    assert_eq!(utc(951_868_800), "20000301T000000Z");
}

#[test]
fn utc_skips_the_leap_day_of_a_common_century() {
    assert_eq!(utc(4_107_542_399), "21000228T235959Z");
    assert_eq!(utc(4_107_542_400), "21000301T000000Z");
}

// An independent day-by-day calendar over two 400-year eras, so every month
// length and every leap rule is exercised against the closed form.
#[test]
fn civil_agrees_with_a_day_by_day_calendar() {
    let (mut year, mut month, mut day) = (1970, 1, 1);
    for days in 0..2 * 146_097 {
        assert_eq!(civil(days), (year, month, day), "day {days}");
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let length = match month {
            2 if leap => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };
        day += 1;
        if day > length {
            (day, month) = (1, month + 1);
        }
        if month > 12 {
            (month, year) = (1, year + 1);
        }
    }
}

#[test]
fn the_default_evidence_names_the_suite_and_its_start() {
    let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_623_812);
    assert_eq!(
        options(&["--suite", "queens"]).report(start).unwrap(),
        Path::new("zetesis-bench-queens-20260928T193012Z.json")
    );
}

#[test]
fn a_named_evidence_file_is_kept() {
    let start = SystemTime::UNIX_EPOCH;
    assert_eq!(
        options(&["--report", "named.json"]).report(start).unwrap(),
        Path::new("named.json")
    );
}

#[test]
fn native_memory_accepts_binary_units() {
    assert_eq!(options(&["--memory", "3GiB"]).memory, Some(3 << 30));
}

#[test]
fn a_clock_before_the_epoch_names_no_evidence() {
    let before = SystemTime::UNIX_EPOCH - Duration::from_secs(1);
    assert!(matches!(options(&[]).report(before), Err(Error::Usage(_))));
}

#[test]
fn clingo_on_the_search_path_takes_part() {
    let (directory, path) = search_path();
    assert_eq!(
        clingo(&options(&[]), Some(directory.path().as_os_str())).unwrap(),
        Clingo::Found(path)
    );
}

#[test]
fn no_clingo_on_the_search_path_measures_zetesis_alone() {
    let directory = tempfile::tempdir().unwrap();
    assert_eq!(
        clingo(&options(&[]), Some(directory.path().as_os_str())).unwrap(),
        Clingo::Absent
    );
}

#[test]
fn without_clingo_declines_one_on_the_search_path() {
    let (directory, _) = search_path();
    assert_eq!(
        clingo(
            &options(&["--without-clingo"]),
            Some(directory.path().as_os_str())
        )
        .unwrap(),
        Clingo::Declined
    );
}

#[test]
fn a_named_clingo_is_used_without_a_search() {
    let (_directory, path) = search_path();
    let named = path.to_str().unwrap();
    assert_eq!(
        clingo(&options(&["--clingo", named]), None).unwrap(),
        Clingo::Found(path)
    );
}

// Only Unix records whether a file may be executed.
#[cfg(unix)]
#[test]
fn a_named_clingo_that_cannot_run_is_refused() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_directory, path) = search_path();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    let named = path.to_str().unwrap();
    assert!(matches!(
        clingo(&options(&["--clingo", named]), None),
        Err(Error::Io(_))
    ));
}

#[test]
fn external_workloads_refuse_other_input_selectors() {
    for selector in [
        vec!["--suite", "corpus"],
        vec!["--suite", "scalability"],
        vec!["--case", "case.lp"],
        vec!["--examples", "examples"],
        vec!["--include-einstein"],
        vec!["--without-clingo"],
    ] {
        let error = Cli::try_parse_from(
            ["zetesis-bench", "run", "--workloads", "private.json"]
                .into_iter()
                .chain(selector),
        )
        .unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }
}

#[test]
fn external_workloads_keep_profiles_without_a_case_selection() {
    let options = options(&[
        "--workloads",
        "private.json",
        "--compare-threads",
        "1,2",
        "--compare-grounders",
    ]);
    assert_eq!(
        options.workloads.as_deref(),
        Some(Path::new("private.json"))
    );
    let plan = options.plan().unwrap();
    assert_eq!(
        plan.suite(),
        zetesis_validation::performance::matrix::Suite::Corpus
    );
    assert_eq!(plan.profiles().len(), 4);
    assert_eq!(plan.selection(), None);
}

#[test]
fn programmatic_external_selection_cannot_bypass_conflicts() {
    for arguments in [
        vec!["--suite", "scalability"],
        vec!["--case", "case.lp"],
        vec!["--examples", "examples"],
        vec!["--include-einstein"],
        vec!["--without-clingo"],
    ] {
        let mut options = options(&arguments);
        options.workloads = Some("private.json".into());
        assert!(options.plan().is_err(), "{arguments:?}");
    }
}

#[test]
fn external_workloads_require_discovered_clingo() {
    let empty = tempfile::tempdir().unwrap();
    let mut options = options(&["--workloads", "private.json"]);
    assert!(matches!(
        clingo(&options, Some(empty.path().as_os_str())),
        Err(Error::Usage(reason)) if reason.contains("complete-family qualification")
    ));
    let (directory, path) = search_path();
    assert_eq!(
        clingo(&options, Some(directory.path().as_os_str())).unwrap(),
        Clingo::Found(path)
    );
    options.without_clingo = true;
    assert!(matches!(
        clingo(&options, Some(directory.path().as_os_str())),
        Err(Error::Usage(_))
    ));
}

#[test]
fn native_occurrence_defaults_follow_the_decoder_contract() {
    let options = options(&[]);
    let limits = options.native_answers();
    let defaults = zetesis_validation::answers::native_json::Limits::default();
    assert_eq!(options.native_atoms, defaults.max_atoms);
    assert_eq!(options.native_value_nodes, defaults.max_value_nodes);
    assert_eq!(limits.max_atoms, defaults.max_atoms);
    assert_eq!(limits.max_value_nodes, defaults.max_value_nodes);
}

#[test]
fn native_occurrence_controls_leave_solver_profiles_unchanged() {
    let controls = options(&[
        "--answer-bytes",
        "33554432",
        "--native-atoms",
        "1234567",
        "--native-value-nodes",
        "7654321",
    ]);
    let limits = controls.native_answers();
    assert_eq!(limits.max_atoms, 1_234_567);
    assert_eq!(limits.max_value_nodes, 7_654_321);
    assert_eq!(
        serde_json::to_value(controls.plan().unwrap().profiles()).unwrap(),
        serde_json::to_value(options(&[]).plan().unwrap().profiles()).unwrap()
    );
}

#[test]
fn suite_presets_keep_larger_native_occurrence_allowances() {
    use zetesis_validation::performance::scalability;
    let small = options(&[
        "--suite",
        "scalability",
        "--native-atoms",
        "1",
        "--native-value-nodes",
        "2",
    ])
    .native_answers();
    assert_eq!(small.max_atoms, scalability::NATIVE_ATOMS);
    assert_eq!(small.max_value_nodes, scalability::NATIVE_VALUE_NODES);
    for suite in ["corpus", "baseline", "queens", "series", "scalability"] {
        let limits = options(&[
            "--suite",
            suite,
            "--native-atoms",
            "9000000",
            "--native-value-nodes",
            "17000000",
        ])
        .native_answers();
        assert_eq!(limits.max_atoms, 9_000_000, "{suite}");
        assert_eq!(limits.max_value_nodes, 17_000_000, "{suite}");
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn native_occurrence_controls_are_retained_in_campaign_evidence() {
    let (directory, native) = search_path();
    let root = zetesis_test_support::repository::correctness();
    let report = directory.path().join("limits.json");
    let options = options(&[
        root.to_str().unwrap(),
        "--zetesis",
        native.to_str().unwrap(),
        "--without-clingo",
        "--report",
        report.to_str().unwrap(),
        "--case",
        "standalone/send-money/send-money.lp",
        "--memory-runs",
        "0",
        "--answer-bytes",
        "33554432",
        "--native-atoms",
        "1234567",
        "--native-value-nodes",
        "7654321",
    ]);
    // Cancellation prevents every child, while the ordinary request/publication
    // path still records the exact decoder limits passed to the matrix.
    let completion = super::execute(
        &options,
        zetesis_presentation::Layout::new(
            std::num::NonZeroUsize::new(80).unwrap(),
            zetesis_presentation::ColorMode::Never,
        ),
        &mut Vec::new(),
        &mut Vec::new(),
        &std::sync::atomic::AtomicBool::new(true),
    )
    .unwrap();
    assert!(matches!(completion, crate::Completion::NonPass));
    let evidence: serde_json::Value =
        serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert_eq!(
        evidence["report"]["native_normalization_limits"]["atoms"],
        1_234_567
    );
    assert_eq!(
        evidence["report"]["native_normalization_limits"]["value_nodes"],
        7_654_321
    );
    for field in [
        "max_input_bytes",
        "max_witnesses",
        "max_symbols",
        "max_cost_dimensions",
    ] {
        assert_eq!(
            evidence["report"]["limits"]["answers"][field], 33_554_432,
            "{field}"
        );
    }
    assert_eq!(
        evidence["report"]["native_normalization_limits"]["spelling_bytes"],
        33_554_432
    );
    assert_eq!(
        evidence["report"]["native_normalization_limits"]["input_bytes"],
        options.native_report_bytes
    );
    assert_eq!(evidence["report"]["metadata"].as_array().unwrap().len(), 0);
    assert_eq!(evidence["report"]["samples"].as_array().unwrap().len(), 5);
    assert!(
        evidence["report"]["samples"]
            .as_array()
            .unwrap()
            .iter()
            .all(|sample| sample["capture"].is_null())
    );
}

#[test]
fn answer_bytes_default_follows_the_answer_decoder() {
    assert_eq!(
        options(&[]).answer_bytes,
        zetesis_validation::answers::Limits::default().max_input_bytes
    );
}
