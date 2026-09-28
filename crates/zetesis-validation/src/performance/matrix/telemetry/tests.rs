//! Authored producer views test observation reconciliation independently of solving.
use super::*;
use serde_json::json;

use crate::performance::matrix::fixtures::fixture;

mod tight_tests;
mod residual_tests;
mod hybrid_tests;
mod terminal_tests;

#[test]
fn actual_cpu_route_remains_distinct_from_requested_metal() {
    let (document, text) = fixture();
    let cpu = observe(&document, text.as_bytes(), NativeExecution::default()).unwrap();
    assert_eq!(cpu.execution.backend, Backend::Cpu);
    let request = NativeExecution {
        backend: Backend::Gpu(Some(GpuApi::Metal)),
        ..Default::default()
    };
    assert!(observe(&document, text.as_bytes(), request).is_err());
}
#[test]
fn typed_phase_contradiction_invalidates_observation() {
    let (mut document, text) = fixture();
    document["statistics"]["stage_timings"]["measurements"]["grounding"]["elapsed_ns"] = json!(201);
    assert!(observe(&document, text.as_bytes(), NativeExecution::default()).is_err());
}
#[test]
fn unmeasured_grounding_cannot_be_published_as_zero() {
    let (mut document, text) = fixture();
    document["statistics"]["phase_timings"]["measurements"]["candidate_setup"] =
        json!({"calls":0,"elapsed_ns":0,"complete":true});
    assert!(observe(&document, text.as_bytes(), NativeExecution::default()).is_err());
}
#[test]
fn duplicate_backend_metadata_is_not_a_unique_route() {
    let (document, mut text) = fixture();
    text.push_str("Backend: cpu (second fixture)\n");
    assert!(observe(&document, text.as_bytes(), NativeExecution::default()).is_err());
}
#[test]
fn static_device_counter_absence_is_explicit() {
    let (document, text) = fixture();
    let text = text
        .replace(
            "Backend: cpu (fixture)",
            "Backend: gpu (Synthetic Metal, Metal; vendor=0x0000; static atoms=1, rules=1)",
        )
        .replace(
            "effective execution: backend=cpu; oracle=closure; grounder=eager",
            "effective execution: backend=requested GPU policy; oracle=closure; grounder=eager; see backend diagnostics for actual adapter",
        );
    let request = NativeExecution {
        backend: Backend::Gpu(Some(GpuApi::Metal)),
        ..Default::default()
    };
    let observed = observe(&document, text.as_bytes(), request).unwrap();
    assert!(matches!(
        observed.execution.device,
        DeviceWork::Unavailable { .. }
    ));
}
#[test]
fn lazy_device_identity_must_match_the_reported_route() {
    let value = json!({"backend":"Metal","adapter":"other","submitted_candidates":3,"completed_candidates":3,"stopped_candidates":0,"queued_results":0});
    assert!(lazy(&value, GpuApi::Metal, Some("Synthetic Metal")).is_err());
}
#[test]
fn lazy_unfinished_candidates_prevent_complete_observation() {
    let value = json!({"backend":"Metal","adapter":"Synthetic Metal","submitted_candidates":3,"completed_candidates":2,"stopped_candidates":0,"queued_results":0});
    assert!(lazy(&value, GpuApi::Metal, Some("Synthetic Metal")).is_err());
}
#[test]
fn zero_device_work_is_retained_without_acceleration_claim() {
    let value = json!({"backend":"Metal","adapter":"Synthetic Metal","submitted_candidates":0,"completed_candidates":0,"stopped_candidates":0,"queued_results":0,
        "dispatches":0,"world_instances":0,"uploaded_bytes":0,"downloaded_bytes":0});
    assert!(matches!(
        lazy(&value, GpuApi::Metal, Some("Synthetic Metal")).unwrap(),
        DeviceWork::Lazy { dispatches: 0, .. }
    ));
}
#[test]
fn formula_pending_work_prevents_exhaustion_evidence() {
    let value = json!({"adapter":"Synthetic Metal, Metal; vendor=0x0000","gpu_candidates":2,"gpu_decided":1,"cpu_residuals":1,"pending_candidates":1,"queued_models":0,"completion":{"complete":true,"failed":0}});
    assert!(formula(&value, GpuApi::Metal, Some("Synthetic Metal")).is_err());
}
#[test]
fn formula_candidate_sum_cannot_wrap() {
    let value = json!({"adapter":"Synthetic Metal, Metal; vendor=0x0000","gpu_candidates":0,"gpu_decided":u64::MAX,"cpu_residuals":1});
    assert!(formula(&value, GpuApi::Metal, Some("Synthetic Metal")).is_err());
}

#[test]
fn cpu_metadata_cannot_hide_device_counters() {
    let (mut document, text) = fixture();
    document["statistics"]["lazy_execution"] = json!({"dispatches":1});
    assert!(observe(&document, text.as_bytes(), NativeExecution::default()).is_err());
}

#[test]
fn positive_transfers_require_a_reported_dispatch() {
    let value = json!({"backend":"Metal","adapter":"Synthetic Metal","submitted_candidates":0,"completed_candidates":0,"stopped_candidates":0,"queued_results":0,
        "dispatches":0,"world_instances":0,"uploaded_bytes":1,"downloaded_bytes":0});
    assert!(lazy(&value, GpuApi::Metal, Some("Synthetic Metal")).is_err());
}

fn lazy_fixture() -> (Value, String) {
    let (mut document, text) = fixture();
    let text = text
        .replace(
            "Backend: cpu (fixture)",
            "Backend: gpu (Synthetic Metal, Metal; vendor=0x0000; lazy immutable reduct rounds)",
        )
        .replace(
            "effective execution: backend=cpu; oracle=closure; grounder=eager",
            "effective execution: backend=requested GPU policy; oracle=closure; grounder=lazy; see backend diagnostics for actual adapter",
        )
        .replace(
            "  stage grounding_mode: eager",
            "  stage grounding_mode: lazy_interleaved",
        )
        .replace(
            "  stage grounding: calls=1; elapsed_ns=200; complete=true",
            "  stage grounding: unavailable=interleaved",
        )
        .replace(
            "  stage solving: calls=3; elapsed_ns=500; complete=true",
            "  stage solving: calls=3; elapsed_ns=700; complete=true",
        );
    document["statistics"]["stage_timings"]["grounding_mode"] = json!("lazy_interleaved");
    document["statistics"]["stage_timings"]["measurements"]["grounding"] = Value::Null;
    document["statistics"]["stage_timings"]["measurements"]["solving"]["elapsed_ns"] = json!(700);
    document["statistics"]["lazy_execution"] = json!({"backend":"Metal","adapter":"Synthetic Metal","submitted_candidates":3,"completed_candidates":3,"stopped_candidates":0,"queued_results":0,
        "dispatches":5,"world_instances":15,"uploaded_bytes":64,"downloaded_bytes":32});
    (document, text)
}

#[test]
fn lazy_grounding_retains_its_interleaved_measurement() {
    let (document, text) = lazy_fixture();
    let request = NativeExecution {
        backend: Backend::Gpu(Some(GpuApi::Metal)),
        grounder: Grounder::Lazy,
        ..Default::default()
    };
    let observed = observe(&document, text.as_bytes(), request).unwrap();
    assert!(observed.timing.stages["grounding"].is_none());
    assert!(matches!(
        observed.execution.device,
        DeviceWork::Lazy {
            dispatches: 5,
            completed_candidates: 3,
            ..
        }
    ));
}

/// The lazy fixture as a Vulkan device reports it.
fn vulkan_lazy_fixture() -> (Value, String) {
    let (mut document, text) = lazy_fixture();
    let text = text.replace(
        "Synthetic Metal, Metal; vendor=0x0000",
        "Synthetic Vulkan, Vulkan; vendor=0x1002",
    );
    document["statistics"]["lazy_execution"]["backend"] = json!("Vulkan");
    document["statistics"]["lazy_execution"]["adapter"] = json!("Synthetic Vulkan");
    (document, text)
}

fn lazy_request(backend: Backend) -> NativeExecution {
    NativeExecution {
        backend,
        grounder: Grounder::Lazy,
        ..Default::default()
    }
}

#[test]
fn a_vulkan_route_is_observed_as_vulkan() {
    let (document, text) = vulkan_lazy_fixture();
    let request = lazy_request(Backend::Gpu(Some(GpuApi::Vulkan)));
    let observed = observe(&document, text.as_bytes(), request).unwrap();
    assert_eq!(
        observed.execution.backend,
        Backend::Gpu(Some(GpuApi::Vulkan))
    );
    assert_eq!(
        observed.execution.adapter.as_deref(),
        Some("Synthetic Vulkan")
    );
}

#[test]
fn a_vulkan_route_does_not_satisfy_a_metal_request() {
    let (document, text) = vulkan_lazy_fixture();
    let request = lazy_request(Backend::Gpu(Some(GpuApi::Metal)));
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

#[test]
fn a_gpu_request_accepts_only_the_native_api() {
    let native = GpuApi::native();
    for ((document, text), api) in [
        (lazy_fixture(), GpuApi::Metal),
        (vulkan_lazy_fixture(), GpuApi::Vulkan),
    ] {
        let observed = observe(&document, text.as_bytes(), lazy_request(Backend::Gpu(None)));
        assert_eq!(observed.is_ok(), api == native, "{api}");
        if let Ok(observed) = observed {
            assert_eq!(observed.execution.backend, Backend::Gpu(Some(native)));
        }
    }
}

#[test]
fn lazy_device_activity_must_name_the_route_api() {
    let (mut document, text) = vulkan_lazy_fixture();
    document["statistics"]["lazy_execution"]["backend"] = json!("Metal");
    let request = lazy_request(Backend::Gpu(Some(GpuApi::Vulkan)));
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

#[test]
fn an_untargeted_device_api_is_refused() {
    let (document, text) = lazy_fixture();
    let text = text.replace("Synthetic Metal, Metal;", "Synthetic Metal, Dx12;");
    let request = lazy_request(Backend::Gpu(Some(GpuApi::Metal)));
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

fn formula_fixture() -> (Value, String) {
    let (mut document, text) = fixture();
    let text = text.replace("Backend: cpu (fixture)", "Backend: hybrid GPU propagation + exact CPU residual search (Synthetic Metal, Metal; vendor=0x0000); oracle: Ferraris reduct countermodel; grounder: eager (requested eager)")
        .replace("effective execution: backend=cpu; oracle=closure", "effective execution: backend=hybrid GPU propagation + exact CPU residual search; oracle=countermodel");
    document["statistics"]["execution"] = json!({"adapter":"Synthetic Metal, Metal; vendor=0x0000","gpu_candidates":2,"gpu_decided":1,"cpu_residuals":1,
        "pending_candidates":0,"queued_models":0,"completion":{"complete":true,"entered":2,"completed":2,"failed":0,"residuals":1,"residual_completed":1,"residual_failed":0},"gpu_batches":1,"gpu_work":10,"peak_accounted_bytes":256});
    (document, text)
}

#[test]
fn formula_device_work_retains_exact_cpu_residuals() {
    let (document, text) = formula_fixture();
    let request = NativeExecution {
        backend: Backend::Gpu(Some(GpuApi::Metal)),
        ..Default::default()
    };
    let observed = observe(&document, text.as_bytes(), request).unwrap();
    assert!(matches!(
        observed.execution.device,
        DeviceWork::Formula {
            candidates: 2,
            cpu_residuals: 1,
            ..
        }
    ));
}

#[test]
fn missing_lazy_activity_is_not_static_counter_absence() {
    let (mut document, text) = lazy_fixture();
    document["statistics"]["lazy_execution"] = Value::Null;
    let request = NativeExecution {
        backend: Backend::Gpu(Some(GpuApi::Metal)),
        grounder: Grounder::Lazy,
        ..Default::default()
    };
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

#[test]
fn missing_hybrid_activity_is_not_static_counter_absence() {
    let (mut document, text) = formula_fixture();
    document["statistics"]["execution"] = Value::Null;
    let request = NativeExecution {
        backend: Backend::Gpu(Some(GpuApi::Metal)),
        ..Default::default()
    };
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

#[test]
fn simultaneous_activity_objects_do_not_describe_one_route() {
    let (mut document, text) = formula_fixture();
    document["statistics"]["lazy_execution"] = json!({});
    let request = NativeExecution {
        backend: Backend::Gpu(Some(GpuApi::Metal)),
        ..Default::default()
    };
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

#[test]
fn effective_cpu_cannot_describe_a_primary_metal_route() {
    let (document, text) = fixture();
    let text = text.replace(
        "Backend: cpu (fixture)",
        "Backend: gpu (Synthetic Metal, Metal; vendor=0x0000; static atoms=1, rules=1)",
    );
    let request = NativeExecution {
        backend: Backend::Gpu(Some(GpuApi::Metal)),
        ..Default::default()
    };
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

#[test]
fn eager_effective_metadata_cannot_describe_lazy_grounding() {
    let (document, text) = lazy_fixture();
    let text = text.replace(
        "oracle=closure; grounder=lazy",
        "oracle=closure; grounder=eager",
    );
    let request = NativeExecution {
        backend: Backend::Gpu(Some(GpuApi::Metal)),
        grounder: Grounder::Lazy,
        ..Default::default()
    };
    assert!(observe(&document, text.as_bytes(), request).is_err());
}

#[test]
fn committed_formula_work_requires_completion_attempts() {
    let (mut document, _) = formula_fixture();
    document["statistics"]["execution"]["completion"] = json!({"complete":true,"entered":0,"completed":0,"failed":0,"residuals":0,"residual_completed":0,"residual_failed":0});
    assert!(
        formula(
            &document["statistics"]["execution"],
            GpuApi::Metal,
            Some("Synthetic Metal")
        )
        .is_err()
    );
}

#[test]
fn uncommitted_completion_attempts_need_not_equal_commits() {
    let (mut document, _) = formula_fixture();
    document["statistics"]["execution"]["completion"] = json!({"complete":true,"entered":5,"completed":4,"failed":1,"residuals":3,"residual_completed":2,"residual_failed":1});
    assert!(
        formula(
            &document["statistics"]["execution"],
            GpuApi::Metal,
            Some("Synthetic Metal")
        )
        .is_ok()
    );
}

#[test]
fn completion_attempt_sums_cannot_wrap() {
    let (mut document, _) = formula_fixture();
    document["statistics"]["execution"]["completion"]["entered"] = json!(0);
    document["statistics"]["execution"]["completion"]["completed"] = json!(u64::MAX);
    document["statistics"]["execution"]["completion"]["failed"] = json!(1);
    assert!(
        formula(
            &document["statistics"]["execution"],
            GpuApi::Metal,
            Some("Synthetic Metal")
        )
        .is_err()
    );
}

#[test]
fn residual_attempts_are_subsets_of_entered_attempts() {
    let (mut document, _) = formula_fixture();
    document["statistics"]["execution"]["completion"]["residual_completed"] = json!(3);
    document["statistics"]["execution"]["completion"]["residuals"] = json!(3);
    assert!(
        formula(
            &document["statistics"]["execution"],
            GpuApi::Metal,
            Some("Synthetic Metal")
        )
        .is_err()
    );
}

#[test]
fn committed_certificates_require_nonresidual_completions() {
    let (mut document, _) = formula_fixture();
    document["statistics"]["execution"]["completion"]["residual_completed"] = json!(2);
    document["statistics"]["execution"]["completion"]["residuals"] = json!(2);
    assert!(
        formula(
            &document["statistics"]["execution"],
            GpuApi::Metal,
            Some("Synthetic Metal")
        )
        .is_err()
    );
}

#[test]
fn submitted_formula_candidates_require_device_batches() {
    let (mut document, _) = formula_fixture();
    document["statistics"]["execution"]["gpu_batches"] = json!(0);
    assert!(
        formula(
            &document["statistics"]["execution"],
            GpuApi::Metal,
            Some("Synthetic Metal")
        )
        .is_err()
    );
}

#[test]
fn empty_formula_batches_are_not_reported_as_device_work() {
    let (mut document, _) = formula_fixture();
    for counter in ["gpu_candidates", "gpu_decided", "cpu_residuals"] {
        document["statistics"]["execution"][counter] = json!(0);
    }
    assert!(
        formula(
            &document["statistics"]["execution"],
            GpuApi::Metal,
            Some("Synthetic Metal")
        )
        .is_err()
    );
}

fn cpu_batch_fixture() -> (Value, String) {
    let (mut document, text) = fixture();
    let text = text.replace(
        "effective execution: backend=cpu; oracle=closure",
        "effective execution: backend=cpu batched exact completion; oracle=countermodel",
    );
    let (formula_document, _) = formula_fixture();
    document["statistics"]["execution"] = formula_document["statistics"]["execution"].clone();
    let execution = &mut document["statistics"]["execution"];
    execution["adapter"] = Value::Null;
    for counter in [
        "gpu_batches",
        "gpu_candidates",
        "gpu_work",
        "gpu_rounds",
        "gpu_decided",
        "peak_accounted_bytes",
    ] {
        execution[counter] = json!(0);
    }
    execution["completion"] = json!({"complete":true,"entered":1,"completed":1,"failed":0,"residuals":1,"residual_completed":1,"residual_failed":0});
    (document, text)
}

#[test]
fn cpu_batch_residuals_retain_their_host_route() {
    let (document, text) = cpu_batch_fixture();
    let observation = observe(&document, text.as_bytes(), NativeExecution::default()).unwrap();
    assert!(matches!(observation.execution.device, DeviceWork::Cpu));
}

#[test]
fn cpu_batch_residuals_require_completion_attempts() {
    let (mut document, text) = cpu_batch_fixture();
    document["statistics"]["execution"]["completion"] = json!({"complete":true,"entered":0,"completed":0,"failed":0,"residuals":0,"residual_completed":0,"residual_failed":0});
    assert!(observe(&document, text.as_bytes(), NativeExecution::default()).is_err());
}

fn prepared_fixture() -> (Value, String) {
    let (mut document, text) = fixture();
    let text = text
        .replace("failed_attempts=included", "failed_attempts=included; schema=3")
        .replace(
            "  phase candidate_generation:",
            "  phase certificate_setup: unmeasured\n  phase certified_membership: unmeasured\n  phase candidate_generation:",
        )
        .replace(
            "  phase gpu_host_oracle:",
            "  phase reduct_preparation: unmeasured\n  phase gpu_host_oracle:",
        );
    let phases = &mut document["statistics"]["phase_timings"];
    phases["schema"] = json!(3);
    for name in [
        "certificate_setup",
        "certified_membership",
        "reduct_preparation",
    ] {
        phases["measurements"][name] = Value::Null;
    }
    (document, text)
}

#[test]
fn supported_phase_versions_keep_their_exact_population() {
    let (document, text) = fixture();
    let old = observe(&document, text.as_bytes(), NativeExecution::default()).unwrap();
    assert_eq!((old.timing.phase_schema, old.timing.phases.len()), (1, 11));
    let (mut document, text) = prepared_fixture();
    let current = observe(&document, text.as_bytes(), NativeExecution::default()).unwrap();
    assert_eq!(
        (current.timing.phase_schema, current.timing.phases.len()),
        (3, 14)
    );
    document["statistics"]["phase_timings"]["schema"] = json!(2);
    document["statistics"]["phase_timings"]["measurements"]
        .as_object_mut()
        .unwrap()
        .remove("reduct_preparation");
    let certificate = text
        .replace("schema=3", "schema=2")
        .replace("  phase reduct_preparation: unmeasured\n", "");
    let middle = observe(
        &document,
        certificate.as_bytes(),
        NativeExecution::default(),
    )
    .unwrap();
    assert_eq!(
        (middle.timing.phase_schema, middle.timing.phases.len()),
        (2, 13)
    );
}

#[test]
fn typed_phase_versions_must_match_the_text_profile() {
    let (document, text) = prepared_fixture();
    for schema in [Value::Null, json!(1), json!(2), json!(4)] {
        let mut changed = document.clone();
        changed["statistics"]["phase_timings"]["schema"] = schema;
        assert!(observe(&changed, text.as_bytes(), NativeExecution::default()).is_err());
    }
    let mut missing = document;
    missing["statistics"]["phase_timings"]
        .as_object_mut()
        .unwrap()
        .remove("schema");
    assert!(observe(&missing, text.as_bytes(), NativeExecution::default()).is_err());
}

#[test]
fn unknown_typed_phases_are_not_silently_discarded() {
    let (mut document, text) = prepared_fixture();
    document["statistics"]["phase_timings"]["measurements"]["future_preparation"] = Value::Null;
    assert!(observe(&document, text.as_bytes(), NativeExecution::default()).is_err());
}

#[test]
fn positive_consequences_remains_a_distinct_procedure() {
    let (document, text) = prepared_fixture();
    let text = text.replace("oracle=closure", "oracle=positive-consequences");
    let observed = observe(&document, text.as_bytes(), NativeExecution::default()).unwrap();
    assert_eq!(
        observed.execution.procedure,
        Procedure::PositiveConsequences
    );
    assert!(matches!(observed.execution.device, DeviceWork::Cpu));
    assert_eq!(
        serde_json::to_value(observed.execution.procedure).unwrap(),
        "positive_consequences"
    );
}

#[test]
fn positive_consequences_cannot_satisfy_an_explicit_oracle_request() {
    let (document, text) = prepared_fixture();
    let text = text.replace("oracle=closure", "oracle=positive-consequences");
    for oracle in [Oracle::Closure, Oracle::Countermodel] {
        let request = NativeExecution {
            oracle,
            ..Default::default()
        };
        assert_eq!(
            observe(&document, text.as_bytes(), request).unwrap_err(),
            "actual oracle differs from explicit requested procedure"
        );
    }
}

#[test]
fn unknown_procedures_are_not_automatic_specializations() {
    let (document, text) = prepared_fixture();
    let text = text.replace("oracle=closure", "oracle=future-certificate");
    assert_eq!(
        observe(&document, text.as_bytes(), NativeExecution::default()).unwrap_err(),
        "unsupported actual oracle metadata"
    );
}

#[test]
fn positive_consequences_cannot_describe_a_metal_route() {
    let (document, text) = formula_fixture();
    let text = text.replace("oracle=countermodel", "oracle=positive-consequences");
    let request = NativeExecution {
        backend: Backend::Gpu(Some(GpuApi::Metal)),
        ..Default::default()
    };
    assert_eq!(
        observe(&document, text.as_bytes(), request).unwrap_err(),
        "positive consequences require the eager CPU formula route"
    );
}

#[test]
fn positive_consequences_cannot_describe_lazy_grounding() {
    let (mut document, text) = prepared_fixture();
    let text = text
        .replace(
            "oracle=closure; grounder=eager",
            "oracle=positive-consequences; grounder=lazy",
        )
        .replace("grounding_mode: eager", "grounding_mode: lazy_interleaved")
        .replace(
            "stage grounding: calls=1; elapsed_ns=200; complete=true",
            "stage grounding: unavailable=interleaved",
        )
        .replace(
            "stage solving: calls=3; elapsed_ns=500; complete=true",
            "stage solving: calls=3; elapsed_ns=700; complete=true",
        );
    let stages = &mut document["statistics"]["stage_timings"];
    stages["grounding_mode"] = json!("lazy_interleaved");
    stages["measurements"]["grounding"] = Value::Null;
    stages["measurements"]["solving"]["elapsed_ns"] = json!(700);
    let request = NativeExecution {
        grounder: Grounder::Lazy,
        ..Default::default()
    };
    assert_eq!(
        observe(&document, text.as_bytes(), request).unwrap_err(),
        "positive consequences require the eager CPU formula route"
    );
}

#[test]
fn automatic_grounding_records_the_mode_actually_taken() {
    let automatic = NativeExecution {
        grounder: Grounder::Auto,
        ..Default::default()
    };
    let (document, text) = fixture();
    let eager = observe(&document, text.as_bytes(), automatic).unwrap();
    assert_eq!(eager.timing.grounding_mode, "eager");
    let (document, text) = lazy_fixture();
    let request = NativeExecution {
        backend: Backend::Gpu(Some(GpuApi::Metal)),
        ..automatic
    };
    let lazy = observe(&document, text.as_bytes(), request).unwrap();
    assert_eq!(lazy.timing.grounding_mode, "lazy_interleaved");
    assert!(matches!(lazy.execution.device, DeviceWork::Lazy { .. }));
}

#[test]
fn automatic_grounding_still_requires_typed_and_text_agreement() {
    let automatic = NativeExecution {
        grounder: Grounder::Auto,
        ..Default::default()
    };
    let (mut document, text) = fixture();
    document["statistics"]["stage_timings"]["grounding_mode"] = json!("lazy_interleaved");
    assert!(observe(&document, text.as_bytes(), automatic).is_err());
    let (document, text) = fixture();
    let text = text.replace(
        "effective execution: backend=cpu; oracle=closure; grounder=eager",
        "effective execution: backend=cpu; oracle=closure; grounder=lazy",
    );
    assert!(observe(&document, text.as_bytes(), automatic).is_err());
}
