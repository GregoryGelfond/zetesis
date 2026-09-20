//! Device inventory without reading or compiling an answer-set program.

use std::io::Write;

use crate::RunError;

/// Print compiled GPU APIs and detected adapter capabilities without reading
/// source, enumerating candidates, or creating a compute device. Advertised
/// capabilities do not guarantee later device/pipeline initialization succeeds.
///
/// # Errors
/// Returns [`RunError`] for output or GPU discovery failures. A CPU-only build
/// reports that GPU support is absent and succeeds without attempting discovery.
pub fn devices(output: &mut impl Write) -> Result<(), RunError> {
    writeln!(
        output,
        "CPU: available (lazy source joins and eager static closure scans)"
    )?;
    inventory(output)?;
    writeln!(
        output,
        "Auto backend: CPU until a measured GPU crossover is established. Explicit GPU backends support eager and admitted lazy grounding."
    )?;
    writeln!(
        output,
        "NVIDIA selection filters the vendor through an available graphics API; CUDA is not implemented."
    )?;
    Ok(())
}

#[cfg(not(feature = "gpu"))]
fn inventory(output: &mut impl Write) -> Result<(), RunError> {
    writeln!(
        output,
        "GPU: support not compiled; install the default build or enable --features gpu"
    )?;
    Ok(())
}

#[cfg(feature = "gpu")]
fn inventory(output: &mut impl Write) -> Result<(), RunError> {
    let compiled = zetesis_wgpu::compiled_backends();
    writeln!(output, "Compiled GPU APIs: {compiled:?}")?;
    let adapters = zetesis_wgpu::discover_adapters().map_err(RunError::Gpu)?;
    if adapters.is_empty() {
        writeln!(output, "GPU: no adapters detected; auto uses CPU")?;
    }
    for adapter in adapters {
        writeln!(
            output,
            "Adapter: {}; API={}; vendor=0x{:04x}; device=0x{:04x}; type={}; physical={}; static-profile={}",
            adapter.name(),
            adapter.backend(),
            adapter.vendor_id(),
            adapter.device_id(),
            adapter.device_type(),
            adapter.is_hardware_gpu(),
            adapter.supports_static_oracle()
        )?;
        if let Some(issue) = adapter.capability_issue() {
            writeln!(output, "  Capability issue: {issue}")?;
        }
        writeln!(
            output,
            "  Advertised optional features: {:?}",
            adapter.features()
        )?;
        writeln!(
            output,
            "  Driver: {}; {}",
            adapter.driver(),
            adapter.driver_info()
        )?;
    }
    writeln!(
        output,
        "Device and pipeline initialization are checked when the GPU is selected."
    )?;
    writeln!(
        output,
        "Current kernels require no optional features; advertised support is not enabled use."
    )?;
    Ok(())
}
