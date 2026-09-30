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
