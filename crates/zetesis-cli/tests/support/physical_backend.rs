//! Explicit device-test requests, independent of discovery and reported metadata.

#[derive(Clone, Copy)]
pub enum Backend {
    Metal,
    Vulkan,
}

impl Backend {
    pub fn requested(self) -> zetesis_cli::Backend {
        match self {
            Self::Metal => zetesis_cli::Backend::Metal,
            Self::Vulkan => zetesis_cli::Backend::Vulkan,
        }
    }

    pub fn argument(self) -> &'static str {
        self.requested().label()
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Metal => "Metal",
            Self::Vulkan => "Vulkan",
        }
    }
}
