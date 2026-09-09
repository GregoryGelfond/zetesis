//! The test adapter preserves command identity under bounded library capture.
#![cfg(any(target_os = "linux", target_os = "macos"))]
#[path = "support/process.rs"]
pub mod subprocess;

use std::time::Duration;
use subprocess::{Command, capture};
use zetesis_validation::process::{Limits, Stop};

#[test]
fn arguments_remain_literal() {
    let argument = " spaced '$value' `value` ; value ";
    let result = Command::new("/usr/bin/printf")
        .args(["%s", argument])
        .bounded_output();
    assert!(result.status.success());
    assert_eq!(result.stdout, argument.as_bytes());
    assert!(result.stderr.is_empty());
}

#[test]
fn environment_assignments_remain_literal() {
    let value = " spaced='$value';`value` ";
    let result = Command::new("/usr/bin/printenv")
        .arg("ZETESIS_CAPTURE_TEST_VALUE")
        .env("ZETESIS_CAPTURE_TEST_VALUE", value)
        .bounded_output();
    assert!(result.status.success());
    assert_eq!(result.stdout, format!("{value}\n").as_bytes());
}

#[test]
fn environment_removals_are_preserved() {
    let result = Command::new("/usr/bin/printenv")
        .arg("PATH")
        .env_remove("PATH")
        .bounded_output();
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
}

#[test]
fn working_directory_is_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().canonicalize().unwrap();
    let result = Command::new("/bin/pwd").current_dir(&path).bounded_output();
    assert!(result.status.success());
    assert_eq!(
        String::from_utf8(result.stdout).unwrap().trim(),
        path.to_str().unwrap()
    );
}

#[test]
fn nonzero_exit_remains_a_completed_capture() {
    let result = Command::new("/bin/sh")
        .args(["-c", "exit 23"])
        .bounded_output();
    assert_eq!(result.status.code(), Some(23));
}

#[test]
fn zero_deadline_retains_a_stopped_capture() {
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    let result = capture(
        &command,
        Limits {
            timeout: Duration::ZERO,
            ..Limits::default()
        },
    );
    assert_eq!(result.stop(), Stop::Deadline);
}

#[test]
fn capture_ceiling_retains_only_the_bounded_prefix() {
    let mut command = Command::new("/usr/bin/printf");
    command.arg("123456");
    let result = capture(
        &command,
        Limits {
            max_output_bytes: 5,
            ..Limits::default()
        },
    );
    assert_eq!(result.stop(), Stop::OutputLimit);
    assert_eq!(result.stdout(), b"12345");
}

#[test]
fn invalid_environment_keys_are_refused_before_spawn() {
    for key in ["", "A=B", "A\0B"] {
        assert!(
            std::panic::catch_unwind(|| {
                Command::new("/usr/bin/printf").env(key, "value");
            })
            .is_err()
        );
    }
}

#[test]
fn assignment_like_program_names_are_refused() {
    assert!(std::panic::catch_unwind(|| Command::new("A=B")).is_err());
}
