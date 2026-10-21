use super::ExecutionResources;
use crate::{Backend, SolveConfig};
use zetesis_backend::GpuApi;
use zetesis_wgpu::GpuDeviceSource;

fn config(backend: Backend, stats: bool) -> SolveConfig {
    SolveConfig {
        backend,
        stats,
        ..SolveConfig::DEFAULT
    }
}

const METAL: Backend = Backend::Gpu(Some(GpuApi::Metal));
const VULKAN: Backend = Backend::Gpu(Some(GpuApi::Vulkan));

#[test]
fn a_cpu_session_starts_no_discovery() {
    let resources = ExecutionResources::discovering(&config(Backend::Cpu, true));
    for backend in [METAL, VULKAN] {
        assert!(matches!(
            resources.device_source(&config(backend, true)),
            Ok(GpuDeviceSource::Selected { .. })
        ));
    }
}

#[test]
fn a_gpu_session_selects_over_its_own_discovery() {
    let resources = ExecutionResources::discovering(&config(METAL, false));
    assert!(resources.gpu_context().is_none());
    assert!(matches!(
        resources.device_source(&config(METAL, false)),
        Ok(GpuDeviceSource::Discovered { .. })
    ));
}

#[test]
fn a_discovery_of_another_api_is_not_used() {
    let resources = ExecutionResources::discovering(&config(METAL, false));
    assert!(matches!(
        resources.device_source(&config(VULKAN, false)),
        Ok(GpuDeviceSource::Selected { .. })
    ));
}

// The timestamp request follows the session's statistics, whichever
// configuration began the discovery.
#[test]
fn the_session_statistics_decide_the_timestamp_request() {
    let discovering = ExecutionResources::discovering(&config(METAL, false));
    assert!(matches!(
        discovering.device_source(&config(METAL, true)),
        Ok(GpuDeviceSource::Discovered { timing: true, .. })
    ));
    let ordinary = ExecutionResources::default();
    for stats in [false, true] {
        assert!(matches!(
            ordinary.device_source(&config(METAL, stats)),
            Ok(GpuDeviceSource::Selected { timing, .. }) if timing == stats
        ));
    }
}
