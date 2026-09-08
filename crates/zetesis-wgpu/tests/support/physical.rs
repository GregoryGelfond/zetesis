//! Explicit test selection; no environment override or adapter-absence skip.

use zetesis_wgpu::{GpuBackendPreference, GpuInfo, GpuSelection};

#[derive(Clone, Copy)]
pub enum Backend {
    Metal,
    Vulkan,
}

impl Backend {
    pub fn selection(self) -> GpuSelection {
        GpuSelection {
            backend: match self {
                Self::Metal => GpuBackendPreference::Metal,
                Self::Vulkan => GpuBackendPreference::Vulkan,
            },
            vendor_id: None,
        }
    }

    pub fn verify(self, info: &GpuInfo) {
        assert_eq!(
            info.backend(),
            match self {
                Self::Metal => "Metal",
                Self::Vulkan => "Vulkan",
            }
        );
        assert!(info.is_hardware_gpu());
        eprintln!("physical adapter={:?}", info.metadata());
    }
}
