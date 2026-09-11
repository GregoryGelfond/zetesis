use super::*;
use crate::AggregateGpuPlanLimits;
use crate::aggregate::tests::fixtures;
use crate::{GpuBackendPreference, GpuErrorKind, GpuSelection};
use zetesis_ferraris::native_aggregate::Function;

#[test]
fn granted_aggregate_resources_are_checked_individually() {
    let limits = wgpu::Limits::default();
    assert!(check_limits(&limits).is_ok());
    for limited in [
        wgpu::Limits {
            max_compute_invocations_per_workgroup: 63,
            ..limits.clone()
        },
        wgpu::Limits {
            max_compute_workgroup_size_x: 63,
            ..limits.clone()
        },
        wgpu::Limits {
            max_compute_workgroup_storage_size: 1023,
            ..limits.clone()
        },
        wgpu::Limits {
            max_storage_buffers_per_shader_stage: 3,
            ..limits.clone()
        },
        wgpu::Limits {
            max_uniform_buffers_per_shader_stage: 0,
            ..limits.clone()
        },
        wgpu::Limits {
            max_bindings_per_bind_group: 4,
            ..limits.clone()
        },
    ] {
        assert_eq!(
            check_limits(&limited).unwrap_err().kind(),
            GpuErrorKind::Capacity
        );
    }
}

fn corrupt_identity(selection: GpuSelection) -> GpuAggregateOracle {
    // Keep the actual pipeline, dispatch and transport. Only the returned epoch
    // is deliberately corrupt, so successful submissions cannot become verdicts.
    let shader = SHADER.replace("output[result] = dimensions.epoch;", "output[result] = 0u;");
    assert_ne!(shader, SHADER);
    let runtime = pollster::block_on(Runtime::new(
        GpuOptions::default(),
        selection,
        DeviceProfile {
            device_label: "aggregate malformed readback control",
            shader_label: "aggregate corrupted epoch control",
            pipeline_label: "aggregate invalid result control",
            shader: shader.into(),
            entry_point: "reduce",
            validate_limits: check_limits,
        },
    ))
    .unwrap();
    GpuAggregateOracle {
        runtime,
        resident: None,
        epoch: 0,
        last: None,
        activity: AggregateGpuActivity::default(),
    }
}

#[test]
#[ignore = "requires actual Metal; explicit physical qualification"]
fn metal_aggregate_readback_failure_retains_submitted_work() {
    qualify_failure(GpuBackendPreference::Metal);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_aggregate_readback_failure_retains_submitted_work() {
    qualify_failure(GpuBackendPreference::Vulkan);
}

fn qualify_failure(backend: GpuBackendPreference) {
    let mut oracle = corrupt_identity(GpuSelection {
        backend,
        vendor_id: None,
    });
    assert!(oracle.info().is_hardware_gpu());
    assert_eq!(
        oracle.info().backend(),
        match backend {
            GpuBackendPreference::Metal => "Metal",
            GpuBackendPreference::Vulkan => "Vulkan",
            GpuBackendPreference::Auto | GpuBackendPreference::Dx12 | GpuBackendPreference::Gl =>
                panic!("explicit physical fixture"),
        }
    );
    let theory = fixtures::theory();
    let group = fixtures::group(&theory, Function::Sum, 65);
    let worlds = fixtures::worlds(&theory);
    let records = fixtures::observations(&group, &worlds, 33);
    let plan = AggregateGpuPlan::new(
        &group,
        AggregateGpuPlanLimits::default(),
        &Control::default(),
    )
    .unwrap();
    let error = oracle
        .check_batch(
            &plan,
            &records,
            AggregateGpuLimits::default(),
            &Control::default(),
        )
        .unwrap_err();
    assert!(
        matches!(error, AggregateGpuError::Gpu(error) if error.kind() == GpuErrorKind::Readback)
    );
    let submitted = oracle.activity();
    assert_eq!(submitted.submissions, 1);
    assert_eq!(submitted.submitted_occurrences, records.len() as u64);
    assert!(submitted.scheduled_work > 0);
    assert!(submitted.uploaded_bytes > plan.bytes());
    assert_eq!(submitted.completed_occurrences, 0);
    assert_eq!(submitted.completed_work, 0);
    assert_eq!(submitted.downloaded_bytes, 0);
    assert!(oracle.resident.is_none());
    assert!(oracle.last_batch_stats().is_none());
    oracle.clear_residency();
    assert_eq!(oracle.activity(), submitted);
    assert!(
        matches!(oracle.check_batch(&plan, &[], AggregateGpuLimits::default(), &Control::default()), Err(AggregateGpuError::Gpu(error)) if error.kind() == GpuErrorKind::Device)
    );
}

fn invalidates_shared_primitive(backend: GpuBackendPreference) {
    let mut aggregate = corrupt_identity(GpuSelection {
        backend,
        vendor_id: None,
    });
    assert!(aggregate.info().is_hardware_gpu());
    let context = aggregate.context().clone();
    let mut formula = crate::GpuFormulaOracle::from_context(&context).unwrap();
    let mut lazy = crate::GpuLazyOracle::from_context(&context).unwrap();
    assert!(formula.context().same_instance(aggregate.context()));
    assert!(lazy.context().same_instance(aggregate.context()));
    let empty =
        zetesis_core::Program::new(vec![], zetesis_core::AdmissionLimits::default()).unwrap();
    let empty_batch = lazy
        .check_batch(
            &empty,
            &[],
            zetesis_cpu::lazy::Limits::default(),
            crate::GpuLimits::default(),
            &Control::default(),
        )
        .unwrap();
    assert!(empty_batch.checks.is_empty());
    let theory = fixtures::theory();
    let group = fixtures::group(&theory, Function::Sum, 1);
    let worlds = fixtures::worlds(&theory);
    let records = fixtures::observations(&group, &worlds, 1);
    let plan = AggregateGpuPlan::new(
        &group,
        AggregateGpuPlanLimits::default(),
        &Control::default(),
    )
    .unwrap();
    let outcome = aggregate.check_batch(
        &plan,
        &records,
        AggregateGpuLimits::default(),
        &Control::default(),
    );
    assert!(
        matches!(outcome, Err(AggregateGpuError::Gpu(error)) if error.kind() == GpuErrorKind::Readback)
    );
    assert_eq!(aggregate.activity().submissions, 1);
    assert_eq!(aggregate.activity().completed_occurrences, 0);
    // An empty peer call must still observe the failed shared device.
    let failure = formula
        .propagate_batch(&theory, &[], crate::FormulaLimits::default())
        .unwrap_err();
    assert_eq!(failure.kind(), GpuErrorKind::Device);
    let failure = lazy
        .check_batch(
            &empty,
            &[],
            zetesis_cpu::lazy::Limits::default(),
            crate::GpuLimits::default(),
            &Control::default(),
        )
        .unwrap_err();
    assert!(matches!(failure.cause,
        zetesis_cpu::lazy::Cause::Execution(error) if error.kind() == GpuErrorKind::Device
    ));
    assert_eq!(failure.progress, zetesis_cpu::lazy::Progress::default());
    assert_eq!(lazy.statistics().dispatches, 0);
    let cancelled = Control::default();
    cancelled.cancel();
    let failure = lazy
        .check_batch(
            &empty,
            &[],
            zetesis_cpu::lazy::Limits::default(),
            crate::GpuLimits::default(),
            &cancelled,
        )
        .unwrap_err();
    assert!(matches!(
        failure.cause,
        zetesis_cpu::lazy::Cause::Source(zetesis_cpu::Stop::Cancelled)
    ));
    aggregate.clear_residency();
    formula.clear_residency();
    assert!(matches!(
        crate::GpuFormulaOracle::from_context(&context),
        Err(error) if error.kind() == GpuErrorKind::Device
    ));
}

#[test]
#[ignore = "requires actual Metal; explicit physical qualification"]
fn metal_readback_failure_invalidates_context_peers() {
    invalidates_shared_primitive(GpuBackendPreference::Metal);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_readback_failure_invalidates_context_peers() {
    invalidates_shared_primitive(GpuBackendPreference::Vulkan);
}
