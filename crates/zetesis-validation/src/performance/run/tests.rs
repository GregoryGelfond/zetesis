//! Private source preparation refuses partial or changed execution inputs.
use super::*;

fn corpus() -> examples::Corpus {
    examples::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/correctness"),
        examples::Limits::default(),
    )
    .unwrap()
}

#[test]
fn every_baseline_case_has_a_distinct_source_identity() {
    let cases = Case::ALL;
    let paths: BTreeSet<_> = cases.iter().map(Case::path).collect();
    assert_eq!(paths.len(), cases.len());
}

#[test]
fn source_changes_between_loading_and_sealing_are_refused() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.lp");
    std::fs::write(&path, b"original.\n").unwrap();
    let original = identity::seal(&path, 1024).unwrap();
    std::fs::write(&path, b"changed.\n").unwrap();
    assert!(matches!(
        checked_seal(&path, 1024, original.sha256()),
        Err(Error::Boundary(crate::selected::Error::Path {
            detail: "input changed after corpus verification",
            ..
        }))
    ));
}

#[test]
fn blocked_include_directories_refuse_private_copying() {
    let corpus = corpus();
    let directory = tempfile::tempdir().unwrap();
    let blocker = directory.path().join("standalone");
    std::fs::write(&blocker, b"existing content").unwrap();
    let error = copy_sources(
        &corpus,
        &BTreeSet::from([Case::Send.path()]),
        directory.path(),
        1_048_576,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        Error::Boundary(crate::selected::Error::Io { .. })
    ));
    assert_eq!(std::fs::read(blocker).unwrap(), b"existing content");
}

#[test]
fn unwritable_source_entries_refuse_private_copying() {
    let corpus = corpus();
    let directory = tempfile::tempdir().unwrap();
    let entry = directory.path().join(Case::Send.path());
    std::fs::create_dir_all(&entry).unwrap();
    assert!(matches!(
        copy_sources(
            &corpus,
            &BTreeSet::from([Case::Send.path()]),
            directory.path(),
            1_048_576,
        ),
        Err(Error::Boundary(crate::selected::Error::Io { .. }))
    ));
    assert!(entry.is_dir());
}

#[test]
fn reference_solver_exit_codes_cannot_qualify_helpers() {
    let corpus = corpus();
    let path = "scenarios/shortest-path/variant-01/04-no-path.lp";
    let case = corpus
        .cases()
        .iter()
        .find(|case| case.path() == path)
        .unwrap();
    let stdout = b"UNSATISFIABLE\nModels: 0\nCoverage: exhausted\n";
    let reference = answers::native_text(stdout, false, answers::Limits::default()).unwrap();
    let mut sample = Sample {
        slot: Slot {
            case: Case::Selected(path.into()),
            phase: Phase::Memory,
            round: 0,
            producer: Producer::Reference,
        },
        capture: Capture {
            executable: "/helper".into(),
            arguments: Vec::new(),
            directory: "/".into(),
            started_unix_ns: Some(1),
            elapsed_ns: Some(1),
            stop: Some(process::Stop::Completed),
            exit: Some(process::Exit {
                code: Some(10),
                signal: None,
            }),
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
            failure: None,
            cleanup_failure: None,
            unresolved_child: None,
            helper_child_id: Some(7),
        },
        decision: Decision::Pass,
        detail: None,
        selected_models: None,
        cost: None,
        diagnostics: None,
        memory: Some(process::memory::Measurement {
            schema: 1,
            child: 8,
            exit_code: Some(20),
            signal: None,
            raw_max_rss: 0,
            raw_unit: process::memory::Unit::current().unwrap(),
            peak_rss_bytes: 0,
        }),
        memory_record: None,
    };
    let result = qualify(
        &mut sample,
        case.contract(),
        Some(&reference),
        answers::Limits::default(),
    );
    assert!(matches!(result, Err((Decision::InvocationFailure, _))));
}
