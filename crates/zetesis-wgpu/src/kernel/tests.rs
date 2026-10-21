use super::*;

#[test]
fn the_base_workgroup_size_must_fit_both_granted_limits() {
    let kernel = Kernel::Formula(GpuKernelOptions::default());
    for (size_x, invocations) in [(32, 64), (64, 32)] {
        let limits = wgpu::Limits {
            max_compute_workgroup_size_x: size_x,
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

fn granted(invocations: u32, size_x: u32) -> wgpu::Limits {
    wgpu::Limits {
        max_compute_invocations_per_workgroup: invocations,
        max_compute_workgroup_size_x: size_x,
        ..wgpu::Limits::default()
    }
}

fn fitted_size(levels: &[u32], limits: &wgpu::Limits) -> GpuWorkgroupSize {
    dispatch_size(GpuWorkgroupSizing::Fitted, levels.iter().copied(), limits)
}

#[test]
fn a_serial_schedule_keeps_the_fitted_base() {
    assert_eq!(
        fitted_size(&[], &granted(1024, 1024)),
        GpuWorkgroupSize::Threads64
    );
}

#[test]
fn levels_of_at_most_64_nodes_keep_the_fitted_base() {
    // No larger size shortens a level of at most 64 nodes.
    for levels in [vec![64; 10], vec![8; 40], vec![1; 900]] {
        assert_eq!(
            fitted_size(&levels, &granted(1024, 1024)),
            GpuWorkgroupSize::Threads64
        );
    }
}

#[test]
fn wide_levels_take_the_least_strided_depth() {
    assert_eq!(
        fitted_size(&[1024; 10], &granted(1024, 1024)),
        GpuWorkgroupSize::Threads256
    );
}

#[test]
fn the_bound_excludes_sizes_beyond_twice_the_base_invocation_slots() {
    // One 200-node level among a hundred single nodes: depths are 104, 102
    // and 101 at 64, 128 and 256. The base bound is 2 * 64 * 104 = 13,312
    // invocation slots; 128 uses 13,056 and 256 would use 25,856.
    let mut levels = vec![1; 100];
    levels.push(200);
    assert_eq!(
        fitted_size(&levels, &granted(1024, 1024)),
        GpuWorkgroupSize::Threads128
    );
}

#[test]
fn equal_depth_prefers_the_smaller_size() {
    // A widest level of 65 to 128 nodes needs two strides at 64 and one at
    // 128 or 256, as does every narrower level: 128 and 256 tie.
    for widest in [65, 96, 128] {
        assert_eq!(
            fitted_size(&[widest, 1, 64, widest], &granted(1024, 1024)),
            GpuWorkgroupSize::Threads128,
            "widest level {widest}"
        );
    }
}

#[test]
fn the_bound_admits_exactly_twice_the_base_invocation_slots() {
    // Level widths 129 and 1: depths 4, 3 and 2 at 64, 128 and 256. At 256
    // the slots are 256 * 2 = 512, exactly 2 * 64 * 4, which the bound admits.
    assert_eq!(
        fitted_size(&[129, 1], &granted(1024, 1024)),
        GpuWorkgroupSize::Threads256
    );
}

#[test]
fn the_fitted_rule_selects_among_64_128_and_256() {
    assert_eq!(FITTED.map(GpuWorkgroupSize::invocations), [64, 128, 256]);
}

#[test]
fn granted_limits_exclude_larger_sizes() {
    assert_eq!(
        fitted_size(&[4096; 4], &granted(128, 1024)),
        GpuWorkgroupSize::Threads128
    );
    assert_eq!(
        fitted_size(&[4096; 4], &granted(1024, 64)),
        GpuWorkgroupSize::Threads64
    );
}

#[test]
fn a_fixed_size_ignores_the_schedule() {
    for size in GpuWorkgroupSize::ALL {
        assert_eq!(
            dispatch_size(
                GpuWorkgroupSizing::Fixed(size),
                [4096_u32; 4].into_iter(),
                &granted(1024, 1024)
            ),
            size
        );
    }
}

#[test]
fn a_fitted_kernel_compiles_every_granted_size_from_its_base() {
    let sizes: Vec<_> = candidate_sizes(GpuWorkgroupSizing::Fitted, &granted(1024, 1024)).collect();
    assert_eq!(
        sizes,
        [
            GpuWorkgroupSize::Threads64,
            GpuWorkgroupSize::Threads128,
            GpuWorkgroupSize::Threads256
        ]
    );
}

#[test]
fn strided_loops_keep_the_largest_compiled_size_of_headroom() {
    assert_eq!(
        MAX_FORMULA_STRIDE,
        GpuWorkgroupSize::Threads256.invocations()
    );
    assert!(fits_strided_loop(u32::MAX - 256));
    assert!(!fits_strided_loop(u32::MAX - 255));
}

/// Every level-width sequence of at most three levels drawn from widths around
/// each size boundary, the serial schedule included.
fn boundary_schedules() -> Vec<Vec<u32>> {
    const WIDTHS: [u32; 11] = [1, 63, 64, 65, 127, 128, 129, 255, 256, 257, 640];
    let mut schedules = vec![Vec::new()];
    let mut previous = vec![Vec::new()];
    for _ in 0..3 {
        previous = previous
            .iter()
            .flat_map(|levels: &Vec<u32>| {
                WIDTHS.into_iter().map(move |width| {
                    let mut longer = levels.clone();
                    longer.push(width);
                    longer
                })
            })
            .collect();
        schedules.extend(previous.iter().cloned());
    }
    schedules
}

fn strided_depth(widths: &[u32], size: GpuWorkgroupSize) -> u64 {
    widths
        .iter()
        .map(|width| u64::from(width.div_ceil(size.invocations())))
        .sum()
}

#[test]
fn every_fitted_selection_meets_the_rule_postcondition() {
    // The selection is checked against the rule's postcondition, not against
    // a second implementation of the selection: a granted fitted size within
    // the bound, never deeper than 64, of least depth among the granted sizes
    // within the bound, and the smallest of those on a tie.
    for granted_size in [64, 128, 256, 1024] {
        let limits = granted(granted_size, granted_size);
        let admitted: Vec<_> = FITTED
            .into_iter()
            .filter(|size| size.invocations() <= granted_size)
            .collect();
        for widths in boundary_schedules() {
            let selected = fitted_size(&widths, &limits);
            let depth = |size| strided_depth(&widths, size);
            let base = depth(GpuWorkgroupSize::Threads64);
            let within = |size: GpuWorkgroupSize| {
                u64::from(size.invocations()) * depth(size) <= 2 * 64 * base
            };
            let context = format!("{widths:?} granting {granted_size}");
            assert!(admitted.contains(&selected), "{context}");
            assert!(within(selected), "{context}");
            assert!(depth(selected) <= base, "{context}");
            for other in admitted.iter().copied().filter(|size| within(*size)) {
                assert!(depth(selected) <= depth(other), "{context}");
                if other.invocations() < selected.invocations() {
                    assert!(depth(other) > depth(selected), "{context}");
                }
            }
        }
    }
}
