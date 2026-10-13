//! The front's decision to hand off, where it finds its sibling, and its
//! account of a hand-off that failed. A hand-off that succeeds replaces the
//! process, so it is checked end to end from an installed front instead.

use super::{exec, opens_device, sibling};
use crate::{Invocation, RunError};
use std::io::ErrorKind;
use std::path::Path;

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

/// The failure a hand-off to `path` reports, with its reason.
fn failure(path: &Path) -> (RunError, ErrorKind) {
    let error = exec(path, &["zetesis".into(), "devices".into()]);
    let RunError::GpuExecutableUnavailable {
        path: reported,
        error: cause,
    } = &error
    else {
        panic!("{error:?}");
    };
    assert_eq!(reported, path);
    let kind = cause.kind();
    (error, kind)
}

#[test]
fn a_missing_sibling_names_its_path_and_the_remedy() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("zetesis");
    let (error, kind) = failure(&path);
    assert_eq!(kind, ErrorKind::NotFound);
    let message = error.to_string();
    assert!(message.contains(&path.display().to_string()), "{message}");
    assert!(message.contains("rerun scripts/install.sh"), "{message}");
}

#[test]
fn a_sibling_that_cannot_be_executed_names_the_remedy() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("zetesis");
    std::fs::write(&path, "not an executable").unwrap();
    let (error, kind) = failure(&path);
    assert_eq!(kind, ErrorKind::PermissionDenied);
    assert!(error.to_string().contains("rerun scripts/install.sh"));
}

#[test]
fn another_failure_names_its_reason_without_the_remedy() {
    // Reinstalling cannot repair an exhausted or failing system.
    let error = RunError::GpuExecutableUnavailable {
        path: "zetesis-libexec/0/zetesis".into(),
        error: ErrorKind::OutOfMemory.into(),
    };
    assert!(!error.to_string().contains("rerun"));
}
