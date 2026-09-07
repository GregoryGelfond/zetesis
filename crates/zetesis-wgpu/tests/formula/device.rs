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
