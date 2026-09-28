//! Explicit test selection; no environment override or adapter-absence skip.

use zetesis_backend::GpuApi;
use zetesis_wgpu::GpuInfo;

/// Asserts that `info` reports a physical adapter driven through `api`.
pub fn verify(api: GpuApi, info: &GpuInfo) {
    assert_eq!(info.backend(), api.name());
    assert!(info.is_hardware_gpu());
    eprintln!("physical adapter={:?}", info.metadata());
}
