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
