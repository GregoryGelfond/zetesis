//! Reported identities are preserved; metadata never proves GPU execution.

use std::cmp::Ordering;

use super::{GpuBackendPreference, GpuInfo, GpuSelection, HostPlatform, choose, compare_info};
use crate::{GpuErrorKind, GpuOptions};

fn info(backend: wgpu::Backend, category: wgpu::DeviceType) -> GpuInfo {
    let mut raw = wgpu::AdapterInfo::new(category, backend);
    raw.name = "reported name".into();
    raw.driver = "reported driver".into();
    raw.driver_info = "reported version".into();
    raw.vendor = 0x10de;
    raw.device = 0x1234;
    raw.device_pci_bus_id = "0000:01:00.0".into();
    GpuInfo::from_report(raw, &wgpu::Limits::default(), true)
}

#[test]
fn every_reported_api_and_category_keeps_its_identity_and_hardware_boundary() {
    for (backend, label) in [
        (wgpu::Backend::Noop, "Noop"),
        (wgpu::Backend::Vulkan, "Vulkan"),
        (wgpu::Backend::Metal, "Metal"),
        (wgpu::Backend::Dx12, "Dx12"),
        (wgpu::Backend::Gl, "Gl"),
        (wgpu::Backend::BrowserWebGpu, "BrowserWebGpu"),
    ] {
        for (category, category_label, hardware) in [
            (wgpu::DeviceType::Other, "Other", false),
            (wgpu::DeviceType::IntegratedGpu, "IntegratedGpu", true),
            (wgpu::DeviceType::DiscreteGpu, "DiscreteGpu", true),
            (wgpu::DeviceType::VirtualGpu, "VirtualGpu", false),
            (wgpu::DeviceType::Cpu, "Cpu", false),
        ] {
            let info = info(backend, category);
            assert_eq!(info.backend(), label);
            assert_eq!(info.device_type(), category_label);
            assert_eq!(info.is_hardware_gpu(), hardware);
            assert_eq!(info.name(), "reported name");
            assert_eq!(info.driver(), "reported driver");
            assert_eq!(info.driver_info(), "reported version");
            assert_eq!(info.vendor_id(), 0x10de);
            assert_eq!(info.device_id(), 0x1234);
            assert_eq!(info.pci_bus_id(), "0000:01:00.0");
            assert!(info.supports_static_oracle());
            assert!(info.capability_issue().is_none());
        }
    }
}

#[test]
fn each_reported_identity_field_breaks_ties_independently_of_input_order() {
    let first = info(wgpu::Backend::Vulkan, wgpu::DeviceType::DiscreteGpu);
    let mut later = Vec::new();
    for field in 0..4 {
        let mut next = first.clone();
        match field {
            0 => next.raw.device_pci_bus_id.push('z'),
            1 => next.raw.name.push('z'),
            2 => next.raw.driver.push('z'),
            _ => next.raw.driver_info.push('z'),
        }
        later.push(next);
    }
    for other in later {
        assert_eq!(
            compare_info(&first, &other, HostPlatform::Other),
            Ordering::Less
        );
        assert_eq!(
            compare_info(&other, &first, HostPlatform::Other),
            Ordering::Greater
        );
        for inventory in [[&first, &other], [&other, &first]] {
            let selected = choose(
                inventory.into_iter().enumerate(),
                GpuOptions::default(),
                GpuSelection::default(),
                HostPlatform::Other,
            )
            .unwrap();
            assert_eq!(inventory[selected], &first);
        }
    }
    assert_eq!(
        compare_info(&first, &first, HostPlatform::Other),
        Ordering::Equal
    );
}

#[test]
fn software_categories_have_stable_order_only_when_explicitly_admitted() {
    let categories = [
        wgpu::DeviceType::VirtualGpu,
        wgpu::DeviceType::Other,
        wgpu::DeviceType::Cpu,
    ];
    let reports = categories.map(|category| info(wgpu::Backend::Vulkan, category));
    for pair in reports.windows(2) {
        assert_eq!(
            compare_info(&pair[0], &pair[1], HostPlatform::Other),
            Ordering::Less
        );
    }
    assert_eq!(
        choose(
            reports.iter().enumerate(),
            GpuOptions::default(),
            GpuSelection::default(),
            HostPlatform::Other
        )
        .unwrap_err()
        .kind(),
        GpuErrorKind::AdapterRefused
    );
    assert_eq!(
        choose(
            reports.iter().enumerate(),
            GpuOptions { require_gpu: false },
            GpuSelection::default(),
            HostPlatform::Other
        )
        .unwrap(),
        0
    );
    for platform in [
        HostPlatform::Apple,
        HostPlatform::Windows,
        HostPlatform::Other,
    ] {
        assert!(
            platform.backend_rank(wgpu::Backend::Gl)
                < platform.backend_rank(wgpu::Backend::BrowserWebGpu)
        );
        assert!(
            platform.backend_rank(wgpu::Backend::BrowserWebGpu)
                < platform.backend_rank(wgpu::Backend::Noop)
        );
    }
}

#[test]
fn compiled_api_inventory_contains_only_enabled_explicit_native_apis() {
    let actual = super::compiled_backends();
    let enabled = wgpu::Instance::enabled_backend_features();
    for backend in [
        GpuBackendPreference::Metal,
        GpuBackendPreference::Vulkan,
        GpuBackendPreference::Dx12,
        GpuBackendPreference::Gl,
    ] {
        assert_eq!(
            actual.contains(&backend),
            enabled.intersects(backend.backends())
        );
    }
    assert!(!actual.contains(&GpuBackendPreference::Auto));
    let unique: std::collections::BTreeSet<_> = actual.iter().map(ToString::to_string).collect();
    assert_eq!(actual.len(), unique.len());
}
