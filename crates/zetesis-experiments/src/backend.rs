//! Requested physical API, kept distinct from observed adapter metadata.

use clap::ValueEnum;
use zetesis_wgpu::{GpuBackendPreference, GpuSelection};

/// Explicit experiment execution. Physical selection never falls back to CPU
/// or another graphics API. The historical Metal default remains unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Backend {
    /// Require a physical GPU through Metal.
    #[default]
    Metal,
    /// Require a physical GPU through Vulkan.
    Vulkan,
    /// Measure CPU baselines only, making no GPU execution claim.
    Cpu,
}

impl Backend {
    pub(crate) const fn selection(self) -> Option<GpuSelection> {
        let backend = match self {
            Self::Cpu => return None,
            Self::Metal => GpuBackendPreference::Metal,
            Self::Vulkan => GpuBackendPreference::Vulkan,
        };
        Some(GpuSelection {
            backend,
            vendor_id: None,
        })
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Metal => "metal",
            Self::Vulkan => "vulkan",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requested_apis_never_map_to_automatic_selection() {
        for (backend, expected) in [
            (Backend::Metal, GpuBackendPreference::Metal),
            (Backend::Vulkan, GpuBackendPreference::Vulkan),
        ] {
            let selection = backend.selection().unwrap();
            assert_eq!(selection.backend, expected);
            assert_eq!(selection.vendor_id, None);
        }
    }

    #[test]
    fn cpu_measurement_requests_no_device() {
        assert!(Backend::Cpu.selection().is_none());
    }
}
