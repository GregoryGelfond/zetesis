//! Optional metadata is a borrowed view; raw reported values remain available.

use super::GpuInfo;
use crate::{AdapterBackend, AdapterCategory};

fn info() -> GpuInfo {
    GpuInfo::from_report(
        wgpu::AdapterInfo::new(wgpu::DeviceType::IntegratedGpu, wgpu::Backend::Metal),
        wgpu::Features::empty(),
        &wgpu::Limits::default(),
        true,
    )
}

#[test]
fn advertised_features_preserve_the_backend_report() {
    let features = wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::SUBGROUP;
    let report = GpuInfo::from_report(
        wgpu::AdapterInfo::new(wgpu::DeviceType::IntegratedGpu, wgpu::Backend::Metal),
        features,
        &wgpu::Limits::default(),
        true,
    );
    assert_eq!(report.features(), features);
}

#[test]
fn absent_adapter_text_is_none() {
    let info = info();
    let metadata = info.metadata();
    assert_eq!(metadata.pci_bus_id, None);
    assert_eq!(metadata.driver, None);
    assert_eq!(metadata.driver_info, None);
}

#[test]
fn adapter_text_borrows_reported_storage() {
    let mut info = info();
    info.raw.device_pci_bus_id = "0000:01:00.0".into();
    info.raw.driver = "reported driver".into();
    info.raw.driver_info = "reported version".into();
    let metadata = info.metadata();
    for (typed, raw) in [
        (metadata.pci_bus_id, info.pci_bus_id()),
        (metadata.driver, info.driver()),
        (metadata.driver_info, info.driver_info()),
    ] {
        assert_eq!(typed, Some(raw));
        assert!(std::ptr::eq(typed.unwrap(), raw));
    }
}

#[test]
fn zero_identifiers_remain_reported_values() {
    let mut info = info();
    info.raw.vendor = 0;
    info.raw.device = 0;
    let metadata = info.metadata();
    assert_eq!(metadata.vendor_id, info.vendor_id());
    assert_eq!(metadata.device_id, info.device_id());
}

#[test]
fn adapter_backend_is_typed() {
    let info = info();
    let metadata = info.metadata();
    assert_eq!(metadata.backend, AdapterBackend::Metal);
    assert_eq!(metadata.backend.to_string(), info.backend());
}

#[test]
fn adapter_category_is_typed() {
    let info = info();
    let metadata = info.metadata();
    assert_eq!(metadata.category, AdapterCategory::IntegratedGpu);
    assert_eq!(metadata.category.to_string(), info.device_type());
}
