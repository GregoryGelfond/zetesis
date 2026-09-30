//! Complete support scans retain their own work and submission receipts.

use super::*;

fn tight_fixture() -> (Value, String) {
    let (mut document, text) = formula_fixture();
    let text = text
        .replace(
            "hybrid GPU propagation + exact CPU residual search",
            "GPU tight support",
        )
        .replace("Ferraris reduct countermodel", "Ferraris ranked support")
        .replace("oracle=countermodel", "oracle=tight-support");
    let execution = &mut document["statistics"]["execution"];
    execution["gpu_limits"] = Value::Null;
    execution["gpu_rounds"] = json!(0);
    execution["gpu_decided"] = json!(2);
    execution["cpu_residuals"] = json!(0);
    execution["gpu_submitted_batches"] = json!(1);
    execution["gpu_submitted_candidates"] = json!(2);
    execution["tight_work_per_candidate"] = json!(100);
    execution["gpu_scheduled_work"] = json!(10);
    execution["completion"] = json!({"complete":true,"entered":2,"completed":2,"failed":0,
        "residuals":0,"residual_completed":0,"residual_failed":0});
    (document, text)
}

#[test]
fn tight_device_work_is_not_propagation() {
    let (document, text) = tight_fixture();
    let observed = observe(&document, text.as_bytes(), metal()).unwrap();
    assert_eq!(
        observed.execution.backend,
        Backend::Gpu(Some(GpuApi::Metal))
    );
    assert_eq!(observed.execution.procedure, Procedure::TightSupport);
    assert_eq!(
        serde_json::to_value(observed.execution.procedure).unwrap(),
        "tight_support"
    );
    assert!(matches!(
        observed.execution.device,
        DeviceWork::TightSupport {
            batches: 1,
            candidates: 2,
            work: 10,
            submitted_batches: 1,
            submitted_candidates: 2,
            scheduled_work: 10,
            work_per_candidate_limit: 100,
            peak_accounted_bytes: 256
        }
    ));
    let retained = serde_json::to_value(&observed.execution.device).unwrap();
    assert_eq!(retained["kind"], "tight_support");
    assert!(retained.get("sweeps").is_none());
    assert!(retained.get("cpu_residuals").is_none());
}

#[test]
fn scheduled_tight_work_is_distinct_from_decoded_work() {
    let (mut document, text) = tight_fixture();
    let execution = &mut document["statistics"]["execution"];
    execution["gpu_submitted_batches"] = json!(2);
    execution["gpu_submitted_candidates"] = json!(4);
    execution["gpu_scheduled_work"] = json!(20);
    let observed = observe(&document, text.as_bytes(), metal()).unwrap();
    assert!(matches!(
        observed.execution.device,
        DeviceWork::TightSupport {
            candidates: 2,
            work: 10,
            submitted_candidates: 4,
            scheduled_work: 20,
            ..
        }
    ));
}

#[test]
fn tight_counters_must_match_the_complete_scan_contract() {
    let (document, text) = tight_fixture();
    for (field, value) in [
        ("gpu_rounds", json!(1)),
        (
            "gpu_limits",
            json!({"work_per_candidate":100,"rounds_per_candidate":1}),
        ),
        ("gpu_scheduled_work", json!(9)),
        ("gpu_scheduled_work", json!(11)),
        ("tight_work_per_candidate", json!(4)),
        ("gpu_submitted_candidates", json!(1)),
        ("gpu_submitted_batches", json!(0)),
        ("gpu_submitted_batches", json!(3)),
        ("pending_candidates", json!(1)),
        ("gpu_work", json!(9)),
        ("gpu_work", json!(-1)),
    ] {
        let mut invalid = document.clone();
        invalid["statistics"]["execution"][field] = value;
        assert!(
            observe(&invalid, text.as_bytes(), metal()).is_err(),
            "{field}"
        );
    }
}

#[test]
fn tight_routes_require_their_new_receipts() {
    let (document, text) = tight_fixture();
    for field in [
        "gpu_rounds",
        "gpu_limits",
        "gpu_submitted_batches",
        "gpu_submitted_candidates",
        "gpu_scheduled_work",
        "tight_work_per_candidate",
    ] {
        let mut missing = document.clone();
        missing["statistics"]["execution"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            observe(&missing, text.as_bytes(), metal()).is_err(),
            "{field}"
        );
    }
}

#[test]
fn tight_routes_cannot_report_residual_completion() {
    let (mut document, text) = tight_fixture();
    let execution = &mut document["statistics"]["execution"];
    execution["gpu_decided"] = json!(1);
    execution["cpu_residuals"] = json!(1);
    for field in ["residuals", "residual_completed"] {
        execution["completion"][field] = json!(1);
    }
    assert!(observe(&document, text.as_bytes(), metal()).is_err());
}

#[test]
fn tight_routes_cannot_report_propagation_residual_reasons() {
    let (document, text) = tight_fixture();
    for reasons in [
        json!({"fixed_point":0,"round_limit":0,"work_limit":0}),
        json!({"fixed_point":1,"round_limit":0,"work_limit":0}),
    ] {
        let mut invalid = document.clone();
        invalid["statistics"]["execution"]["gpu_residuals"] = reasons;
        assert!(observe(&invalid, text.as_bytes(), metal()).is_err());
    }
}

#[test]
fn tight_routes_cannot_hide_residual_attempts_as_uncommitted_work() {
    let (mut document, text) = tight_fixture();
    document["statistics"]["execution"]["completion"] = json!({"complete":true,
        "entered":3,"completed":3,"failed":0,"residuals":1,
        "residual_completed":1,"residual_failed":0});
    assert!(observe(&document, text.as_bytes(), metal()).is_err());
}

#[test]
fn tight_identity_must_match_route_and_request() {
    let (document, text) = tight_fixture();
    for changed in [
        text.replace("oracle=tight-support", "oracle=countermodel"),
        text.replace(
            "backend=GPU tight support",
            "backend=hybrid GPU propagation + exact CPU residual search",
        ),
        text.replace(
            "Backend: GPU tight support",
            "Backend: hybrid GPU propagation + exact CPU residual search",
        ),
        text.replace(", Metal; vendor=", ", Vulkan; vendor="),
    ] {
        assert!(observe(&document, changed.as_bytes(), metal()).is_err());
    }
    let request = NativeExecution {
        oracle: Oracle::Countermodel,
        ..metal()
    };
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

#[test]
fn an_empty_tight_device_record_does_not_invent_work() {
    let (mut document, text) = tight_fixture();
    let execution = &mut document["statistics"]["execution"];
    for field in [
        "gpu_batches",
        "gpu_candidates",
        "gpu_decided",
        "gpu_submitted_batches",
        "gpu_submitted_candidates",
        "gpu_work",
        "gpu_scheduled_work",
        "peak_accounted_bytes",
    ] {
        execution[field] = json!(0);
    }
    for field in ["entered", "completed"] {
        execution["completion"][field] = json!(0);
    }
    let observed = observe(&document, text.as_bytes(), metal()).unwrap();
    assert!(matches!(
        observed.execution.device,
        DeviceWork::TightSupport {
            batches: 0,
            candidates: 0,
            work: 0,
            scheduled_work: 0,
            ..
        }
    ));
    document["statistics"]["execution"]["gpu_work"] = json!(1);
    assert!(observe(&document, text.as_bytes(), metal()).is_err());
}

#[test]
fn legacy_propagation_records_remain_distinct() {
    let (mut document, text) = formula_fixture();
    assert!(matches!(
        observe(&document, text.as_bytes(), metal())
            .unwrap()
            .execution
            .device,
        DeviceWork::Formula {
            work: 10,
            cpu_residuals: 1,
            ..
        }
    ));
    document["statistics"]["execution"]["tight_work_per_candidate"] = json!(100);
    assert!(observe(&document, text.as_bytes(), metal()).is_err());
}
