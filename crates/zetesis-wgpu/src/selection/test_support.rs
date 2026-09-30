//! The selections the selection module's unit tests share.

use zetesis_backend::GpuApi;

use super::GpuSelection;

/// The Metal selection.
pub(super) const METAL: GpuSelection = GpuSelection { api: GpuApi::Metal };

/// The Vulkan selection.
pub(super) const VULKAN: GpuSelection = GpuSelection {
    api: GpuApi::Vulkan,
};
