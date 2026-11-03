//! Residual receipts describe decoded device verdicts, not completion attempts.

use super::*;

#[test]
fn decoded_residual_reasons_retain_each_cause() {
    let (mut document, text) = formula_fixture();
    let execution = &mut document["statistics"]["execution"];
    execution["gpu_candidates"] = json!(7);
    execution["cpu_residuals"] = json!(6);
    execution["gpu_residuals"] = json!({"fixed_point":1,"round_limit":2,"work_limit":3});
    execution["completion"] = json!({"complete":true,"entered":7,"completed":7,"failed":0,
        "residuals":6,"residual_completed":6,"residual_failed":0});
    let observed = observe(&document, text.as_bytes(), metal()).unwrap();
    assert!(matches!(
        observed.execution.device,
        DeviceWork::Formula {
            candidates: 7,
            cpu_residuals: 6,
            gpu_residuals: Some(FormulaResidualStatistics {
                fixed_point: 1,
                round_limit: 2,
                work_limit: 3,
            }),
            ..
        }
    ));
    let retained = serde_json::to_value(&observed.execution.device).unwrap();
    assert_eq!(
        retained["gpu_residuals"],
        json!({"fixed_point":1,"round_limit":2,"work_limit":3})
    );
}

#[test]
fn historical_residual_reasons_remain_unavailable() {
    let (document, text) = formula_fixture();
    for receipt in [None, Some(Value::Null)] {
        let mut historical = document.clone();
        if let Some(receipt) = receipt {
            historical["statistics"]["execution"]["gpu_residuals"] = receipt;
        }
        let observed = observe(&historical, text.as_bytes(), metal()).unwrap();
        assert!(matches!(
            observed.execution.device,
            DeviceWork::Formula {
                gpu_residuals: None,
                ..
            }
        ));
        assert!(
            serde_json::to_value(&observed.execution.device)
                .unwrap()
                .get("gpu_residuals")
                .is_none()
        );
    }
}

#[test]
fn reported_zero_residuals_remain_measured() {
    let (mut document, text) = formula_fixture();
    let execution = &mut document["statistics"]["execution"];
    execution["gpu_decided"] = json!(2);
    execution["cpu_residuals"] = json!(0);
    execution["gpu_residuals"] = json!({"fixed_point":0,"round_limit":0,"work_limit":0});
    execution["completion"] = json!({"complete":true,"entered":2,"completed":2,"failed":0,
        "residuals":0,"residual_completed":0,"residual_failed":0});
    let observed = observe(&document, text.as_bytes(), metal()).unwrap();
    assert_eq!(
        serde_json::to_value(&observed.execution.device).unwrap()["gpu_residuals"],
        json!({"fixed_point":0,"round_limit":0,"work_limit":0})
    );
}

#[test]
fn residual_reasons_partition_decoded_residual_candidates() {
    let (document, text) = formula_fixture();
    for reasons in [
        json!({"fixed_point":0,"round_limit":0,"work_limit":0}),
        json!({"fixed_point":1,"round_limit":1,"work_limit":0}),
        json!({"fixed_point":u64::MAX,"round_limit":1,"work_limit":1}),
        json!({"fixed_point":1,"round_limit":0,"work_limit":u64::MAX}),
    ] {
        let mut invalid = document.clone();
        invalid["statistics"]["execution"]["gpu_residuals"] = reasons.clone();
        assert!(
            observe(&invalid, text.as_bytes(), metal()).is_err(),
            "{reasons}"
        );
    }
}

#[test]
fn present_residual_receipts_require_unsigned_counters() {
    let (document, text) = formula_fixture();
    for reasons in [
        json!({"fixed_point":1,"round_limit":0}),
        json!({"fixed_point":1,"round_limit":0,"work_limit":-1}),
        json!({"fixed_point":1,"round_limit":"0","work_limit":0}),
        json!({"fixed_point":1,"round_limit":0,"work_limit":false}),
        json!(0),
    ] {
        let mut invalid = document.clone();
        invalid["statistics"]["execution"]["gpu_residuals"] = reasons.clone();
        assert!(
            observe(&invalid, text.as_bytes(), metal()).is_err(),
            "{reasons}"
        );
    }
}

#[test]
fn completion_retries_do_not_duplicate_decoded_reasons() {
    let (mut document, text) = formula_fixture();
    let execution = &mut document["statistics"]["execution"];
    execution["gpu_residuals"] = json!({"fixed_point":0,"round_limit":1,"work_limit":0});
    execution["completion"] = json!({"complete":true,"entered":5,"completed":4,"failed":1,
        "residuals":3,"residual_completed":2,"residual_failed":1});
    assert!(observe(&document, text.as_bytes(), metal()).is_ok());
}

#[test]
fn cpu_routes_cannot_report_decoded_device_residuals() {
    let (document, text) = cpu_batch_fixture();
    for reasons in [
        json!({"fixed_point":0,"round_limit":0,"work_limit":0}),
        json!({"fixed_point":1,"round_limit":0,"work_limit":0}),
    ] {
        let mut invalid = document.clone();
        invalid["statistics"]["execution"]["gpu_residuals"] = reasons;
        assert!(observe(&invalid, text.as_bytes(), NativeExecution::default()).is_err());
    }
}

fn measured_handover() -> Value {
    json!({"attempts":1,"incomplete_attempts":0,"committed_slates":1,"zero_residual_slates":0,
        "query_work":3,"complete_work":true,"counts_exact":true,
        "last_attempt":{"ordinal":1,"slate_sequence":0,"retry":false,"decode_complete":true,"committed":true,
            "residuals":{"fixed_point":1,"round_limit":0,"work_limit":0},"open_atoms":1,"peak_observation_bytes":256,
            "search":{"unentered":0,"certified":0,"exhausted":1,"witnesses":0,"stopped":0,
                "visited":1,"attempted_splits":0,"published_splits":0,"deepest":0,"width":1,"work":3,"host_time_ns":null}}})
}

#[test]
fn historical_handover_absence_is_distinct_from_observed_zero() {
    let (document, text) = formula_fixture();
    for receipt in [
        None,
        Some(Value::Null),
        Some(
            json!({"attempts":0,"incomplete_attempts":0,"committed_slates":0,
            "zero_residual_slates":0,"query_work":0,"complete_work":true,"counts_exact":true,"last_attempt":null}),
        ),
    ] {
        let mut input = document.clone();
        if let Some(r) = &receipt {
            input["statistics"]["execution"]["gpu_residual_handovers"] = r.clone();
        }
        let output = observe(&input, text.as_bytes(), metal()).unwrap();
        let data = serde_json::to_value(output.execution.device).unwrap();
        assert_eq!(
            data.get("gpu_residual_handovers"),
            receipt.as_ref().filter(|v| !v.is_null())
        );
    }
}

#[test]
fn handover_wide_integers_must_come_from_the_exact_capture() {
    let (mut input, text) = formula_fixture();
    input["statistics"]["execution"]["gpu_residual_handovers"] = measured_handover();
    let handover = &mut input["statistics"]["execution"]["gpu_residual_handovers"];
    handover["query_work"] = json!("total-above-u64");
    handover["last_attempt"]["search"]["work"] = json!("query-above-u64");
    let source = input
        .to_string()
        .replace("\"total-above-u64\"", "18446744073709551617")
        .replace("\"query-above-u64\"", "18446744073709551616");
    let document: Value = serde_json::from_str(&source).unwrap();
    assert!(observe(&document, text.as_bytes(), metal()).is_err());
    let observed =
        Observation::from_capture(&document, source.as_bytes(), text.as_bytes(), metal()).unwrap();
    let DeviceWork::Formula {
        gpu_residual_handovers: Some(receipt),
        ..
    } = observed.execution.device
    else {
        panic!("expected exact handover")
    };
    assert_eq!(receipt.query_work, u128::from(u64::MAX) + 2);
    let query = receipt.last_attempt.unwrap().search.unwrap();
    assert_eq!(query.work, u128::from(u64::MAX) + 1);
    assert_eq!(
        (query.deepest, query.width, query.host_time_ns),
        (Some(0), Some(1), None)
    );
}

#[test]
fn malformed_handover_fields_and_inconsistent_slot_counts_are_refused() {
    let (document, text) = formula_fixture();
    for mutation in 0..4 {
        let mut record = measured_handover();
        match mutation {
            0 => {
                record["last_attempt"]
                    .as_object_mut()
                    .unwrap()
                    .remove("search");
            }
            1 => record["last_attempt"]["search"]["published_splits"] = json!(1),
            2 => record["last_attempt"]["search"]["unentered"] = json!(1),
            _ => record["query_work"] = json!(3.5),
        }
        let mut input = document.clone();
        input["statistics"]["execution"]["gpu_residual_handovers"] = record;
        let bytes = serde_json::to_vec(&input).unwrap();
        assert!(Observation::from_capture(&input, &bytes, text.as_bytes(), metal()).is_err());
    }
}

#[test]
fn tight_check_refuses_a_regional_handover_receipt() {
    let (mut document, text) = super::tight_tests::tight_fixture();
    document["statistics"]["execution"]["gpu_residual_handovers"] = measured_handover();
    assert!(observe(&document, text.as_bytes(), metal()).is_err());
}

#[test]
fn incomplete_decode_retains_reasons_without_fabricating_a_host_join() {
    let (mut input, text) = formula_fixture();
    let mut handover = measured_handover();
    handover["committed_slates"] = json!(0);
    handover["incomplete_attempts"] = json!(1);
    handover["query_work"] = json!(0);
    let last = &mut handover["last_attempt"];
    last["committed"] = json!(false);
    last["decode_complete"] = json!(false);
    last["search"] = Value::Null;
    input["statistics"]["execution"]["gpu_residual_handovers"] = handover;
    let report = observe(&input, text.as_bytes(), metal()).unwrap();
    let DeviceWork::Formula {
        gpu_residual_handovers: Some(receipt),
        ..
    } = report.execution.device
    else {
        panic!("missing observed prefix")
    };
    let attempt = receipt.last_attempt.unwrap();
    assert!(!attempt.decode_complete && !attempt.committed);
    assert_eq!(attempt.residuals.fixed_point, 1);
    assert!(attempt.search.is_none());
}

#[test]
fn per_reason_search_stays_with_its_decoded_slot_population() {
    let (mut document, text) = formula_fixture();
    let mut receipt = measured_handover();
    let empty = json!({"open_atoms":0,"unentered":0,"certified":0,"exhausted":0,"witnesses":0,"stopped":0,
                        "visited":0,"attempted_splits":0,"published_splits":0,"work":0});
    let mut served = empty.clone();
    served["open_atoms"] = json!(1);
    served["exhausted"] = json!(1);
    served["visited"] = json!(1);
    served["work"] = json!(3);
    receipt["last_attempt"]["by_reason"] =
        json!({"fixed_point":served, "round_limit":empty, "work_limit":empty});
    document["statistics"]["execution"]["gpu_residual_handovers"] = receipt;
    assert!(observe(&document, text.as_bytes(), metal()).is_ok());
    let groups = &mut document["statistics"]["execution"]["gpu_residual_handovers"]["last_attempt"]
        ["by_reason"];
    groups["round_limit"] = served;
    groups["fixed_point"] = empty;
    assert!(observe(&document, text.as_bytes(), metal()).is_err());
}

#[test]
fn sampling_receipts_preserve_unknown_counts_on_legacy_compatible_input() {
    let (mut document, text) = formula_fixture();
    let mut handover = measured_handover();
    handover["samples"] = json!({
        "version":1,"algorithm":"splitmix64-reject-v1","seed":5,
        "byte_limit":100,"retained_bytes":0,"peak_bytes":80,
        "worlds":{"seen":null,"capacity":0,"retained":0},
        "batches":{"seen":1,"capacity":1,"retained":1},"groups":null
    });
    document["statistics"]["execution"]["gpu_residual_handovers"] = handover;
    let observation = observe(&document, text.as_bytes(), metal()).unwrap();
    let DeviceWork::Formula {
        gpu_residual_handovers: Some(receipt),
        ..
    } = observation.execution.device
    else {
        panic!("general GPU handover receipt expected")
    };
    let sample = receipt.samples.unwrap();
    assert_eq!(sample.worlds.seen, None);
    assert_eq!(sample.batches.seen, Some(1));
    assert_eq!(sample.groups, None);
}

#[test]
fn sampling_receipt_rejects_extra_batch_or_overfull_reservoir() {
    let (document, text) = formula_fixture();
    let sample = json!({"version":1,"algorithm":"splitmix64-reject-v1","seed":5,
        "byte_limit":100,"retained_bytes":0,"peak_bytes":80,
        "worlds":{"seen":1,"capacity":1,"retained":1},
        "batches":{"seen":1,"capacity":1,"retained":1},"groups":null});
    for (name, field, bad) in [("batches", "seen", 2), ("worlds", "retained", 2)] {
        let mut input = document.clone();
        let mut handover = measured_handover();
        handover["samples"] = sample.clone();
        handover["samples"][name][field] = json!(bad);
        input["statistics"]["execution"]["gpu_residual_handovers"] = handover;
        assert!(observe(&input, text.as_bytes(), metal()).is_err());
    }
}

#[test]
fn sample_rank_claims_require_coverage_and_a_matching_retained_count() {
    let (document, text) = formula_fixture();
    let metrics = [
        "open_atoms",
        "visited",
        "attempted_splits",
        "published_splits",
        "depth",
        "work",
        "query_ns",
        "residuals",
        "width",
        "batch_ns",
    ];
    let empty = json!({"eligible":0,"known":0,"retained":0,"sum":0,"maximum":null,
        "median":null,"p90":null,"quantiles":"unavailable"});
    let mut group = json!({});
    for field in metrics {
        group[field] = empty.clone();
    }
    let mut handover = measured_handover();
    let mut groups = json!({});
    for reason in ["all", "fixed_point", "round_limit", "work_limit"] {
        groups[reason] = group.clone();
        let n = if matches!(reason, "all" | "fixed_point") {
            3
        } else {
            0
        };
        groups[reason]["residuals"] = json!({"eligible":1,"known":1,"retained":1,
            "sum":n,"maximum":n,"median":n,"p90":n,"quantiles":"exact"});
        for field in ["width", "batch_ns"] {
            groups[reason][field] = json!({"eligible":1,"known":0,"retained":0,
                "sum":null,"maximum":null,"median":null,"p90":null,"quantiles":"unavailable"});
        }
    }
    handover["last_attempt"]["residuals"]["fixed_point"] = json!(3);
    handover["last_attempt"]["search"]["certified"] = json!(2);
    handover["samples"] = json!({
        "version":1,"algorithm":"splitmix64-reject-v1","seed":0,"byte_limit":4096,
        "retained_bytes":256,"peak_bytes":512,
        "worlds":{"seen":3,"capacity":1,"retained":1},
        "batches":{"seen":1,"capacity":1,"retained":1},"groups":groups});
    let metric = json!({"eligible":3,"known":3,"retained":1,"sum":22,"maximum":10,
        "median":8,"p90":8,"quantiles":"estimated"});
    for (change, accepted) in [
        (json!({}), true),
        (json!({"quantiles":"exact"}), false),
        (json!({"known":1}), false),
        (json!({"retained":0}), false),
        (json!({"maximum":5}), false),
        (json!({"p90":null}), false),
    ] {
        let mut input = document.clone();
        let mut record = metric.clone();
        record
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        handover["samples"]["groups"]["all"]["work"] = record;
        input["statistics"]["execution"]["gpu_residual_handovers"] = handover.clone();
        assert_eq!(
            observe(&input, text.as_bytes(), metal()).is_ok(),
            accepted,
            "{change}"
        );
    }
}
