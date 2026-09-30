//! Reported identities are preserved; metadata never proves GPU execution.

use std::cmp::Ordering;

use super::reported_static_admission;
use super::test_support::VULKAN;
use super::{GpuInfo, backends, choose, compare_info};
use crate::{GpuErrorKind, GpuOptions};
use zetesis_backend::GpuApi;

fn info(backend: wgpu::Backend, category: wgpu::DeviceType) -> GpuInfo {
    let mut raw = wgpu::AdapterInfo::new(category, backend);
    raw.name = "reported name".into();
    raw.driver = "reported driver".into();
    raw.driver_info = "reported version".into();
    raw.vendor = 0x10de;
    raw.device = 0x1234;
    raw.device_pci_bus_id = "0000:01:00.0".into();
    GpuInfo::from_report(raw, wgpu::Features::empty(), &wgpu::Limits::default(), true)
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
        assert_eq!(compare_info(&first, &other), Ordering::Less);
        assert_eq!(compare_info(&other, &first), Ordering::Greater);
        for inventory in [[&first, &other], [&other, &first]] {
            let selected = choose(
                inventory.into_iter().enumerate(),
                GpuOptions::default(),
                VULKAN,
                reported_static_admission,
            )
            .unwrap();
            assert_eq!(inventory[selected], &first);
        }
    }
    assert_eq!(compare_info(&first, &first), Ordering::Equal);
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
        assert_eq!(compare_info(&pair[0], &pair[1]), Ordering::Less);
    }
    assert_eq!(
        choose(
            reports.iter().enumerate(),
            GpuOptions::default(),
            VULKAN,
            reported_static_admission,
        )
        .unwrap_err()
        .kind(),
        GpuErrorKind::AdapterRefused
    );
    assert_eq!(
        choose(
            reports.iter().enumerate(),
            GpuOptions { require_gpu: false },
            VULKAN,
            reported_static_admission,
        )
        .unwrap(),
        0
    );
}

#[test]
fn the_compiled_api_inventory_lists_each_enabled_targeted_api_once() {
    let actual = super::compiled_apis();
    let enabled = wgpu::Instance::enabled_backend_features();
    for api in [GpuApi::Metal, GpuApi::Vulkan] {
        assert_eq!(actual.contains(&api), enabled.intersects(backends(api)));
    }
    let unique: std::collections::BTreeSet<_> = actual.iter().map(|api| api.label()).collect();
    assert_eq!(actual.len(), unique.len());
}
