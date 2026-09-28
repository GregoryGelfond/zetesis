//! Explicit device-test requests, independent of discovery and reported metadata.

use zetesis_backend::GpuApi;

/// The backend request and argument that name a physical device's API.
pub trait Physical: Copy {
    fn requested(self) -> zetesis_cli::Backend;
    fn argument(self) -> &'static str;
}

impl Physical for GpuApi {
    fn requested(self) -> zetesis_cli::Backend {
        zetesis_cli::Backend::Gpu(Some(self))
    }

    fn argument(self) -> &'static str {
        self.requested().label()
    }
}
