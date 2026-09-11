use super::*;
#[test]
fn formula_profile_checks_every_advertised_compute_requirement() {
    let good = wgpu::Limits::default();
    check_limits(&good).unwrap();
    for field in 0..6 {
        let mut limits = good.clone();
        match field {
            0 => limits.max_compute_invocations_per_workgroup = 63,
            1 => limits.max_compute_workgroup_size_x = 63,
            2 => limits.max_compute_workgroup_storage_size = 15,
            3 => limits.max_storage_buffers_per_shader_stage = 5,
            4 => limits.max_uniform_buffers_per_shader_stage = 0,
            _ => limits.max_bindings_per_bind_group = 6,
        }
        assert_eq!(
            check_limits(&limits).unwrap_err().kind(),
            GpuErrorKind::Capacity
        );
    }
}

fn busy_preserves_state(backend: GpuBackendPreference) {
    use zetesis_ferraris::{AdmissionLimits, Node};
    let mut oracle = GpuFormulaOracle::new_selected(
        GpuOptions::default(),
        GpuSelection {
            backend,
            vendor_id: None,
        },
    )
    .unwrap();
    assert!(oracle.info().is_hardware_gpu());
    assert_eq!(
        oracle.info().backend(),
        match backend {
            GpuBackendPreference::Metal => "Metal",
            GpuBackendPreference::Vulkan => "Vulkan",
            _ => panic!("explicit device fixture"),
        }
    );
    let context = oracle.context().clone();
    let theory = Theory::new(1, vec![Node::Atom(0)], vec![0], AdmissionLimits::default()).unwrap();
    let candidates = [Interpretation::new(&theory, [0]).unwrap()];
    oracle
        .propagate_batch(&theory, &candidates, super::super::FormulaLimits::default())
        .unwrap();
    let epoch = oracle.epoch;
    {
        let _lease = context.lease().unwrap();
        let failure = oracle
            .propagate_batch(&theory, &candidates, super::super::FormulaLimits::default())
            .unwrap_err();
        assert_eq!(failure.kind(), GpuErrorKind::Busy);
        assert_eq!(oracle.epoch, epoch);
        assert!(oracle.last_batch_stats().is_none());
        assert!(oracle.resident.is_some());
        assert!(context.check_health().is_ok());
        let created = GpuFormulaOracle::from_context(&context);
        assert!(matches!(created, Err(error) if error.kind() == GpuErrorKind::Busy));
    }
    let checks = oracle
        .propagate_batch(&theory, &candidates, super::super::FormulaLimits::default())
        .unwrap();
    assert_eq!(
        checks[0].verdict(),
        super::super::FormulaVerdict::NoProperSubset
    );
    assert_eq!(oracle.epoch, epoch + 1);
    let stats = oracle.last_batch_stats().unwrap();
    assert!(!stats.theory_uploaded);
    assert!(!stats.transport_allocated);
}

#[test]
#[ignore = "requires actual Metal; explicit physical qualification"]
fn metal_busy_refusal_preserves_formula_state() {
    busy_preserves_state(GpuBackendPreference::Metal);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_busy_refusal_preserves_formula_state() {
    busy_preserves_state(GpuBackendPreference::Vulkan);
}
