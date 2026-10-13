//! Acceptance depends on capture and cleanup evidence, not plausible solver text.

use super::*;
use zetesis_test_support::repository;

mod sealing;

fn complete(exit: i32) -> InvocationRecord {
    InvocationRecord {
        executable: Path::new("/synthetic/solver").into(),
        arguments: Vec::new(),
        stop: Some(process::Stop::Completed),
        exit: Some(process::Exit {
            code: Some(exit),
            signal: None,
        }),
        elapsed_ns: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
        failure: None,
        cleanup_failure: None,
        unresolved_child: None,
    }
}

#[test]
fn a_deadline_profile_passes_its_time_limit_to_the_native_solver() {
    // The limit is part of the profile's identity and the report records it,
    // so the solver must be asked for it: after the search method, before the
    // output flags, as the matrix campaign asks.
    let input = Path::new("/unused/case.lp");
    let plain: Vec<String> = native_arguments(NativeExecution::default(), input)
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    assert!(!plain.iter().any(|flag| flag == "--time-limit"));
    let deadline: Vec<String> = native_arguments(
        NativeExecution {
            time_limit_seconds: std::num::NonZeroU64::new(3600),
            ..Default::default()
        },
        input,
    )
    .iter()
    .map(|argument| argument.to_string_lossy().into_owned())
    .collect();
    let position = deadline
        .iter()
        .position(|flag| flag == "--time-limit")
        .unwrap();
    assert_eq!(deadline[position + 1], "3600");
    assert_eq!(deadline[position + 2], "--json");
    assert_eq!(deadline.last().unwrap(), "/unused/case.lp");
}

#[test]
fn unreaped_children_preclude_completed_evidence() {
    let mut record = complete(0);
    record.unresolved_child = Some(123);
    assert!(!record.complete(false));
    assert!(!record.complete(true));
}

#[test]
fn cleanup_faults_preclude_completed_evidence() {
    let mut record = complete(0);
    record.cleanup_failure = Some(InvocationFailure {
        kind: InvocationFault::Capture(process::Operation::ReapChild),
        detail: "cleanup did not establish exit".into(),
    });
    assert!(!record.complete(false));
    assert!(record.cleanup_failure().is_some());
}

#[test]
fn clingo_result_codes_are_reference_only() {
    for exit in [10, 20, 30] {
        let record = complete(exit);
        assert!(record.complete(true));
        assert!(!record.complete(false));
    }
}

#[test]
fn start_failures_keep_their_typed_origin() {
    for (error, expected) in [
        (
            process::StartError::UnsupportedPlatform,
            InvocationFault::UnsupportedPlatform,
        ),
        (
            process::StartError::RelativePath,
            InvocationFault::RelativePath,
        ),
        (
            process::StartError::DeadlineOverflow,
            InvocationFault::DeadlineOverflow,
        ),
        (
            process::StartError::Spawn(std::io::Error::other(
                "misleading relative path diagnostic",
            )),
            InvocationFault::Spawn,
        ),
    ] {
        let failure = InvocationFailure::start(&error);
        assert_eq!(failure.kind(), expected);
        assert_eq!(failure.detail(), error.to_string());
    }
}

#[test]
fn absent_native_capture_cannot_pass_a_case() {
    let root = repository::upstream().join("curated");
    let corpus = curated::open(&root, curated::Limits::default()).unwrap();
    let (decision, detail) =
        compare::case(&corpus.cases()[0], &complete(0), None, Limits::default());
    assert_eq!(decision, Decision::InvocationFailure);
    assert!(detail.unwrap().contains("unresolved child cleanup"));
}

/// A completed native run whose statistics name `executable`.
fn naming(executable: &Path) -> InvocationRecord {
    let mut record = complete(0);
    record.stdout = serde_json::to_vec(
        &serde_json::json!({"statistics": {"executable": executable.to_str().unwrap()}}),
    )
    .unwrap();
    record
}

/// The settlement rule for a native executable at `native`.
fn rule(native: &Path) -> identity::Executables {
    identity::Executables::new(&identity::seal(native, 1024).unwrap(), 1024)
}

#[test]
fn a_passing_case_whose_native_run_names_another_executable_seals_it() {
    let directory = tempfile::tempdir().unwrap();
    let (native, sibling) = (
        directory.path().join("native"),
        directory.path().join("sibling"),
    );
    std::fs::write(&native, "native").unwrap();
    std::fs::write(&sibling, "sibling").unwrap();
    let mut executables = rule(&native.canonicalize().unwrap());
    let decided = settled(
        (Decision::Pass, None),
        Some(&naming(&sibling.canonicalize().unwrap())),
        &mut executables,
    );
    assert_eq!(decided, (Decision::Pass, None));
    assert_eq!(
        executables.close().0.unwrap().canonical(),
        sibling.canonicalize().unwrap()
    );
}

#[test]
fn a_passing_case_whose_native_run_names_a_third_executable_is_an_invalid_report() {
    let directory = tempfile::tempdir().unwrap();
    let paths: Vec<std::path::PathBuf> = ["native", "sibling", "third"]
        .iter()
        .map(|name| {
            let path = directory.path().join(name);
            std::fs::write(&path, name).unwrap();
            path.canonicalize().unwrap()
        })
        .collect();
    let mut executables = rule(&paths[0]);
    let first = settled(
        (Decision::Pass, None),
        Some(&naming(&paths[1])),
        &mut executables,
    );
    assert_eq!(first.0, Decision::Pass);
    let third = settled(
        (Decision::Pass, None),
        Some(&naming(&paths[2])),
        &mut executables,
    );
    assert_eq!(third.0, Decision::InvalidReport);
    assert!(third.1.unwrap().contains("an earlier run named"));
}

#[test]
fn a_passing_native_report_without_statistics_is_an_invalid_report() {
    let directory = tempfile::tempdir().unwrap();
    let native = directory.path().join("native");
    std::fs::write(&native, "native").unwrap();
    let mut executables = rule(&native);
    let unreadable = settled((Decision::Pass, None), Some(&complete(0)), &mut executables);
    assert_eq!(unreadable.0, Decision::InvalidReport);
}

#[test]
fn a_failed_case_settles_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let native = directory.path().join("native");
    std::fs::write(&native, "native").unwrap();
    let mut executables = rule(&native);
    // A third executable would be refused, but a case that did not pass is
    // left as it was.
    let failed = settled(
        (Decision::ModelMismatch, Some("differs".to_owned())),
        Some(&naming(Path::new("/elsewhere/zetesis"))),
        &mut executables,
    );
    assert_eq!(
        failed,
        (Decision::ModelMismatch, Some("differs".to_owned()))
    );
}
