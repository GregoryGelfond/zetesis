use super::super::profile::check_limits;
use super::*;
#[test]
fn formula_profile_checks_every_advertised_compute_requirement() {
    let good = wgpu::Limits {
        max_compute_workgroup_storage_size: 24,
        ..wgpu::Limits::default()
    };
    check_limits(&good).unwrap();
    for field in 0..6 {
        let mut limits = good.clone();
        match field {
            0 => limits.max_compute_invocations_per_workgroup = 63,
            1 => limits.max_compute_workgroup_size_x = 63,
            2 => limits.max_compute_workgroup_storage_size = 23,
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
    cold_preparation_refusals(&mut oracle, &theory, &candidates);
}

fn cold_preparation_refusals(
    oracle: &mut GpuFormulaOracle,
    old: &Theory,
    candidates: &[Interpretation],
) {
    use zetesis_ferraris::{AdmissionLimits, Node};
    let next = Theory::new(
        1,
        vec![Node::Atom(0), Node::Atom(0), Node::And(0, 1)],
        vec![2],
        AdmissionLimits::default(),
    )
    .unwrap();
    let inputs = [Interpretation::new(&next, [0]).unwrap()];
    let epoch = oracle.epoch;
    let error = oracle
        .propagate_batch(
            &next,
            &inputs,
            FormulaLimits {
                max_batch_bytes: 0,
                ..Default::default()
            },
        )
        .unwrap_err();
    assert_eq!(error.kind(), GpuErrorKind::Capacity);
    assert!(
        oracle
            .resident
            .as_ref()
            .unwrap()
            .graph
            .shape
            .theory
            .same_instance(old)
    );
    assert_eq!(
        oracle
            .propagate_batch(&next, candidates, FormulaLimits::default())
            .unwrap_err()
            .kind(),
        GpuErrorKind::Seed
    );
    assert!(
        oracle
            .resident
            .as_ref()
            .unwrap()
            .graph
            .shape
            .theory
            .same_instance(old)
    );
    // Minimum setup8 passes; the two-level setup10 refuses after cold packing.
    let error = oracle
        .propagate_batch(
            &next,
            &inputs,
            FormulaLimits {
                max_work_per_candidate: 9,
                ..Default::default()
            },
        )
        .unwrap_err();
    assert_eq!(error.kind(), GpuErrorKind::Capacity);
    assert!(oracle.resident.is_none());
    assert_eq!(oracle.epoch, epoch);
    assert!(oracle.last_batch_stats().is_none());
    assert_eq!(oracle.last_submission_candidates(), None);
    oracle.context().check_health().unwrap();
    oracle
        .propagate_batch(old, candidates, FormulaLimits::default())
        .unwrap();
    assert!(oracle.last_batch_stats().unwrap().theory_uploaded);
    let cancelled = Control::default();
    cancelled.cancel();
    let epoch = oracle.epoch;
    // Directly enter the host preparation phase, after the public pre-poll.
    let error = oracle
        .prepare(
            &next,
            &inputs,
            FormulaLimits::default(),
            &cancelled,
            epoch + 1,
        )
        .err()
        .unwrap();
    assert_eq!(error.interruption(), Some(zetesis_cpu::Stop::Cancelled));
    assert!(oracle.resident.is_none());
    assert_eq!(oracle.epoch, epoch);
    oracle.context().check_health().unwrap();
    println!("formula minimum/identity preserve residency; late preparation leaves healthy-cold");
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

fn submission_receipt(backend: GpuBackendPreference) {
    let mut oracle = GpuFormulaOracle::new_selected(
        GpuOptions::default(),
        GpuSelection {
            backend,
            vendor_id: None,
        },
    )
    .unwrap();
    require_profile_device(oracle.compiled_profile(), backend);
    let theory = Theory::new(
        1,
        vec![zetesis_ferraris::Node::Atom(0)],
        vec![0],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let candidates = [
        Interpretation::new(&theory, []).unwrap(),
        Interpretation::new(&theory, [0]).unwrap(),
    ];
    let limits = FormulaLimits {
        max_rounds: 0,
        max_work_per_candidate: 4,
        ..Default::default()
    };
    let checks = oracle
        .propagate_batch(&theory, &candidates, limits)
        .unwrap();
    assert_eq!(oracle.last_submission_candidates(), Some(2));
    assert_eq!(checks[0].verdict(), super::super::FormulaVerdict::NotModel);
    assert_eq!(
        checks[1].verdict(),
        super::super::FormulaVerdict::Residual(super::super::ResidualReason::RoundLimit)
    );
    for check in checks {
        assert_eq!(
            check.statistics(),
            super::super::FormulaStatistics { work: 4, rounds: 0 }
        );
    }
    oracle.propagate_batch(&theory, &[], limits).unwrap();
    assert_eq!(oracle.last_submission_candidates(), None);
    let cancelled = Control::default();
    cancelled.cancel();
    let failure = oracle
        .propagate_batch_with_control(&theory, &candidates, limits, &cancelled)
        .unwrap_err();
    assert_eq!(failure.interruption(), Some(zetesis_cpu::Stop::Cancelled));
    assert_eq!(oracle.last_submission_candidates(), None);
    for work in [0, 3] {
        let failure = oracle
            .propagate_batch(
                &theory,
                &candidates,
                FormulaLimits {
                    max_work_per_candidate: work,
                    ..limits
                },
            )
            .unwrap_err();
        assert_eq!(failure.kind(), GpuErrorKind::Capacity);
        assert_eq!(oracle.last_submission_candidates(), None);
        oracle.context().check_health().unwrap();
    }
    submitted_interruption(&mut oracle, &candidates, limits, &cancelled);
}

fn submitted_interruption(
    oracle: &mut GpuFormulaOracle,
    candidates: &[Interpretation],
    limits: FormulaLimits,
    cancelled: &Control,
) {
    // Invoke the same transport after host admission with already-stopped control.
    // The receipt is written only after queue.submit; readback then observes Stop.
    // This controls the phase exactly, without a race against shader duration.
    let runtime = &oracle.profile.runtime;
    let _lease = runtime.context.lease().unwrap();
    let resident = oracle.resident.as_mut().unwrap();
    let plan = Plan::new(
        &resident.graph,
        candidates.len(),
        limits,
        runtime.limits(),
        false,
        2,
    )
    .unwrap();
    let seeds = plan.pack(&resident.graph, candidates).unwrap();
    let scopes = ErrorScopes::new(runtime.device());
    let outcome = resident.dispatch(
        runtime,
        &seeds,
        &plan,
        limits.timeout,
        cancelled,
        &mut oracle.last_submission_candidates,
    );
    let failure = runtime.complete(scopes, outcome).unwrap_err();
    assert_eq!(failure.kind(), GpuErrorKind::Interrupted);
    assert_eq!(failure.interruption(), Some(zetesis_cpu::Stop::Cancelled));
    assert_eq!(oracle.last_submission_candidates, Some(2));
    assert!(oracle.last.is_none());
    assert_eq!(
        runtime.check_health().unwrap_err().kind(),
        GpuErrorKind::Device
    );
    println!("formula submitted=2 decoded=0 stop=Cancelled phase=after-queue-submit");
}

#[test]
#[ignore = "requires actual Metal; checks submission phase and device limits"]
fn metal_formula_submission_receipt_survives_interruption() {
    submission_receipt(GpuBackendPreference::Metal);
}

#[test]
#[ignore = "requires actual Vulkan; checks submission phase and device limits"]
fn vulkan_formula_submission_receipt_survives_interruption() {
    submission_receipt(GpuBackendPreference::Vulkan);
}
