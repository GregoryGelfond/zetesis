//! Advertised requirements are those of the immutable-round shader.

use crate::GpuErrorKind;

fn exact() -> wgpu::Limits {
    wgpu::Limits {
        max_compute_invocations_per_workgroup: 64,
        max_compute_workgroup_size_x: 64,
        max_compute_workgroup_storage_size: 0,
        max_storage_buffers_per_shader_stage: 5,
        max_uniform_buffers_per_shader_stage: 1,
        max_bindings_per_bind_group: 6,
        ..wgpu::Limits::default()
    }
}

#[test]
fn lazy_profile_admits_its_exact_shader_resources() {
    let module = naga::front::wgsl::parse_str(super::SHADER).unwrap();
    assert_eq!(module.entry_points[0].workgroup_size, [64, 1, 1]);
    let mut storage = 0;
    let mut uniform = 0;
    for (_, global) in module.global_variables.iter() {
        match global.space {
            naga::AddressSpace::Storage { .. } => storage += 1,
            naga::AddressSpace::Uniform => uniform += 1,
            naga::AddressSpace::WorkGroup => panic!("lazy admission promises no workgroup storage"),
            _ => {}
        }
    }
    assert_eq!((storage, uniform), (5, 1));
    (super::GpuLazyOracle::profile().validate_limits)(&exact()).unwrap();
    assert_eq!(
        crate::check_adapter_limits(&exact()).unwrap_err().kind(),
        GpuErrorKind::Capacity
    );
}

#[test]
fn lazy_profile_refuses_each_resource_one_below_its_requirement() {
    for field in 0..5 {
        let mut limits = exact();
        match field {
            0 => limits.max_compute_invocations_per_workgroup -= 1,
            1 => limits.max_compute_workgroup_size_x -= 1,
            2 => limits.max_storage_buffers_per_shader_stage -= 1,
            3 => limits.max_uniform_buffers_per_shader_stage -= 1,
            _ => limits.max_bindings_per_bind_group -= 1,
        }
        assert_eq!(
            (super::GpuLazyOracle::profile().validate_limits)(&limits)
                .unwrap_err()
                .kind(),
            GpuErrorKind::Capacity,
            "field {field}"
        );
    }
}

#[test]
fn lazy_uniform_contains_only_the_live_protocol_fields() {
    let module = naga::front::wgsl::parse_str(super::SHADER).unwrap();
    let (_, dimensions) = module
        .types
        .iter()
        .find(|(_, ty)| ty.name.as_deref() == Some("Dimensions"))
        .unwrap();
    let naga::TypeInner::Struct { members, span } = &dimensions.inner else {
        panic!("uniform has a defined struct layout");
    };
    assert_eq!(u64::from(*span), super::UNIFORM_BYTES);
    assert_eq!(*span, 16);
    assert_eq!(
        members
            .iter()
            .map(|member| (member.name.as_deref(), member.offset))
            .collect::<Vec<_>>(),
        [
            (Some("words"), 0),
            (Some("rules"), 4),
            (Some("worlds"), 8),
            (Some("epoch"), 12)
        ]
    );
}

#[test]
fn lazy_plan_owns_the_supplied_submission_identity() {
    super::tests::inspect(|chunk| {
        for previous in [0, 41, u32::MAX - 1] {
            let epoch = crate::packing::next_epoch(previous).unwrap();
            let plan = super::Plan::new(
                epoch,
                chunk,
                crate::GpuLimits::default(),
                &wgpu::Limits::default(),
            )
            .unwrap();
            assert_eq!(plan.params(), [1, 1, 1, previous + 1]);
            assert_eq!(plan.decode(&[0, 0, 0, previous + 1]).unwrap(), [0, 0]);
            assert_eq!(
                plan.decode(&[0, 0, 0, previous]).unwrap_err().kind(),
                GpuErrorKind::Readback
            );
        }
        assert_eq!(
            crate::packing::next_epoch(u32::MAX).unwrap_err().kind(),
            GpuErrorKind::Capacity
        );
    });
}
