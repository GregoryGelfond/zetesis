//! The front's decision to hand off, where it finds its sibling, the command
//! it runs in its place, and its account of a hand-off that failed. Nothing
//! here execs inside the test harness: a hand-off is exercised only where it
//! fails, and the prepared command is spawned at this test binary standing in
//! for the sibling, which exits with [`STAND_IN_STATUS`]. A test that reached
//! exec by mistake would end the harness with that status and fail the run.

use super::{hand_off, hand_off_from, names_one_directory, opens_device, prepared, sibling};
use crate::{Invocation, RunError};
use std::ffi::OsString;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;

fn parsed(arguments: &[&str]) -> Invocation {
    Invocation::try_parse_from(arguments).unwrap()
}

#[test]
fn a_gpu_solve_opens_a_device() {
    for backend in ["gpu", "metal", "vulkan"] {
        let invocation = parsed(&["zetesis", "solve", "-", "--backend", backend]);
        assert!(opens_device(&invocation), "{backend}");
    }
}

#[test]
fn a_cpu_solve_opens_no_device() {
    assert!(!opens_device(&parsed(&["zetesis", "solve", "-"])));
    assert!(!opens_device(&parsed(&[
        "zetesis",
        "solve",
        "-",
        "--backend",
        "cpu"
    ])));
}

#[test]
fn devices_opens_a_device() {
    assert!(opens_device(&parsed(&["zetesis", "devices"])));
}

#[test]
fn a_gpu_test_command_is_not_handed_off() {
    // Its GPU work runs in child processes of this executable, each of which
    // decides for itself.
    let invocation = parsed(&["zetesis", "test", "backend", "--backend", "gpu"]);
    assert!(!opens_device(&invocation));
}

#[test]
fn the_sibling_is_in_the_identity_directory_beside_the_front() {
    assert_eq!(
        sibling(Path::new("/opt/bin/zetesis"), "0.5.0-0123abcd"),
        Path::new("/opt/bin/zetesis-libexec/0.5.0-0123abcd/zetesis")
    );
}

#[test]
fn an_installer_identity_names_one_directory() {
    assert!(names_one_directory("0.5.0-0123456789abcdef"));
}

#[test]
fn an_identity_that_could_leave_the_sibling_directory_is_refused() {
    for identity in ["", ".", "..", "a/b", "../zetesis", "/", "0.5.0/"] {
        assert!(!names_one_directory(identity), "{identity:?}");
    }
}

/// The exit status of this test binary standing in for the sibling: nonzero
/// and distinctive, so that a stand-in reached by exec inside the harness
/// fails the run instead of passing it.
const STAND_IN_STATUS: i32 = 97;

/// Set only in a stand-in's environment: where it records what it received.
const STAND_IN_RECORD: &str = "ZETESIS_SIBLING_STAND_IN_RECORD";

/// The test that spawns the stand-in, which is also the stand-in's selection.
const SPAWNING_TEST: &str =
    "sibling::tests::the_sibling_receives_the_program_name_arguments_and_input_unchanged";

/// The front's arguments for a hand-off to this test binary: the front's
/// program name, then the libtest arguments that select [`SPAWNING_TEST`].
fn front_arguments() -> Vec<OsString> {
    ["zetesis", SPAWNING_TEST, "--exact", "--test-threads=1"]
        .into_iter()
        .map(OsString::from)
        .collect()
}

#[test]
fn the_prepared_command_runs_the_sibling_with_the_front_arguments() {
    let arguments = front_arguments();
    let command = prepared(Path::new("/opt/bin/zetesis-libexec/0/zetesis"), &arguments);
    assert_eq!(command.get_program(), "/opt/bin/zetesis-libexec/0/zetesis");
    assert!(command.get_args().eq(arguments[1..].iter()));
}

/// Spawned as the stand-in (with [`STAND_IN_RECORD`] set), this test binary
/// runs only this test, which records its program name, arguments and standard
/// input, one field per line, and exits with [`STAND_IN_STATUS`].
#[test]
fn the_sibling_receives_the_program_name_arguments_and_input_unchanged() {
    if let Some(record) = std::env::var_os(STAND_IN_RECORD) {
        let mut input = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut input).unwrap();
        let mut file = std::fs::File::create(record).unwrap();
        for argument in std::env::args_os() {
            writeln!(file, "{}", argument.to_str().unwrap()).unwrap();
        }
        write!(file, "{input}").unwrap();
        file.sync_all().unwrap();
        std::process::exit(STAND_IN_STATUS);
    }
    let directory = tempfile::tempdir().unwrap();
    let record = directory.path().join("record");
    let this_test_binary = std::env::current_exe().unwrap();
    let mut child = prepared(&this_test_binary, &front_arguments())
        .env(STAND_IN_RECORD, &record)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"p.\nq :- p.\n")
        .unwrap();
    let status = child.wait().unwrap();
    assert_eq!(status.code(), Some(STAND_IN_STATUS));
    assert_eq!(
        std::fs::read_to_string(record).unwrap(),
        format!("zetesis\n{SPAWNING_TEST}\n--exact\n--test-threads=1\np.\nq :- p.\n")
    );
}

/// The failure a hand-off of `devices` reports from the front at `front`,
/// with its sibling's path, its reason and its message.
fn failure(front: std::io::Result<PathBuf>) -> (Option<PathBuf>, ErrorKind, String) {
    let arguments = vec![OsString::from("zetesis"), OsString::from("devices")];
    let invocation = Invocation::try_parse_from(&arguments).unwrap();
    let error = hand_off_from(&invocation, &arguments, front, "0.5.0-0").unwrap();
    let message = error.to_string();
    let RunError::GpuExecutableUnavailable { path, error: cause } = error else {
        panic!("{error:?}");
    };
    (path, cause.kind(), message)
}

#[test]
fn a_cpu_invocation_is_not_handed_off() {
    let arguments = vec![
        OsString::from("zetesis"),
        OsString::from("solve"),
        OsString::from("-"),
    ];
    let invocation = Invocation::try_parse_from(&arguments).unwrap();
    // No sibling of this identity exists beside this test binary, so a
    // hand-off would have failed: no result shows that none was attempted.
    assert!(hand_off(&invocation, &arguments, "0.5.0-0").is_none());
}

#[test]
fn a_missing_sibling_names_its_path_and_the_remedy() {
    let directory = tempfile::tempdir().unwrap();
    let front = directory.path().join("zetesis");
    let (path, kind, message) = failure(Ok(front.clone()));
    let expected = sibling(&front, "0.5.0-0");
    assert_eq!(path.as_deref(), Some(expected.as_path()));
    assert_eq!(kind, ErrorKind::NotFound);
    assert!(
        message.contains(&expected.display().to_string()),
        "{message}"
    );
    assert!(message.contains("rerun scripts/install.sh"), "{message}");
}

#[test]
fn a_sibling_that_cannot_be_executed_names_the_remedy() {
    let directory = tempfile::tempdir().unwrap();
    let front = directory.path().join("zetesis");
    let path = sibling(&front, "0.5.0-0");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "not an executable").unwrap();
    let (_, kind, message) = failure(Ok(front));
    assert_eq!(kind, ErrorKind::PermissionDenied);
    assert!(message.contains("rerun scripts/install.sh"), "{message}");
}

#[test]
fn a_front_that_cannot_resolve_its_path_reports_no_sibling_path() {
    let (path, kind, message) = failure(Err(ErrorKind::NotFound.into()));
    assert_eq!(path, None);
    assert_eq!(kind, ErrorKind::NotFound);
    assert!(message.contains("could not be located"), "{message}");
    // Reinstalling does not resolve this executable's own path.
    assert!(!message.contains("rerun"), "{message}");
}

#[test]
fn another_failure_names_its_reason_without_the_remedy() {
    // Reinstalling cannot repair an exhausted or failing system.
    let error = RunError::GpuExecutableUnavailable {
        path: Some("zetesis-libexec/0/zetesis".into()),
        error: ErrorKind::OutOfMemory.into(),
    };
    assert!(!error.to_string().contains("rerun"));
}
