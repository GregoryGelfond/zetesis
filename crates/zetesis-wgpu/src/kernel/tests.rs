use super::*;

#[test]
fn kernel_width_must_fit_both_granted_limits() {
    let kernel = Kernel::Formula(GpuKernelOptions::default());
    for (width, invocations) in [(32, 64), (64, 32)] {
        let limits = wgpu::Limits {
            max_compute_workgroup_size_x: width,
            max_compute_invocations_per_workgroup: invocations,
            ..Default::default()
        };
        assert_eq!(
            kernel.validate(&limits).unwrap_err().kind(),
            GpuErrorKind::Capacity
        );
    }
    assert!(kernel.validate(&wgpu::Limits::default()).is_ok());
}

#[test]
fn missing_authored_fragment_is_a_typed_failure() {
    let error = Kernel::Formula(GpuKernelOptions::default())
        .source(Cow::Borrowed(""), wgpu::Features::SUBGROUP)
        .unwrap_err();
    assert_eq!(error.kind(), GpuErrorKind::Validation);
}

#[test]
fn tight_kernel_refuses_a_general_propagation_request() {
    for propagation in [
        GpuPropagation::ChangedDependencies,
        GpuPropagation::IncrementalGroups,
    ] {
        let kernel = Kernel::Tight(GpuKernelOptions {
            propagation,
            ..GpuKernelOptions::default()
        });
        assert_eq!(
            kernel
                .validate(&wgpu::Limits::default())
                .unwrap_err()
                .kind(),
            GpuErrorKind::Validation
        );
    }
}
