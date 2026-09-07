//! Native adapter discovery and deterministic selection by reported metadata.

use std::cmp::Ordering;
use std::fmt;

use crate::{
    AdapterBackend, AdapterCategory, AdapterMetadata, GpuError, GpuErrorKind, GpuOptions,
    check_adapter_limits,
};

/// NVIDIA's PCI vendor identifier. Matching uses the reported numeric ID,
/// never a device-name substring. This selects wgpu devices, not CUDA.
pub const NVIDIA_VENDOR_ID: u32 = 0x10de;

/// Native compute API preference. An explicit API is a hard selection filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GpuBackendPreference {
    /// Choose among the compiled native APIs using the documented ranking.
    #[default]
    Auto,
    /// Require Metal; unavailable when not compiled for the current target.
    Metal,
    /// Require Vulkan; this can drive NVIDIA and other vendors' GPUs.
    Vulkan,
    /// Require Direct3D 12; unavailable when not compiled for the current target.
    Dx12,
    /// Require OpenGL/OpenGL ES with compute support and the oracle's limits.
    Gl,
}

impl GpuBackendPreference {
    const EXPLICIT: [Self; 4] = [Self::Metal, Self::Vulkan, Self::Dx12, Self::Gl];

    fn backends(self) -> wgpu::Backends {
        match self {
            Self::Auto => {
                wgpu::Backends::METAL
                    | wgpu::Backends::VULKAN
                    | wgpu::Backends::DX12
                    | wgpu::Backends::GL
            }
            Self::Metal => wgpu::Backends::METAL,
            Self::Vulkan => wgpu::Backends::VULKAN,
            Self::Dx12 => wgpu::Backends::DX12,
            Self::Gl => wgpu::Backends::GL,
        }
    }

    fn admits(self, backend: wgpu::Backend) -> bool {
        self.backends().contains(backend.into())
    }
}

impl fmt::Display for GpuBackendPreference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Auto => "auto",
            Self::Metal => "metal",
            Self::Vulkan => "vulkan",
            Self::Dx12 => "dx12",
            Self::Gl => "gl",
        })
    }
}

/// Hard adapter filters, separate from the physical-device admission policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuSelection {
    /// Required compute API, or Auto to rank compatible compiled native APIs.
    pub backend: GpuBackendPreference,
    /// Required exact backend-reported vendor ID; `None` admits any vendor.
    /// Use [`NVIDIA_VENDOR_ID`] for NVIDIA through a supported wgpu API.
    pub vendor_id: Option<u32>,
}

/// Actual adapter identity and advertised static-oracle capability preflight.
/// Discovery does not establish that device creation or kernel execution works.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuInfo {
    raw: wgpu::AdapterInfo,
    capability_issue: Option<String>,
}

impl GpuInfo {
    fn from_adapter(adapter: &wgpu::Adapter) -> Self {
        Self::from_report(
            adapter.get_info(),
            &adapter.limits(),
            adapter
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS),
        )
    }

    fn from_report(raw: wgpu::AdapterInfo, limits: &wgpu::Limits, compute: bool) -> Self {
        let capability_issue = if compute {
            check_adapter_limits(limits)
                .err()
                .map(|error| error.to_string())
        } else {
            Some("the adapter does not advertise compute shaders".to_owned())
        };
        Self {
            raw,
            capability_issue,
        }
    }

    /// Adapter's reported name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.raw.name
    }

    /// Borrow reported metadata as typed values with explicit optional text.
    /// This has fixed cost and performs no allocation or adapter discovery.
    #[must_use]
    pub fn metadata(&self) -> AdapterMetadata<'_> {
        AdapterMetadata {
            name: self.name(),
            backend: self.backend_kind(),
            category: self.category(),
            vendor_id: self.vendor_id(),
            device_id: self.device_id(),
            pci_bus_id: reported_text(self.pci_bus_id()),
            driver: reported_text(self.driver()),
            driver_info: reported_text(self.driver_info()),
        }
    }

    /// Observed compute API, independent of the original selection preference.
    #[must_use]
    pub fn backend_kind(&self) -> AdapterBackend {
        match self.raw.backend {
            wgpu::Backend::Noop => AdapterBackend::Noop,
            wgpu::Backend::Vulkan => AdapterBackend::Vulkan,
            wgpu::Backend::Metal => AdapterBackend::Metal,
            wgpu::Backend::Dx12 => AdapterBackend::Dx12,
            wgpu::Backend::Gl => AdapterBackend::Gl,
            wgpu::Backend::BrowserWebGpu => AdapterBackend::BrowserWebGpu,
        }
    }

    /// Reported device category, without inferring successful execution.
    #[must_use]
    pub fn category(&self) -> AdapterCategory {
        match self.raw.device_type {
            wgpu::DeviceType::Other => AdapterCategory::Other,
            wgpu::DeviceType::IntegratedGpu => AdapterCategory::IntegratedGpu,
            wgpu::DeviceType::DiscreteGpu => AdapterCategory::DiscreteGpu,
            wgpu::DeviceType::VirtualGpu => AdapterCategory::VirtualGpu,
            wgpu::DeviceType::Cpu => AdapterCategory::Cpu,
        }
    }

    /// Graphics/compute API used, preserving the existing title-case labels.
    #[must_use]
    pub fn backend(&self) -> &str {
        self.backend_kind().label()
    }

    /// Unmodified wgpu device-category name.
    #[must_use]
    pub fn device_type(&self) -> &str {
        self.category().label()
    }

    /// Whether wgpu classified this adapter as an integrated or discrete GPU.
    #[must_use]
    pub fn is_hardware_gpu(&self) -> bool {
        matches!(
            self.raw.device_type,
            wgpu::DeviceType::IntegratedGpu | wgpu::DeviceType::DiscreteGpu
        )
    }

    /// Exact backend-reported vendor ID, usually a PCI ID. Zero can mean unknown.
    #[must_use]
    pub fn vendor_id(&self) -> u32 {
        self.raw.vendor
    }

    /// Exact backend-reported device ID; not a unique physical-device identifier.
    #[must_use]
    pub fn device_id(&self) -> u32 {
        self.raw.device
    }

    /// Reported PCI bus identifier, or an empty string when unavailable.
    #[must_use]
    pub fn pci_bus_id(&self) -> &str {
        &self.raw.device_pci_bus_id
    }

    /// Backend-reported driver name, possibly empty.
    #[must_use]
    pub fn driver(&self) -> &str {
        &self.raw.driver
    }

    /// Backend-reported driver version/details, possibly empty.
    #[must_use]
    pub fn driver_info(&self) -> &str {
        &self.raw.driver_info
    }

    /// Whether advertised compute support and limits pass the static profile's
    /// preflight. This is independent of hardware category and execution success.
    #[must_use]
    pub fn supports_static_oracle(&self) -> bool {
        self.capability_issue.is_none()
    }

    /// First advertised capability refusal, if any. Device/pipeline creation
    /// can still fail when this is absent.
    #[must_use]
    pub fn capability_issue(&self) -> Option<&str> {
        self.capability_issue.as_deref()
    }
}

fn reported_text(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}

/// Native APIs compiled for this target. Auto, browser WebGPU, and the
/// nonexecuting Noop backend are excluded. This performs no hardware discovery.
#[must_use]
pub fn compiled_backends() -> Vec<GpuBackendPreference> {
    let enabled = wgpu::Instance::enabled_backend_features();
    GpuBackendPreference::EXPLICIT
        .into_iter()
        .filter(|backend| enabled.intersects(backend.backends()))
        .collect()
}

/// Enumerate native adapters with hardware category and capability diagnostics.
/// Software/virtual adapters are included and explicitly identified. Results use
/// the same reported-metadata order as Auto; a physical GPU may appear through
/// multiple APIs. No compute device is created. No visible adapters yields an
/// empty vector; discovery does not substitute a CPU solver.
///
/// # Errors
/// Returns [`GpuErrorKind::AdapterUnavailable`] if no native API is compiled
/// for this target, or an allocation failure while collecting inventory.
pub fn discover_adapters() -> Result<Vec<GpuInfo>, GpuError> {
    pollster::block_on(async {
        let candidates = enumerate(GpuBackendPreference::Auto).await?;
        let mut infos = Vec::new();
        infos
            .try_reserve_exact(candidates.len())
            .map_err(|error| GpuError::new(GpuErrorKind::Allocation, error.to_string()))?;
        infos.extend(candidates.into_iter().map(|candidate| candidate.info));
        infos.sort_by(|left, right| compare_info(left, right, HostPlatform::current()));
        Ok(infos)
    })
}

struct Candidate {
    adapter: wgpu::Adapter,
    info: GpuInfo,
}

pub(crate) async fn select_adapter(
    options: GpuOptions,
    selection: GpuSelection,
) -> Result<(wgpu::Adapter, GpuInfo), GpuError> {
    let mut candidates = enumerate(selection.backend).await?;
    let index = choose(
        candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| (index, &candidate.info)),
        options,
        selection,
        HostPlatform::current(),
    )?;
    let candidate = candidates.swap_remove(index);
    Ok((candidate.adapter, candidate.info))
}

async fn enumerate(preference: GpuBackendPreference) -> Result<Vec<Candidate>, GpuError> {
    let backends = requested_backends(preference, wgpu::Instance::enabled_backend_features())?;
    // Backend choice never uses environment overrides. In particular, a WGPU
    // variable cannot defeat an explicit API filter or enable the Noop backend.
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapters = instance.enumerate_adapters(backends).await;
    let mut candidates = Vec::new();
    candidates
        .try_reserve_exact(adapters.len())
        .map_err(|error| GpuError::new(GpuErrorKind::Allocation, error.to_string()))?;
    for adapter in adapters {
        let info = GpuInfo::from_adapter(&adapter);
        candidates.push(Candidate { adapter, info });
    }
    Ok(candidates)
}

fn requested_backends(
    preference: GpuBackendPreference,
    enabled: wgpu::Backends,
) -> Result<wgpu::Backends, GpuError> {
    let requested = preference.backends() & enabled;
    if requested.is_empty() {
        return Err(GpuError::new(
            GpuErrorKind::AdapterUnavailable,
            format!(
                "requested {preference} backend is not compiled for this target (enabled: {enabled:?})"
            ),
        ));
    }
    Ok(requested)
}

fn choose<'a>(
    infos: impl IntoIterator<Item = (usize, &'a GpuInfo)>,
    options: GpuOptions,
    selection: GpuSelection,
    platform: HostPlatform,
) -> Result<usize, GpuError> {
    let mut seen = false;
    let mut matched = false;
    let mut issue = None;
    let mut best: Option<(usize, &GpuInfo)> = None;
    for (index, info) in infos {
        seen = true;
        if !selection.backend.admits(info.raw.backend)
            || selection
                .vendor_id
                .is_some_and(|vendor| vendor != info.vendor_id())
            || (options.require_gpu && !info.is_hardware_gpu())
        {
            continue;
        }
        matched = true;
        if let Some(reason) = info.capability_issue() {
            issue.get_or_insert(reason);
            continue;
        }
        if best.is_none_or(|(_, current)| compare_info(info, current, platform).is_lt()) {
            best = Some((index, info));
        }
    }
    if let Some((index, _)) = best {
        return Ok(index);
    }
    let (kind, detail) = if !seen {
        (
            GpuErrorKind::AdapterUnavailable,
            "no adapters were exposed by the requested native APIs".to_owned(),
        )
    } else if matched {
        (
            GpuErrorKind::Capacity,
            format!(
                "matching adapters do not satisfy the static oracle profile: {}",
                issue.unwrap_or("unsupported capabilities"),
            ),
        )
    } else {
        (
            GpuErrorKind::AdapterRefused,
            format!(
                "no exposed adapter matches backend={}, vendor_id={:?}, require_gpu={}",
                selection.backend, selection.vendor_id, options.require_gpu,
            ),
        )
    };
    Err(GpuError::new(kind, detail))
}

#[derive(Clone, Copy)]
enum HostPlatform {
    Apple,
    Windows,
    Other,
}

impl HostPlatform {
    fn current() -> Self {
        if cfg!(target_vendor = "apple") {
            Self::Apple
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Other
        }
    }

    fn backend_rank(self, backend: wgpu::Backend) -> u8 {
        match (self, backend) {
            (Self::Apple, wgpu::Backend::Metal)
            | (Self::Windows, wgpu::Backend::Dx12)
            | (Self::Other, wgpu::Backend::Vulkan) => 0,
            (Self::Apple | Self::Windows, wgpu::Backend::Vulkan)
            | (Self::Other, wgpu::Backend::Metal) => 1,
            (Self::Apple | Self::Other, wgpu::Backend::Dx12)
            | (Self::Windows, wgpu::Backend::Metal) => 2,
            (_, wgpu::Backend::Gl) => 3,
            (_, wgpu::Backend::BrowserWebGpu) => 4,
            (_, wgpu::Backend::Noop) => 5,
        }
    }
}

fn compare_info(left: &GpuInfo, right: &GpuInfo, platform: HostPlatform) -> Ordering {
    let key = |info: &GpuInfo| {
        (
            !info.is_hardware_gpu(),
            platform.backend_rank(info.raw.backend),
            category_rank(info.raw.device_type),
            info.vendor_id(),
            info.device_id(),
        )
    };
    key(left)
        .cmp(&key(right))
        .then_with(|| left.pci_bus_id().cmp(right.pci_bus_id()))
        .then_with(|| left.name().cmp(right.name()))
        .then_with(|| left.driver().cmp(right.driver()))
        .then_with(|| left.driver_info().cmp(right.driver_info()))
}

fn category_rank(category: wgpu::DeviceType) -> u8 {
    match category {
        wgpu::DeviceType::DiscreteGpu => 0,
        wgpu::DeviceType::IntegratedGpu => 1,
        wgpu::DeviceType::VirtualGpu => 2,
        wgpu::DeviceType::Other => 3,
        wgpu::DeviceType::Cpu => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GpuBackendPreference, GpuInfo, GpuSelection, HostPlatform, NVIDIA_VENDOR_ID, choose,
        requested_backends,
    };
    use crate::{GpuErrorKind, GpuOptions};

    fn report(name: &str, backend: wgpu::Backend, kind: wgpu::DeviceType, vendor: u32) -> GpuInfo {
        let mut raw = wgpu::AdapterInfo::new(kind, backend);
        raw.name = name.to_owned();
        raw.vendor = vendor;
        GpuInfo::from_report(raw, &wgpu::Limits::default(), true)
    }

    fn pick(infos: &[GpuInfo], selection: GpuSelection, platform: HostPlatform) -> usize {
        choose(
            infos.iter().enumerate(),
            GpuOptions::default(),
            selection,
            platform,
        )
        .expect("compatible physical adapter")
    }

    #[test]
    fn explicit_api_filters_never_enable_an_alternative_or_nonexecuting_backend() {
        for (preference, mask) in [
            (GpuBackendPreference::Metal, wgpu::Backends::METAL),
            (GpuBackendPreference::Vulkan, wgpu::Backends::VULKAN),
            (GpuBackendPreference::Dx12, wgpu::Backends::DX12),
            (GpuBackendPreference::Gl, wgpu::Backends::GL),
        ] {
            assert_eq!(
                requested_backends(preference, wgpu::Backends::all()).unwrap(),
                mask
            );
            assert_eq!(
                requested_backends(preference, wgpu::Backends::all() - mask)
                    .unwrap_err()
                    .kind(),
                GpuErrorKind::AdapterUnavailable,
            );
        }
        let native = requested_backends(GpuBackendPreference::Auto, wgpu::Backends::all()).unwrap();
        assert!(!native.intersects(wgpu::Backends::NOOP | wgpu::Backends::BROWSER_WEBGPU));
        assert_eq!(
            requested_backends(GpuBackendPreference::Auto, wgpu::Backends::NOOP)
                .unwrap_err()
                .kind(),
            GpuErrorKind::AdapterUnavailable,
        );
    }

    #[test]
    fn auto_prefers_the_native_platform_api_among_compatible_physical_adapters() {
        let inventory = [
            report(
                "vulkan",
                wgpu::Backend::Vulkan,
                wgpu::DeviceType::DiscreteGpu,
                NVIDIA_VENDOR_ID,
            ),
            report(
                "metal",
                wgpu::Backend::Metal,
                wgpu::DeviceType::IntegratedGpu,
                0x106b,
            ),
            report(
                "dx12",
                wgpu::Backend::Dx12,
                wgpu::DeviceType::DiscreteGpu,
                NVIDIA_VENDOR_ID,
            ),
        ];
        assert_eq!(
            pick(&inventory, GpuSelection::default(), HostPlatform::Apple),
            1
        );
        assert_eq!(
            pick(&inventory, GpuSelection::default(), HostPlatform::Windows),
            2
        );
        assert_eq!(
            pick(&inventory, GpuSelection::default(), HostPlatform::Other),
            0
        );
    }

    #[test]
    fn auto_skips_incapable_native_devices_and_never_uses_software_under_default_policy() {
        let mut incapable = report(
            "limited Metal",
            wgpu::Backend::Metal,
            wgpu::DeviceType::IntegratedGpu,
            0x106b,
        );
        incapable.capability_issue = Some("workgroup storage is too small".to_owned());
        let inventory = [
            report(
                "software Metal",
                wgpu::Backend::Metal,
                wgpu::DeviceType::Cpu,
                0x106b,
            ),
            incapable,
            report(
                "Vulkan GPU",
                wgpu::Backend::Vulkan,
                wgpu::DeviceType::DiscreteGpu,
                0x1002,
            ),
        ];
        assert_eq!(
            pick(&inventory, GpuSelection::default(), HostPlatform::Apple),
            2
        );
    }

    #[test]
    fn hardware_filter_refuses_cpu_virtual_and_unknown_categories_even_for_nvidia() {
        for kind in [
            wgpu::DeviceType::Cpu,
            wgpu::DeviceType::VirtualGpu,
            wgpu::DeviceType::Other,
        ] {
            let inventory = [report(
                "NVIDIA",
                wgpu::Backend::Vulkan,
                kind,
                NVIDIA_VENDOR_ID,
            )];
            let selection = GpuSelection {
                vendor_id: Some(NVIDIA_VENDOR_ID),
                ..GpuSelection::default()
            };
            assert_eq!(
                choose(
                    inventory.iter().enumerate(),
                    GpuOptions::default(),
                    selection,
                    HostPlatform::Other,
                )
                .unwrap_err()
                .kind(),
                GpuErrorKind::AdapterRefused
            );
            assert_eq!(
                choose(
                    inventory.iter().enumerate(),
                    GpuOptions { require_gpu: false },
                    selection,
                    HostPlatform::Other,
                )
                .unwrap(),
                0
            );
        }
    }

    #[test]
    fn physical_adapters_remain_preferred_when_software_admission_is_explicit() {
        let inventory = [
            report(
                "software Metal",
                wgpu::Backend::Metal,
                wgpu::DeviceType::Cpu,
                0,
            ),
            report(
                "real GPU",
                wgpu::Backend::Gl,
                wgpu::DeviceType::IntegratedGpu,
                0,
            ),
        ];
        assert_eq!(
            choose(
                inventory.iter().enumerate(),
                GpuOptions { require_gpu: false },
                GpuSelection::default(),
                HostPlatform::Apple,
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn vendor_filter_uses_exact_ids_and_combines_with_the_backend_filter() {
        let inventory = [
            report(
                "NVIDIA-looking name",
                wgpu::Backend::Metal,
                wgpu::DeviceType::DiscreteGpu,
                0x1002,
            ),
            report(
                "numeric vendor",
                wgpu::Backend::Vulkan,
                wgpu::DeviceType::DiscreteGpu,
                NVIDIA_VENDOR_ID,
            ),
        ];
        let nvidia = GpuSelection {
            vendor_id: Some(NVIDIA_VENDOR_ID),
            ..GpuSelection::default()
        };
        assert_eq!(pick(&inventory, nvidia, HostPlatform::Apple), 1);
        let contradictory = GpuSelection {
            backend: GpuBackendPreference::Metal,
            ..nvidia
        };
        assert_eq!(
            choose(
                inventory.iter().enumerate(),
                GpuOptions::default(),
                contradictory,
                HostPlatform::Apple,
            )
            .unwrap_err()
            .kind(),
            GpuErrorKind::AdapterRefused
        );
        let metal = GpuSelection {
            backend: GpuBackendPreference::Metal,
            vendor_id: None,
        };
        assert_eq!(pick(&inventory, metal, HostPlatform::Other), 0);
    }

    #[test]
    fn native_discovery_policy_excludes_noop_and_browser_even_when_hardware_is_reported() {
        for backend in [wgpu::Backend::Noop, wgpu::Backend::BrowserWebGpu] {
            let inventory = [report(
                "non-native",
                backend,
                wgpu::DeviceType::DiscreteGpu,
                0,
            )];
            assert_eq!(
                choose(
                    inventory.iter().enumerate(),
                    GpuOptions { require_gpu: false },
                    GpuSelection::default(),
                    HostPlatform::Other,
                )
                .unwrap_err()
                .kind(),
                GpuErrorKind::AdapterRefused
            );
        }
    }

    #[test]
    fn empty_inventory_policy_refusal_and_capability_refusal_are_distinct() {
        assert_eq!(
            choose(
                [].iter().enumerate(),
                GpuOptions::default(),
                GpuSelection::default(),
                HostPlatform::Other,
            )
            .unwrap_err()
            .kind(),
            GpuErrorKind::AdapterUnavailable
        );
        let raw = wgpu::AdapterInfo::new(wgpu::DeviceType::IntegratedGpu, wgpu::Backend::Gl);
        let no_compute = GpuInfo::from_report(raw.clone(), &wgpu::Limits::default(), false);
        assert!(!no_compute.supports_static_oracle());
        assert!(no_compute.capability_issue().unwrap().contains("compute"));
        let small_limits = wgpu::Limits {
            max_compute_invocations_per_workgroup: 1,
            ..wgpu::Limits::default()
        };
        let limited = GpuInfo::from_report(raw, &small_limits, true);
        assert!(!limited.supports_static_oracle());
        for info in [no_compute, limited] {
            assert_eq!(
                choose(
                    [(0, &info)],
                    GpuOptions::default(),
                    GpuSelection::default(),
                    HostPlatform::Other,
                )
                .unwrap_err()
                .kind(),
                GpuErrorKind::Capacity
            );
        }
    }

    #[test]
    fn reported_identity_breaks_ties_independently_of_enumeration_order() {
        let mut first = report(
            "identical",
            wgpu::Backend::Vulkan,
            wgpu::DeviceType::DiscreteGpu,
            NVIDIA_VENDOR_ID,
        );
        first.raw.device_pci_bus_id = "0000:01:00.0".to_owned();
        let mut second = first.clone();
        second.raw.device_pci_bus_id = "0000:02:00.0".to_owned();
        let integrated = report(
            "integrated",
            wgpu::Backend::Vulkan,
            wgpu::DeviceType::IntegratedGpu,
            0,
        );
        let mut inventory = vec![second, integrated, first.clone()];
        let index = pick(&inventory, GpuSelection::default(), HostPlatform::Other);
        assert_eq!(inventory[index], first);
        inventory.reverse();
        let reversed = pick(&inventory, GpuSelection::default(), HostPlatform::Other);
        assert_eq!(inventory[reversed], first);
    }
}

#[cfg(test)]
#[path = "../tests/support/selection_contracts.rs"]
mod contract_tests;

#[cfg(test)]
#[path = "../tests/support/adapter_metadata.rs"]
mod metadata_tests;
