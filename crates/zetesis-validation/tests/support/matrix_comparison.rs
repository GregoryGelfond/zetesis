//! Complete reported answers, explicit failures and typed telemetry are separate obligations.
use super::*;
use serde_json::json;
use std::num::NonZeroUsize;

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[path = "matrix_effects.rs"]
mod effects;

#[path = "matrix_publication.rs"]
mod publication;

#[path = "matrix_native_family.rs"]
mod native_family;

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
        helper: None,
    }
}
fn sample() -> Sample {
    let (mut document, stderr) = crate::performance::matrix::fixtures::fixture();
    document["schema"] = json!(2);
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
        memory: None,
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

fn memory_sample(producer: Producer, exit: process::Exit) -> Sample {
    let mut sample = sample();
    sample.slot.phase = Phase::Memory;
    sample.slot.producer = producer;
    sample.capture.as_mut().unwrap().helper_child_id = Some(2);
    if producer == Producer::Reference {
        sample.capture.as_mut().unwrap().stdout =
            br#"{"Result":"UNSATISFIABLE","Models":{"More":"no","Number":0},"Call":[{}]}"#.to_vec();
    }
    sample.memory = Some(process::memory::Measurement {
        schema: 1,
        child: 3,
        exit_code: exit.code,
        signal: exit.signal,
        raw_max_rss: 4096,
        raw_unit: process::memory::Unit::Bytes,
        peak_rss_bytes: 4096,
    });
    sample
}

#[test]
fn native_memory_round_requires_a_successful_solver_exit() {
    let mut sample = memory_sample(Producer::Native { profile: 0 }, exit(1));
    let result = qualify(
        &mut sample,
        Some(&contract()),
        Some(&reference()),
        &request(),
    );
    assert!(
        matches!(&result, Err((Decision::InvocationFailure, _))),
        "expected solver exit 1 to fail despite helper exit 0; got {result:?}"
    );
}

#[test]
fn reference_memory_round_requires_a_successful_solver_exit() {
    let mut sample = memory_sample(Producer::Reference, exit(1));
    let result = qualify(
        &mut sample,
        Some(&contract()),
        Some(&reference()),
        &request(),
    );
    assert!(
        matches!(&result, Err((Decision::InvocationFailure, _))),
        "expected solver exit 1 to fail despite helper exit 0; got {result:?}"
    );
}

#[test]
fn native_memory_round_accepts_solver_success() {
    let mut sample = memory_sample(Producer::Native { profile: 0 }, exit(0));
    assert!(
        qualify(
            &mut sample,
            Some(&contract()),
            Some(&reference()),
            &request()
        )
        .is_ok()
    );
}

#[test]
fn reference_memory_round_accepts_reference_exit_codes() {
    for code in [0, 10, 20, 30] {
        let mut sample = memory_sample(Producer::Reference, exit(code));
        let result = qualify(
            &mut sample,
            Some(&contract()),
            Some(&reference()),
            &request(),
        );
        assert!(result.is_ok(), "reference exit {code}: {result:?}");
    }
}

#[test]
fn memory_round_refuses_a_signalled_solver() {
    for producer in [Producer::Native { profile: 0 }, Producer::Reference] {
        let mut sample = memory_sample(
            producer,
            process::Exit {
                code: None,
                signal: Some(15),
            },
        );
        assert!(matches!(
            qualify(
                &mut sample,
                Some(&contract()),
                Some(&reference()),
                &request()
            ),
            Err((Decision::InvocationFailure, _))
        ));
    }
}

#[test]
fn memory_round_requires_helper_success() {
    for producer in [Producer::Native { profile: 0 }, Producer::Reference] {
        let mut sample = memory_sample(producer, exit(0));
        // A helper is not clingo; even a reference-specific exit is a failure.
        sample.capture.as_mut().unwrap().exit = Some(exit(10));
        assert!(matches!(
            qualify(
                &mut sample,
                Some(&contract()),
                Some(&reference()),
                &request()
            ),
            Err((Decision::InvocationFailure, _))
        ));
    }
}

#[test]
fn memory_round_requires_a_solver_resource_record() {
    let mut sample = memory_sample(Producer::Native { profile: 0 }, exit(0));
    sample.memory = None;
    assert!(matches!(
        qualify(
            &mut sample,
            Some(&contract()),
            Some(&reference()),
            &request()
        ),
        Err((Decision::InvalidMemory, _))
    ));
}

#[test]
fn memory_round_refuses_an_invalid_solver_resource_record() {
    let mut sample = memory_sample(Producer::Native { profile: 0 }, exit(0));
    sample.memory.as_mut().unwrap().schema = 0;
    assert!(matches!(
        qualify(
            &mut sample,
            Some(&contract()),
            Some(&reference()),
            &request()
        ),
        Err((Decision::InvalidMemory, _))
    ));
}

#[test]
fn memory_round_keeps_the_solver_outcome_classification() {
    for (document, code, decision) in [
        (failed("unsupported_combination"), 2, Decision::Refused),
        (interrupted(), 3, Decision::Incomplete),
    ] {
        let mut sample = memory_sample(Producer::Native { profile: 0 }, exit(code));
        sample.capture.as_mut().unwrap().stdout = serde_json::to_vec(&document).unwrap();
        assert_eq!(
            qualify(
                &mut sample,
                Some(&contract()),
                Some(&reference()),
                &request()
            )
            .unwrap_err()
            .0,
            decision
        );
    }
}

#[test]
fn complete_answers_require_the_actual_requested_route() {
    let mut sample = sample();
    let answer = qualify(
        &mut sample,
        Some(&contract()),
        Some(&reference()),
        &request(),
    )
    .unwrap();
    assert!(!answer.display.satisfiable());
    assert!(sample.observation.is_some());
    let mut request = request();
    request.plan.profiles[0].backend = crate::selected::Backend::Metal;
    assert_eq!(
        qualify(&mut sample, Some(&contract()), Some(&reference()), &request)
            .unwrap_err()
            .0,
        Decision::InvalidTelemetry
    );
}
#[test]
fn complete_native_answers_cannot_replace_a_failed_reference() {
    assert_eq!(
        qualify(&mut sample(), Some(&contract()), None, &request())
            .unwrap_err()
            .0,
        Decision::ReferenceUnavailable
    );
}

#[test]
fn derived_answers_require_a_complete_reference() {
    assert_eq!(
        qualify(&mut sample(), None, None, &request())
            .unwrap_err()
            .0,
        Decision::ReferenceUnavailable
    );
    assert!(qualify(&mut sample(), None, Some(&reference()), &request()).is_ok());
}

#[test]
fn derived_answers_must_match_the_complete_reference() {
    let different = answers::clingo_json(
        br#"{"Result":"SATISFIABLE","Models":{"More":"no","Number":1},"Call":[{"Witnesses":[{"Value":[]}]}]}"#,
        answers::Limits::default(),
    ).unwrap();
    assert_eq!(
        qualify(&mut sample(), None, Some(&different), &request())
            .unwrap_err()
            .0,
        Decision::ParityMismatch
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
        qualify(
            &mut sample,
            Some(&contract()),
            Some(&reference()),
            &request()
        )
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
fn constraint_cancellation_remains_an_incomplete_census() {
    let mut document = interrupted();
    document["outcome"]["interruption"] =
        json!({"kind":"constraint","code":"cancelled","detail":"unfinished source check"});
    assert_eq!(
        outcome::check(&document, Some(exit(3))).unwrap_err().0,
        Decision::Incomplete
    );
    document["outcome"]["completion"] = json!("exhausted");
    document["outcome"]["coverage"] = json!("exhausted");
    assert_eq!(
        outcome::check(&document, Some(exit(3))).unwrap_err().0,
        Decision::InvalidReport
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
    json!({"schema":2,"format":"zetesis","models":[],"statistics":null,"outcome":{"status":"failed","completion":null,"coverage":"unavailable","published_models":0,"verified_models":null,"checked":null,"interruption":null,"optimization":null,"error":{"kind":kind,"secondary_output_failure":false}}})
}

#[test]
fn legacy_refusal_envelopes_keep_their_existing_diagnostic() {
    let (decision, detail) =
        outcome::check(&failed("bundle_admission"), Some(exit(2))).unwrap_err();
    assert_eq!(decision, Decision::Refused);
    assert_eq!(
        detail,
        "native reported failure kind=bundle_admission; full envelope retained"
    );
}

#[test]
fn typed_failure_detail_is_preserved_without_changing_classification() {
    for (kind, expected) in [
        ("bundle_admission", Decision::Refused),
        ("answer_reconstruction_limit", Decision::Refused),
        ("answer_reconstruction", Decision::InvocationFailure),
        ("gpu", Decision::InvocationFailure),
    ] {
        let mut value = failed(kind);
        value["outcome"]["error"]["detail"] =
            json!("formula expansion work ceiling exceeded: limit 0");
        let (decision, detail) = outcome::check(&value, Some(exit(2))).unwrap_err();
        assert_eq!(decision, expected);
        assert_eq!(
            detail,
            format!(
                "native reported failure kind={kind}: formula expansion work ceiling exceeded: limit 0; full envelope retained"
            )
        );
    }
}

#[test]
fn supplied_nonstring_failure_detail_invalidates_the_envelope() {
    for detail in [Value::Null, json!(17), json!(false), json!([]), json!({})] {
        let mut value = failed("bundle_admission");
        value["outcome"]["error"]["detail"] = detail;
        assert_eq!(
            outcome::check(&value, Some(exit(2))).unwrap_err().0,
            Decision::InvalidReport
        );
    }
}
fn interrupted() -> Value {
    json!({"schema":2,"format":"zetesis","models":[],"statistics":null,"outcome":{"status":"incomplete","completion":"interrupted","coverage":"partial","published_models":0,"verified_models":0,"checked":1,"interruption":{"kind":"oracle","code":"work_limit","detail":"WorkLimit"},"optimization":null,"error":null}})
}

#[test]
fn incomplete_census_keeps_the_validated_typed_reason() {
    for (kind, code, detail) in [
        ("oracle", "work_limit", "oracle work limit reached"),
        (
            "countermodel",
            "projection_bytes",
            "projection history Bytes requires 129; limit is 128",
        ),
        (
            "model_construction",
            "bytes_limit",
            "model construction requires 257 bytes, allowance is 256",
        ),
    ] {
        let mut value = interrupted();
        value["outcome"]["interruption"] = json!({"kind":kind,"code":code,"detail":detail});
        let (decision, actual) = outcome::check(&value, Some(exit(3))).unwrap_err();
        assert_eq!(decision, Decision::Incomplete);
        assert_eq!(
            actual,
            format!(
                "native reported incomplete kind={kind} code={code}: {detail}; full evidence retained"
            )
        );
    }
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
        json!({"schema":2,"format":"zetesis","outcome":{"status":"incomplete","error":null}});
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
        qualify(
            &mut sample,
            Some(&contract()),
            Some(&reference()),
            &request()
        )
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
        qualify(
            &mut sample,
            Some(&contract()),
            Some(&reference()),
            &request()
        )
        .unwrap_err()
        .0,
        Decision::CaptureLimit
    );
}

#[test]
fn deadline_profiles_pass_their_time_limit_to_the_native_solver() {
    let mut request = request();
    let flags = |request: &Request<'_>, producer| {
        let (_, arguments) = arguments(
            request,
            Path::new("/unused"),
            "case.lp",
            producer,
            super::NativeInvocation::Legacy,
        );
        arguments
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
    };
    let native = flags(&request, Producer::Native { profile: 0 });
    assert!(!native.iter().any(|flag| flag == "--time-limit"));
    request.plan = Plan::new(
        Suite::Baseline,
        vec![crate::selected::NativeExecution {
            time_limit_seconds: std::num::NonZeroU64::new(3600),
            ..Default::default()
        }],
        NonZeroUsize::new(1).unwrap(),
        0,
        1,
    )
    .unwrap();
    let native = flags(&request, Producer::Native { profile: 0 });
    let position = native
        .iter()
        .position(|flag| flag == "--time-limit")
        .unwrap();
    assert_eq!(native[position + 1], "3600");
    assert_eq!(native.last().unwrap(), "/unused/case.lp");
    let reference = flags(&request, Producer::Reference);
    assert!(!reference.iter().any(|flag| flag.contains("time")));
}
