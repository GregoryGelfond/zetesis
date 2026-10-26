use super::*;

#[test]
fn both_tight_kernels_translate_to_spirv() {
    // Each support policy's kernel, with its dense twin.
    for support in [TightSupport::Atomic, TightSupport::Grouped] {
        crate::spirv_translation::assert_profile_translates(&GpuTightOracle::profile(support));
    }
}

#[test]
fn device_profile_refuses_each_insufficient_capability() {
    let good = wgpu::Limits::default();
    check_limits(&good).unwrap();
    for field in 0..6 {
        let mut limits = good.clone();
        match field {
            0 => limits.max_compute_invocations_per_workgroup = 63,
            1 => limits.max_compute_workgroup_size_x = 63,
            2 => limits.max_compute_workgroup_storage_size = 39,
            3 => limits.max_storage_buffers_per_shader_stage = 7,
            4 => limits.max_uniform_buffers_per_shader_stage = 0,
            _ => limits.max_bindings_per_bind_group = 8,
        }
        let error = check_limits(&limits).unwrap_err();
        assert_eq!(error.kind(), GpuErrorKind::Capacity);
        assert!(error.detail().starts_with("tight "));
    }
}

/// The dense kernel serves exactly the stages that decline the route for the
/// theory's residency; a theory that may still engage runs the full kernel.
#[test]
fn only_stages_declined_for_the_residency_run_the_dense_kernel() {
    for stage in [Stage::Serial, Stage::Narrow, Stage::Refused] {
        assert_eq!(kernel_variant(&stage), Variant::Dense);
    }
    assert_eq!(kernel_variant(&Stage::Pending), Variant::Full);
}
