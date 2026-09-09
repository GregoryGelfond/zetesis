//! Complete reported answers, explicit failures and typed telemetry are separate obligations.
use super::*;
use serde_json::json;
use std::num::NonZeroUsize;

fn request() -> Request<'static> {
    Request {
        corpus: Path::new("/unused/corpus"),
        native: Path::new("/unused/native"),
        reference: Path::new("/unused/reference"),
        report: Path::new("/unused/report"),
        plan: Plan::new(
            Suite::Baseline,
            vec![crate::selected::NativeExecution::default()],
            NonZeroUsize::new(1).unwrap(),
            0,
            1,
        )
        .unwrap(),
        limits: crate::performance::Limits::default(),
        native_answers: crate::answers::native_json::Limits::default(),
        max_spelling_bytes: 1024,
    }
}
fn sample() -> Sample {
    let (mut document, stderr) = crate::performance::matrix::fixtures::fixture();
    document["schema"] = json!(1);
    document["format"] = json!("zetesis");
    document["models"] = json!([]);
    document["outcome"] = json!({"status":"unsatisfiable","coverage":"exhausted","completion":"exhausted","published_models":0,
        "verified_models":0,"checked":0,"error":null,"interruption":null,"optimization":null});
    Sample {
        slot: Slot {
            case: 0,
            phase: Phase::Timed,
            round: 0,
            producer: Producer::Native { profile: 0 },
        },
        capture: Some(Capture {
            executable: "/unused/native".into(),
            arguments: vec![],
            directory: "/unused".into(),
            started_unix_ns: Some(1),
            elapsed_ns: Some(1),
            stop: Some(process::Stop::Completed),
            exit: Some(process::Exit {
                code: Some(0),
                signal: None,
            }),
            stdout: serde_json::to_vec(&document).unwrap(),
            stderr: stderr.into_bytes(),
            failure: None,
            cleanup_failure: None,
            unresolved_child: None,
            helper_child_id: None,
        }),
        decision: Decision::Pass,
        detail: None,
        blocked_by: None,
        selected_models: None,
        cost: None,
        observation: None,
    }
}
fn contract() -> examples::Contract {
    serde_json::from_value(json!({"satisfiability":"unsat","family":"all","model_count":0,"cost":null,"witnesses":[],"required_symbols":[],"notes":[]})).unwrap()
}
fn reference() -> answers::ReportedAnswers {
    answers::clingo_json(
        br#"{"Result":"UNSATISFIABLE","Models":{"More":"no","Number":0},"Call":[{}]}"#,
        answers::Limits::default(),
    )
    .unwrap()
}
#[test]
fn complete_answers_require_the_actual_requested_route() {
    let mut sample = sample();
    let answer = qualify(&mut sample, &contract(), Some(&reference()), &request()).unwrap();
    assert!(!answer.satisfiable());
    assert!(sample.observation.is_some());
    let mut request = request();
    request.plan.profiles[0].backend = crate::selected::Backend::Metal;
    assert_eq!(
        qualify(&mut sample, &contract(), Some(&reference()), &request)
            .unwrap_err()
            .0,
        Decision::InvalidTelemetry
    );
}
#[test]
fn complete_native_answers_cannot_replace_a_failed_reference() {
    assert_eq!(
        qualify(&mut sample(), &contract(), None, &request())
            .unwrap_err()
            .0,
        Decision::ReferenceUnavailable
    );
}
#[test]
fn complete_capture_with_failed_exit_cannot_pass() {
    let mut sample = sample();
    sample.capture.as_mut().unwrap().exit = Some(process::Exit {
        code: Some(2),
        signal: None,
    });
    assert_eq!(
        qualify(&mut sample, &contract(), Some(&reference()), &request())
            .unwrap_err()
            .0,
        Decision::InvocationFailure
    );
}
#[test]
fn interruption_is_not_an_unsatisfiability_conclusion() {
    assert_eq!(
        outcome::check(&interrupted(), Some(exit(3))).unwrap_err().0,
        Decision::Incomplete
    );
}
#[test]
fn generic_gpu_failure_does_not_claim_adapter_absence() {
    assert_eq!(
        outcome::check(&failed("gpu"), Some(exit(2))).unwrap_err().0,
        Decision::InvocationFailure
    );
}
#[test]
fn explicit_backend_unavailability_keeps_its_own_disposition() {
    assert_eq!(
        outcome::check(&failed("backend_unavailable"), Some(exit(2)))
            .unwrap_err()
            .0,
        Decision::BackendUnavailable
    );
}

fn exit(code: i32) -> process::Exit {
    process::Exit {
        code: Some(code),
        signal: None,
    }
}
fn failed(kind: &str) -> Value {
    json!({"schema":1,"format":"zetesis","models":[],"statistics":null,"outcome":{"status":"failed","completion":null,"coverage":"unavailable","published_models":0,"verified_models":null,"checked":null,"interruption":null,"optimization":null,"error":{"kind":kind,"secondary_output_failure":false}}})
}
fn interrupted() -> Value {
    json!({"schema":1,"format":"zetesis","models":[],"statistics":null,"outcome":{"status":"incomplete","completion":"interrupted","coverage":"partial","published_models":0,"verified_models":0,"checked":1,"interruption":{"kind":"oracle","code":"work_limit","detail":"WorkLimit"},"optimization":null,"error":null}})
}

#[test]
fn successful_status_cannot_classify_an_embedded_refusal() {
    let mut value: Value =
        serde_json::from_slice(sample().capture.as_ref().unwrap().stdout()).unwrap();
    value["outcome"]["error"] =
        json!({"kind":"unsupported_combination","secondary_output_failure":false});
    assert_eq!(
        outcome::check(&value, Some(exit(0))).unwrap_err().0,
        Decision::InvalidReport
    );
}

#[test]
fn minimal_incomplete_status_is_not_interruption_evidence() {
    let value =
        json!({"schema":1,"format":"zetesis","outcome":{"status":"incomplete","error":null}});
    assert_eq!(
        outcome::check(&value, Some(exit(3))).unwrap_err().0,
        Decision::InvalidReport
    );
}

#[test]
fn typed_refusal_requires_the_failure_exit_code() {
    assert_eq!(
        outcome::check(&failed("unsupported_combination"), Some(exit(0)))
            .unwrap_err()
            .0,
        Decision::InvocationFailure
    );
}

#[test]
fn interrupted_search_requires_the_interruption_exit_code() {
    assert_eq!(
        outcome::check(&interrupted(), Some(exit(0))).unwrap_err().0,
        Decision::InvocationFailure
    );
}

#[test]
fn partial_failures_keep_their_primary_error_kind() {
    let mut value = interrupted();
    value["outcome"]["status"] = json!("failed");
    value["outcome"]["error"] = json!({"kind":"output","secondary_output_failure":true});
    assert_eq!(
        outcome::check(&value, Some(exit(2))).unwrap_err().0,
        Decision::InvocationFailure
    );
}

#[test]
fn coverage_contradictions_do_not_enter_the_refusal_census() {
    let mut value = failed("unsupported_combination");
    value["outcome"]["coverage"] = json!("exhausted");
    assert_eq!(
        outcome::check(&value, Some(exit(2))).unwrap_err().0,
        Decision::InvalidReport
    );
}
#[test]
fn complete_capture_cannot_hide_a_timeout_stop() {
    let mut sample = sample();
    sample.capture.as_mut().unwrap().stop = Some(process::Stop::Deadline);
    assert_eq!(
        qualify(&mut sample, &contract(), Some(&reference()), &request())
            .unwrap_err()
            .0,
        Decision::Timeout
    );
}
#[test]
fn complete_capture_cannot_hide_a_capture_stop() {
    let mut sample = sample();
    sample.capture.as_mut().unwrap().stop = Some(process::Stop::OutputLimit);
    assert_eq!(
        qualify(&mut sample, &contract(), Some(&reference()), &request())
            .unwrap_err()
            .0,
        Decision::CaptureLimit
    );
}
