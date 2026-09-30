//! The corpus comparison is consumable without clap or command progress I/O.
use zetesis_test_support::repository;
use zetesis_validation::corpus_comparison::{
    self, Decision, Error, PhysicalStatus, Producer, Request,
};

fn request(directory: &std::path::Path) -> Request {
    Request {
        repo: repository::root(),
        clingo: directory.join("missing-reference"),
        zetesis: directory.join("must-not-run"),
        ..Request::default()
    }
}

#[test]
fn library_failures_are_typed_per_source_results() {
    let directory = tempfile::tempdir().unwrap();
    let report = corpus_comparison::run(&request(directory.path()), |_| {}).unwrap();
    assert_eq!(report.cases().len(), 94);
    for case in report.cases() {
        assert!(matches!(
            case.decision(),
            Decision::InvocationFailed(Producer::Reference, _)
        ));
        assert!(case.reference_answers().is_none());
        assert!(case.native_answers().is_none());
        assert!(!case.answer_parity_passed());
    }
    assert!(!report.passed());
    assert!(!report.answer_parity_passed());
    assert_eq!(report.physical_status(), PhysicalStatus::NotRequested);
}

#[test]
fn progress_observes_every_retained_case_in_order() {
    let directory = tempfile::tempdir().unwrap();
    let mut observed = Vec::new();
    let report = corpus_comparison::run(&request(directory.path()), |case| {
        observed.push((case.path().to_owned(), case.decision().clone()));
    })
    .unwrap();
    let expected: Vec<_> = report
        .cases()
        .iter()
        .map(|case| (case.path().to_owned(), case.decision().clone()))
        .collect();
    assert_eq!(observed, expected);
}

#[test]
fn changing_a_report_view_cannot_change_acceptance() {
    let directory = tempfile::tempdir().unwrap();
    let report = corpus_comparison::run(&request(directory.path()), |_| {}).unwrap();
    let mut view = report.to_json().unwrap();
    view["requested_mode_passed"] = true.into();
    view["cases"][0]["status"] = "pass".into();
    assert!(!report.passed());
    assert!(matches!(
        report.cases()[0].decision(),
        Decision::InvocationFailed(Producer::Reference, _)
    ));
}

#[test]
fn invocation_failure_preserves_the_schema_one_case_fields() {
    let directory = tempfile::tempdir().unwrap();
    let report = corpus_comparison::run(&request(directory.path()), |_| {}).unwrap();
    let view = report.to_json().unwrap();
    assert_eq!(view["schema_version"], 1);
    let case = view["cases"][0].as_object().unwrap();
    let mut fields: Vec<_> = case.keys().map(String::as_str).collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        [
            "detail",
            "example_contract",
            "original_sha256",
            "path",
            "sha256",
            "status"
        ]
    );
}

#[test]
fn invalid_limits_fail_before_progress() {
    let directory = tempfile::tempdir().unwrap();
    let mut request = request(directory.path());
    request.timeout_ms = 0;
    let mut observed = 0;
    let error = corpus_comparison::run(&request, |_| observed += 1).unwrap_err();
    assert!(matches!(error, Error::InvalidLimits));
    assert_eq!(observed, 0);
}

#[test]
fn source_integrity_failure_prevents_case_execution() {
    let directory = tempfile::tempdir().unwrap();
    let mut request = request(directory.path());
    request.repo = directory.path().to_owned();
    let mut observed = 0;
    let error = corpus_comparison::run(&request, |_| observed += 1).unwrap_err();
    assert!(matches!(error, Error::Corpus(_)));
    assert_eq!(observed, 0);
}
