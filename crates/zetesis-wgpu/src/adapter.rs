//! Typed views of reported adapter metadata, independent of selection policy.

use std::fmt;

/// An observed compute API. Unlike a preference, an observation cannot be Auto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterBackend {
    /// The nonexecuting test backend.
    Noop,
    /// Vulkan.
    Vulkan,
    /// Metal.
    Metal,
    /// Direct3D 12.
    Dx12,
    /// OpenGL or OpenGL ES.
    Gl,
    /// Browser WebGPU.
    BrowserWebGpu,
}
impl AdapterBackend {
    /// Stable legacy display label; classification does not require parsing it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Noop => "Noop",
            Self::Vulkan => "Vulkan",
            Self::Metal => "Metal",
            Self::Dx12 => "Dx12",
            Self::Gl => "Gl",
            Self::BrowserWebGpu => "BrowserWebGpu",
        }
    }
}
impl fmt::Display for AdapterBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

/// The device category reported by the backend, not an execution qualification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterCategory {
    /// The backend did not provide a more specific category.
    Other,
    /// An integrated GPU.
    IntegratedGpu,
    /// A discrete GPU.
    DiscreteGpu,
    /// A virtual GPU.
    VirtualGpu,
    /// A CPU adapter.
    Cpu,
}
impl AdapterCategory {
    /// Stable legacy display label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Other => "Other",
            Self::IntegratedGpu => "IntegratedGpu",
            Self::DiscreteGpu => "DiscreteGpu",
            Self::VirtualGpu => "VirtualGpu",
            Self::Cpu => "Cpu",
        }
    }
}
impl fmt::Display for AdapterCategory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

/// A borrowed data view of an adapter's reported metadata.
///
/// Empty optional text means absence in the backend contract and becomes None.
/// Numeric IDs retain their exact reported values; zero is not universally
/// interpreted as absence. These fields do not certify device execution or a
/// unique physical device. Constructing/copying this view performs no discovery
/// or allocation and has fixed cost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdapterMetadata<'a> {
    /// Reported adapter name, without normalization.
    pub name: &'a str,
    /// Observed API, independent of the requested selection policy.
    pub backend: AdapterBackend,
    /// Reported device category.
    pub category: AdapterCategory,
    /// Exact backend-reported vendor ID; zero can mean unknown.
    pub vendor_id: u32,
    /// Exact backend-reported device ID; not a physical-device identifier.
    pub device_id: u32,
    /// PCI bus identifier where reported.
    pub pci_bus_id: Option<&'a str>,
    /// Driver name where reported.
    pub driver: Option<&'a str>,
    /// Driver version/details where reported.
    pub driver_info: Option<&'a str>,
}
