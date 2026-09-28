//! Residual receipts describe decoded device verdicts, not completion attempts.

use super::*;

fn metal() -> NativeExecution {
    NativeExecution {
        backend: Backend::Gpu(Some(GpuApi::Metal)),
        ..Default::default()
    }
}

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
