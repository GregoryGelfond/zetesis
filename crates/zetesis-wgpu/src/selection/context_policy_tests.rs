//! Supplied contexts obey discovery's hard identity policy without rediscovery.

use super::test_support::{METAL, VULKAN};
use super::{GpuInfo, check_selection};
use crate::{GpuErrorKind, GpuOptions};

fn reported(backend: wgpu::Backend, category: wgpu::DeviceType, vendor: u32) -> GpuInfo {
    let mut raw = wgpu::AdapterInfo::new(category, backend);
    raw.name = "NVIDIA is only a reported name".into();
    raw.vendor = vendor;
    GpuInfo::from_report(raw, wgpu::Features::empty(), &wgpu::Limits::default(), true)
}

#[test]
fn context_policy_requires_the_requested_api() {
    for (required, expected) in [
        (METAL, wgpu::Backend::Metal),
        (VULKAN, wgpu::Backend::Vulkan),
    ] {
        for actual in [
            wgpu::Backend::Metal,
            wgpu::Backend::Vulkan,
            wgpu::Backend::Dx12,
            wgpu::Backend::Gl,
        ] {
            let info = reported(actual, wgpu::DeviceType::DiscreteGpu, 0);
            let result = check_selection(&info, GpuOptions::default(), required);
            if actual == expected {
                result.unwrap();
            } else {
                assert_eq!(result.unwrap_err().kind(), GpuErrorKind::AdapterRefused);
            }
        }
    }
}

#[test]
fn context_policy_refuses_nonexecuting_backends() {
    for backend in [wgpu::Backend::Noop, wgpu::Backend::BrowserWebGpu] {
        for selection in [METAL, VULKAN] {
            assert_eq!(
                check_selection(
                    &reported(backend, wgpu::DeviceType::DiscreteGpu, 0),
                    GpuOptions::default(),
                    selection,
                )
                .unwrap_err()
                .kind(),
                GpuErrorKind::AdapterRefused,
            );
        }
    }
}

#[test]
fn context_policy_requires_physical_categories() {
    for category in [
        wgpu::DeviceType::IntegratedGpu,
        wgpu::DeviceType::DiscreteGpu,
    ] {
        check_selection(
            &reported(wgpu::Backend::Vulkan, category, 0),
            GpuOptions::default(),
            VULKAN,
        )
        .unwrap();
    }
    for category in [
        wgpu::DeviceType::Cpu,
        wgpu::DeviceType::VirtualGpu,
        wgpu::DeviceType::Other,
    ] {
        assert_eq!(
            check_selection(
                &reported(wgpu::Backend::Vulkan, category, 0),
                GpuOptions::default(),
                VULKAN,
            )
            .unwrap_err()
            .kind(),
            GpuErrorKind::AdapterRefused,
        );
    }
}

#[test]
fn context_policy_can_explicitly_admit_software() {
    for category in [
        wgpu::DeviceType::Cpu,
        wgpu::DeviceType::VirtualGpu,
        wgpu::DeviceType::Other,
    ] {
        check_selection(
            &reported(wgpu::Backend::Vulkan, category, 0),
            GpuOptions { require_gpu: false },
            VULKAN,
        )
        .unwrap();
    }
}

#[test]
fn context_policy_leaves_profile_admission_separate() {
    let mut info = reported(wgpu::Backend::Metal, wgpu::DeviceType::IntegratedGpu, 0);
    info.capability_issue = Some("synthetic profile refusal".into());
    check_selection(&info, GpuOptions::default(), METAL).unwrap();
    assert_eq!(info.capability_issue(), Some("synthetic profile refusal"));
}
