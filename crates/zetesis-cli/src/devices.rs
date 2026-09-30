//! Device inventory without reading or compiling an answer-set program.

use std::io::Write;

use zetesis_backend::GpuApi;

use crate::{ColorMode, RunError};

/// Print the backends, the compiled GPU APIs and the detected adapters with
/// their capabilities, including the adapter `--backend gpu` would use, without
/// reading source, enumerating candidates, or creating a compute device.
/// Advertised capabilities do not guarantee later device/pipeline
/// initialization succeeds.
///
/// # Errors
/// Returns [`RunError`] for output or GPU discovery failures. A CPU-only build
/// reports that GPU support is absent and succeeds without attempting discovery.
pub fn devices(output: &mut impl Write) -> Result<(), RunError> {
    devices_with_color(output, ColorMode::Never)
}

pub(crate) fn devices_with_color(
    output: &mut impl Write,
    color: ColorMode,
) -> Result<(), RunError> {
    color.metadata(
        output,
        "CPU",
        format_args!("available (lazy source joins and eager static closure scans)"),
    )?;
    inventory(output, color)?;
    writeln!(
        output,
        "Backends: cpu (the default); gpu, the platform's native API ({}); metal; vulkan. GPU backends support eager and admitted lazy grounding.",
        GpuApi::native().name()
    )?;
    Ok(())
}

#[cfg(not(feature = "gpu"))]
fn inventory(output: &mut impl Write, color: ColorMode) -> Result<(), RunError> {
    color.metadata(
        output,
        "GPU",
        format_args!(
            "not compiled into this build; install the default build for GPU execution (see INSTALL.md)"
        ),
    )?;
    Ok(())
}

#[cfg(feature = "gpu")]
fn inventory(output: &mut impl Write, color: ColorMode) -> Result<(), RunError> {
    let compiled: Vec<_> = zetesis_wgpu::compiled_apis()
        .into_iter()
        .map(GpuApi::name)
        .collect();
    color.metadata(
        output,
        "Compiled GPU APIs",
        format_args!("{}", compiled.join(", ")),
    )?;
    let adapters = zetesis_wgpu::discover_adapters().map_err(RunError::Gpu)?;
    if adapters.is_empty() {
        color.metadata(
            output,
            "GPU",
            format_args!("no adapters detected; the cpu backend remains available"),
        )?;
    }
    for adapter in &adapters {
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
    // `--backend gpu` resolves to the native API; this is the adapter device
    // creation would choose for it.
    let native = GpuApi::native();
    let selection = zetesis_wgpu::GpuSelection { api: native };
    match selection.chosen(&adapters, zetesis_wgpu::GpuOptions::default()) {
        Ok(adapter) => color.metadata(
            output,
            "--backend gpu",
            format_args!("{} ({})", adapter.name(), native.name()),
        )?,
        Err(refusal) => color.metadata(
            output,
            "--backend gpu",
            format_args!("no usable {} adapter: {refusal}", native.name()),
        )?,
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
