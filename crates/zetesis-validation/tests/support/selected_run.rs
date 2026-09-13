//! Acceptance depends on capture and cleanup evidence, not plausible solver text.

use super::*;

#[path = "selected_sealing.rs"]
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
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../validation/upstream/clingo-5.8.2/curated");
    let corpus = curated::open(&root, curated::Limits::default()).unwrap();
    let (decision, detail) =
        compare::case(&corpus.cases()[0], &complete(0), None, Limits::default());
    assert_eq!(decision, Decision::InvocationFailure);
    assert!(detail.unwrap().contains("unresolved child cleanup"));
}
