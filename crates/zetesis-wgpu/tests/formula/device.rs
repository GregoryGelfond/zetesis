use super::super::profile::check_limits;
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

fn require_profile_device(profile: &GpuFormulaProfile, backend: GpuBackendPreference) {
    assert!(profile.info().is_hardware_gpu());
    assert_eq!(
        profile.info().backend(),
        match backend {
            GpuBackendPreference::Metal => "Metal",
            GpuBackendPreference::Vulkan => "Vulkan",
            _ => panic!("explicit physical fixture"),
        }
    );
    println!(
        "profile adapter={:?} projection={}",
        profile.info().metadata(),
        profile.projection().label()
    );
}

fn reused_profile(backend: GpuBackendPreference) {
    use zetesis_ferraris::{AdmissionLimits, Node};
    for projection in GateProjection::ALL {
        let profile = GpuFormulaProfile::new_selected_with_projection(
            GpuOptions::default(),
            GpuSelection {
                backend,
                vendor_id: None,
            },
            projection,
        )
        .unwrap();
        require_profile_device(&profile, backend);
        let theory =
            Theory::new(1, vec![Node::Atom(0)], vec![0], AdmissionLimits::default()).unwrap();
        let candidates = [Interpretation::new(&theory, [0]).unwrap()];
        let mut previous = GpuFormulaOracle::from_profile(&profile).unwrap();
        let expected = previous
            .propagate_batch(&theory, &candidates, FormulaLimits::default())
            .unwrap();
        assert_eq!(previous.epoch, 1);
        assert!(previous.resident.is_some());
        // Three live consumers retain the exact compiled owner, while each first
        // dispatch uploads fresh subject and transport with an independent epoch.
        let mut consumers = Vec::new();
        for _ in 0..2 {
            let mut oracle = GpuFormulaOracle::from_profile(&profile.clone()).unwrap();
            assert!(
                oracle
                    .compiled_profile()
                    .same_instance(previous.compiled_profile())
            );
            assert!(oracle.context().same_instance(profile.context()));
            assert_eq!(oracle.projection(), projection);
            assert_eq!(oracle.epoch, 0);
            assert!(oracle.resident.is_none());
            assert!(oracle.last_batch_stats().is_none());
            assert_eq!(
                oracle
                    .propagate_batch(&theory, &candidates, FormulaLimits::default())
                    .unwrap(),
                expected
            );
            assert_eq!(oracle.epoch, 1);
            let stats = oracle.last_batch_stats().unwrap();
            assert!(stats.theory_uploaded);
            assert!(stats.transport_allocated);
            consumers.push(oracle);
        }
        consumers[0].clear_residency();
        assert!(consumers[1].resident.is_some());
        assert_eq!(
            previous
                .propagate_batch(&theory, &candidates, FormulaLimits::default())
                .unwrap(),
            expected
        );
        assert_eq!(previous.epoch, 2);
        assert!(!previous.last_batch_stats().unwrap().theory_uploaded);
    }
}

#[test]
#[ignore = "requires actual Metal; explicit compiled-profile qualification"]
fn metal_profile_starts_fresh_formula_oracles() {
    reused_profile(GpuBackendPreference::Metal);
}

#[test]
#[ignore = "requires actual Vulkan; explicit compiled-profile qualification"]
fn vulkan_profile_starts_fresh_formula_oracles() {
    reused_profile(GpuBackendPreference::Vulkan);
}

fn compilation_identity(backend: GpuBackendPreference) {
    let profile = GpuFormulaProfile::new_selected(
        GpuOptions::default(),
        GpuSelection {
            backend,
            vendor_id: None,
        },
    )
    .unwrap();
    require_profile_device(&profile, backend);
    assert!(profile.same_instance(&profile.clone()));
    let independent = GpuFormulaProfile::from_context(profile.context()).unwrap();
    assert!(profile.context().same_instance(independent.context()));
    assert_eq!(profile.projection(), independent.projection());
    assert!(!profile.same_instance(&independent));
    let bitwise =
        GpuFormulaProfile::from_context_with_projection(profile.context(), GateProjection::Bitwise)
            .unwrap();
    assert!(profile.context().same_instance(bitwise.context()));
    assert_eq!(bitwise.projection(), GateProjection::Bitwise);
    assert!(!profile.same_instance(&bitwise));
}

#[test]
#[ignore = "requires actual Metal; exact compilation identity"]
fn metal_profiles_identify_exact_compilations() {
    compilation_identity(GpuBackendPreference::Metal);
}

#[test]
#[ignore = "requires actual Vulkan; exact compilation identity"]
fn vulkan_profiles_identify_exact_compilations() {
    compilation_identity(GpuBackendPreference::Vulkan);
}

fn profile_lifecycle(backend: GpuBackendPreference) {
    let profile = GpuFormulaProfile::new_selected(
        GpuOptions::default(),
        GpuSelection {
            backend,
            vendor_id: None,
        },
    )
    .unwrap();
    require_profile_device(&profile, backend);
    let context = profile.context();
    {
        let _lease = context.lease().unwrap();
        assert!(matches!(GpuFormulaOracle::from_profile(&profile),
            Err(error) if error.kind() == GpuErrorKind::Busy));
        assert!(context.check_health().is_ok());
    }
    let mut oracle = GpuFormulaOracle::from_profile(&profile).unwrap();
    {
        let _lease = context.lease().unwrap();
        context.invalidate();
        assert!(matches!(GpuFormulaOracle::from_profile(&profile),
            Err(error) if error.kind() == GpuErrorKind::Busy));
    }
    assert!(matches!(GpuFormulaOracle::from_profile(&profile),
        Err(error) if error.kind() == GpuErrorKind::Device));
    let empty = Theory::new(
        0,
        vec![],
        vec![],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    assert_eq!(
        oracle
            .propagate_batch(&empty, &[], FormulaLimits::default())
            .unwrap_err()
            .kind(),
        GpuErrorKind::Device
    );
    assert_eq!(oracle.epoch, 0);
    assert!(oracle.resident.is_none());
}

#[test]
#[ignore = "requires actual Metal; profile reuse retains shared failure precedence"]
fn metal_profile_reuse_checks_context_lifecycle() {
    profile_lifecycle(GpuBackendPreference::Metal);
}

#[test]
#[ignore = "requires actual Vulkan; profile reuse retains shared failure precedence"]
fn vulkan_profile_reuse_checks_context_lifecycle() {
    profile_lifecycle(GpuBackendPreference::Vulkan);
}
