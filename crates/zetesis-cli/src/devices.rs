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
        "Auto backend: CPU for the first seed and batches smaller than {}; larger batches may use a physical GPU. Auto/lazy grounding stays lazy across hardware changes; --grounder eager preserves static lowering across fallback.",
        crate::Backend::AUTO_GPU_MIN_BATCH
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
            "  Driver: {}; {}",
            adapter.driver(),
            adapter.driver_info()
        )?;
    }
    writeln!(
        output,
        "Device and pipeline initialization are checked when the GPU is selected."
    )?;
    Ok(())
}
