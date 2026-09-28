//! The experiments' one application of the backend vocabulary's resolution
//! rule, kept distinct from observed adapter metadata.

use zetesis_backend::{Backend, GpuApi};
use zetesis_wgpu::GpuSelection;

/// The GPU API an experiment runs on: the named API, or the platform's native
/// one for `gpu`. `None` measures CPU baselines only, making no GPU execution
/// claim. A GPU experiment never falls back to the CPU or another API.
pub(crate) const fn api(backend: Backend) -> Option<GpuApi> {
    backend.resolved_api()
}

/// The adapter filter a GPU experiment opens with; `None` for CPU baselines.
pub(crate) const fn selection(backend: Backend) -> Option<GpuSelection> {
    match api(backend) {
        Some(api) => Some(GpuSelection { api }),
        None => None,
    }
}

/// The spelling reports record: what the experiment ran on.
pub(crate) const fn label(backend: Backend) -> &'static str {
    match api(backend) {
        Some(api) => api.label(),
        None => "cpu",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_named_api_is_opened_as_requested() {
        for api in [GpuApi::Metal, GpuApi::Vulkan] {
            let backend = Backend::Gpu(Some(api));
            assert_eq!(selection(backend), Some(GpuSelection { api }));
            assert_eq!(label(backend), api.label());
        }
    }

    #[test]
    fn a_gpu_request_runs_on_the_native_api() {
        let backend = Backend::Gpu(None);
        assert_eq!(selection(backend), Some(GpuSelection::default()));
        assert_eq!(label(backend), GpuApi::native().label());
    }

    #[test]
    fn cpu_measurement_requests_no_device() {
        assert_eq!(selection(Backend::Cpu), None);
        assert_eq!(label(Backend::Cpu), "cpu");
    }
}
