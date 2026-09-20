//! Supplied contexts obey discovery's hard identity policy without rediscovery.

use super::{GpuBackendPreference, GpuInfo, GpuSelection, check_selection};
use crate::{GpuErrorKind, GpuOptions};

fn reported(backend: wgpu::Backend, category: wgpu::DeviceType, vendor: u32) -> GpuInfo {
    let mut raw = wgpu::AdapterInfo::new(category, backend);
    raw.name = "NVIDIA is only a reported name".into();
    raw.vendor = vendor;
    GpuInfo::from_report(raw, wgpu::Features::empty(), &wgpu::Limits::default(), true)
}

#[test]
fn context_policy_requires_the_requested_api() {
    let native = [
        (GpuBackendPreference::Metal, wgpu::Backend::Metal),
        (GpuBackendPreference::Vulkan, wgpu::Backend::Vulkan),
        (GpuBackendPreference::Dx12, wgpu::Backend::Dx12),
        (GpuBackendPreference::Gl, wgpu::Backend::Gl),
    ];
    for (required, expected) in native {
        for (_, actual) in native {
            let info = reported(actual, wgpu::DeviceType::DiscreteGpu, 0);
            let result = check_selection(
                &info,
                GpuOptions::default(),
                GpuSelection {
                    backend: required,
                    vendor_id: None,
                },
            );
            if actual == expected {
                result.unwrap();
            } else {
                assert_eq!(result.unwrap_err().kind(), GpuErrorKind::AdapterRefused);
            }
        }
    }
}

#[test]
fn context_auto_policy_accepts_only_native_apis() {
    for backend in [
        wgpu::Backend::Metal,
        wgpu::Backend::Vulkan,
        wgpu::Backend::Dx12,
        wgpu::Backend::Gl,
    ] {
        check_selection(
            &reported(backend, wgpu::DeviceType::DiscreteGpu, 0),
            GpuOptions::default(),
            GpuSelection::default(),
        )
        .unwrap();
    }
    for backend in [wgpu::Backend::Noop, wgpu::Backend::BrowserWebGpu] {
        assert_eq!(
            check_selection(
                &reported(backend, wgpu::DeviceType::DiscreteGpu, 0),
                GpuOptions::default(),
                GpuSelection::default(),
            )
            .unwrap_err()
            .kind(),
            GpuErrorKind::AdapterRefused,
        );
    }
}

#[test]
fn context_policy_matches_the_numeric_vendor() {
    let info = reported(wgpu::Backend::Vulkan, wgpu::DeviceType::DiscreteGpu, 0x1002);
    for vendor_id in [None, Some(0x1002)] {
        check_selection(
            &info,
            GpuOptions::default(),
            GpuSelection {
                vendor_id,
                ..GpuSelection::default()
            },
        )
        .unwrap();
    }
    let failure = check_selection(
        &info,
        GpuOptions::default(),
        GpuSelection {
            vendor_id: Some(super::NVIDIA_VENDOR_ID),
            ..GpuSelection::default()
        },
    )
    .unwrap_err();
    assert_eq!(failure.kind(), GpuErrorKind::AdapterRefused);
    assert!(failure.detail().contains("vendor_id=0x1002"));
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
            GpuSelection::default(),
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
                GpuSelection::default(),
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
            GpuSelection::default(),
        )
        .unwrap();
    }
}

#[test]
fn context_policy_leaves_profile_admission_separate() {
    let mut info = reported(wgpu::Backend::Metal, wgpu::DeviceType::IntegratedGpu, 0);
    info.capability_issue = Some("synthetic profile refusal".into());
    check_selection(&info, GpuOptions::default(), GpuSelection::default()).unwrap();
    assert_eq!(info.capability_issue(), Some("synthetic profile refusal"));
}
