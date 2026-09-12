//! Select against the requested profile, then rank; static discovery is metadata.

use super::{GpuInfo, GpuSelection, HostPlatform, check_capabilities, choose};
use crate::{GpuError, GpuErrorKind, GpuOptions};

fn report(name: &str, limits: &wgpu::Limits, compute: bool) -> GpuInfo {
    let mut raw = wgpu::AdapterInfo::new(wgpu::DeviceType::IntegratedGpu, wgpu::Backend::Metal);
    raw.name = name.into();
    GpuInfo::from_report(raw, limits, compute)
}

// Small synthetic profiles isolate the shared selector's callback contract.
// Each primitive's shader/resource tests separately establish its real limits.
fn four_buffers(limits: &wgpu::Limits) -> Result<(), GpuError> {
    bindings(limits, 4)
}

fn seven_buffers(limits: &wgpu::Limits) -> Result<(), GpuError> {
    bindings(limits, 7)
}

fn bindings(limits: &wgpu::Limits, count: u32) -> Result<(), GpuError> {
    if limits.max_storage_buffers_per_shader_stage < count {
        Err(GpuError::new(GpuErrorKind::Capacity, "storage buffers"))
    } else {
        Ok(())
    }
}

#[test]
fn weaker_profile_admission_does_not_change_static_discovery() {
    let limits = wgpu::Limits {
        max_storage_buffers_per_shader_stage: 4,
        ..wgpu::Limits::default()
    };
    let info = report("four bindings", &limits, true);
    assert!(!info.supports_static_oracle());
    assert_eq!(
        choose(
            [(0, &info)],
            GpuOptions::default(),
            GpuSelection::default(),
            HostPlatform::Apple,
            |_, _| check_capabilities(true, &limits, four_buffers),
        )
        .unwrap(),
        0
    );
    assert!(!info.supports_static_oracle());
    assert!(
        info.capability_issue()
            .unwrap()
            .contains("storage bindings")
    );
}

#[test]
fn stronger_profile_skips_the_first_ranked_incapable_adapter() {
    let five = wgpu::Limits {
        max_storage_buffers_per_shader_stage: 5,
        ..wgpu::Limits::default()
    };
    let seven = wgpu::Limits {
        max_storage_buffers_per_shader_stage: 7,
        ..five.clone()
    };
    let first = report("a first", &five, true);
    let later = report("b later", &seven, true);
    assert!(first.supports_static_oracle());
    for inventory in [
        [(&first, &five), (&later, &seven)],
        [(&later, &seven), (&first, &five)],
    ] {
        let selected = choose(
            inventory
                .iter()
                .enumerate()
                .map(|(i, (info, _))| (i, *info)),
            GpuOptions::default(),
            GpuSelection::default(),
            HostPlatform::Apple,
            |index, _| check_capabilities(true, inventory[index].1, seven_buffers),
        )
        .unwrap();
        assert_eq!(inventory[selected].0.name(), "b later");
    }
}

#[test]
fn profile_free_context_selection_still_requires_compute() {
    let limits = wgpu::Limits {
        max_storage_buffers_per_shader_stage: 0,
        max_compute_workgroup_storage_size: 0,
        ..wgpu::Limits::default()
    };
    let info = report("generic context", &limits, true);
    assert!(!info.supports_static_oracle());
    for compute in [false, true] {
        let selected = choose(
            [(0, &info)],
            GpuOptions::default(),
            GpuSelection::default(),
            HostPlatform::Apple,
            |_, _| check_capabilities(compute, &limits, |_| Ok(())),
        );
        if compute {
            assert_eq!(selected.unwrap(), 0);
        } else {
            assert_eq!(selected.unwrap_err().kind(), GpuErrorKind::Capacity);
        }
    }
}

#[test]
fn hard_policy_refusal_precedes_primitive_validation() {
    let info = report("foreign vendor", &wgpu::Limits::default(), true);
    let error = choose(
        [(0, &info)],
        GpuOptions::default(),
        GpuSelection {
            vendor_id: Some(0x10de),
            ..GpuSelection::default()
        },
        HostPlatform::Apple,
        |_, _| panic!("hard-refused adapters do not enter primitive admission"),
    )
    .unwrap_err();
    assert_eq!(error.kind(), GpuErrorKind::AdapterRefused);
}

#[test]
fn unexpected_validation_failure_is_not_a_capability_fallback() {
    let info = report("reported", &wgpu::Limits::default(), true);
    let error = choose(
        [(0, &info)],
        GpuOptions::default(),
        GpuSelection::default(),
        HostPlatform::Apple,
        |_, _| Err(GpuError::new(GpuErrorKind::Device, "original failure")),
    )
    .unwrap_err();
    assert_eq!(error.kind(), GpuErrorKind::Device);
    assert_eq!(error.detail(), "original failure");
}
