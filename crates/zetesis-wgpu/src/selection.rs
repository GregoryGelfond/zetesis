//! Native adapter discovery and deterministic selection by reported metadata.

use std::cmp::Ordering;

use zetesis_backend::GpuApi;

use crate::{
    AdapterBackend, AdapterCategory, AdapterMetadata, GpuError, GpuErrorKind, GpuOptions,
    check_adapter_limits,
};

/// The compute APIs zetesis targets, in the order `zetesis devices` lists them.
const TARGETED: [GpuApi; 2] = [GpuApi::Metal, GpuApi::Vulkan];

/// The wgpu backend an API opens.
fn backends(api: GpuApi) -> wgpu::Backends {
    match api {
        GpuApi::Metal => wgpu::Backends::METAL,
        GpuApi::Vulkan => wgpu::Backends::VULKAN,
    }
}

/// The adapter filter, separate from the physical-device admission policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpuSelection {
    /// The compute API to open, a hard filter. A caller holding a
    /// [`zetesis_backend::Backend`] passes its resolved API
    /// ([`zetesis_backend::Backend::resolved_api`]).
    pub api: GpuApi,
}

impl Default for GpuSelection {
    /// The platform's native API: Metal on Apple platforms, Vulkan elsewhere.
    fn default() -> Self {
        Self {
            api: GpuApi::native(),
        }
    }
}

impl GpuSelection {
    /// The adapter this selection opens among `adapters`, as
    /// [`discover_adapters`] reports them, for the static profile: the filter
    /// and order device creation applies, with each adapter's capability judged
    /// from its report. No device is created, so creating one on the adapter
    /// returned can still fail.
    ///
    /// # Errors
    /// The refusal device creation would give: no adapter at all, none admitted
    /// by `options`, or none reporting the static profile's capabilities.
    pub fn chosen(self, adapters: &[GpuInfo], options: GpuOptions) -> Result<&GpuInfo, GpuError> {
        choose(
            adapters.iter().enumerate(),
            options,
            self,
            reported_static_admission,
        )
        .map(|index| &adapters[index])
    }
}

/// Capability judged from an adapter's report: its recorded static-profile
/// capability issue, if any.
fn reported_static_admission(_: usize, info: &GpuInfo) -> Result<(), GpuError> {
    info.capability_issue().map_or(Ok(()), |reason| {
        Err(GpuError::new(GpuErrorKind::Capacity, reason))
    })
}

/// Actual adapter identity and advertised static-oracle capability preflight.
/// Discovery does not establish that device creation or kernel execution works.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuInfo {
    raw: wgpu::AdapterInfo,
    features: wgpu::Features,
    capability_issue: Option<String>,
}

impl GpuInfo {
    fn from_adapter(adapter: &wgpu::Adapter) -> Self {
        Self::from_report(
            adapter.get_info(),
            adapter.features(),
            &adapter.limits(),
            adapter
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS),
        )
    }

    fn from_report(
        raw: wgpu::AdapterInfo,
        features: wgpu::Features,
        limits: &wgpu::Limits,
        compute: bool,
    ) -> Self {
        let capability_issue = check_capabilities(compute, limits, check_adapter_limits)
            .err()
            .map(|error| error.to_string());
        Self {
            raw,
            features,
            capability_issue,
        }
    }

    /// Adapter's reported name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.raw.name
    }

    /// Optional wgpu features advertised by this adapter, not enabled features.
    ///
    /// Discovery does not create a device or prove a feature's execution. Read
    /// [`crate::GpuContext::features`] for the features granted to a live context.
    /// This fixed-size value preserves wgpu's capability vocabulary and performs
    /// no allocation or device operation.
    #[must_use]
    pub fn features(&self) -> wgpu::Features {
        self.features
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

/// The targeted compute APIs compiled for this target, in listing order. This
/// performs no hardware discovery.
#[must_use]
pub fn compiled_apis() -> Vec<GpuApi> {
    let enabled = wgpu::Instance::enabled_backend_features();
    TARGETED
        .into_iter()
        .filter(|api| enabled.intersects(backends(*api)))
        .collect()
}

/// Enumerate the adapters of every compiled targeted API, with hardware category
/// and capability diagnostics. Software and virtual adapters are included and
/// explicitly identified. Results are in selection order (hardware first, then
/// discrete, then stable reported identity). No compute device is created. No
/// visible adapters yields an empty vector; discovery does not substitute a CPU
/// solver.
///
/// # Errors
/// Returns [`GpuErrorKind::AdapterUnavailable`] if no targeted API is compiled
/// for this target, or an allocation failure while collecting inventory.
pub fn discover_adapters() -> Result<Vec<GpuInfo>, GpuError> {
    pollster::block_on(async {
        let targeted = TARGETED
            .into_iter()
            .fold(wgpu::Backends::empty(), |all, api| all | backends(api));
        let compiled = targeted & wgpu::Instance::enabled_backend_features();
        if compiled.is_empty() {
            return Err(GpuError::new(
                GpuErrorKind::AdapterUnavailable,
                "this build compiles neither Metal nor Vulkan",
            ));
        }
        let candidates = enumerate(compiled).await?;
        let mut infos = Vec::new();
        infos
            .try_reserve_exact(candidates.len())
            .map_err(|error| GpuError::new(GpuErrorKind::Allocation, error.to_string()))?;
        infos.extend(candidates.into_iter().map(|candidate| candidate.info));
        infos.sort_by(compare_info);
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
    validate: fn(&wgpu::Limits) -> Result<(), GpuError>,
) -> Result<(wgpu::Adapter, GpuInfo), GpuError> {
    let requested = requested_backends(selection.api, wgpu::Instance::enabled_backend_features())?;
    let mut candidates = enumerate(requested).await?;
    let index = choose(
        candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| (index, &candidate.info)),
        options,
        selection,
        |index, _| {
            let adapter = &candidates[index].adapter;
            check_capabilities(
                adapter
                    .get_downlevel_capabilities()
                    .flags
                    .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS),
                &adapter.limits(),
                validate,
            )
        },
    )?;
    let candidate = candidates.swap_remove(index);
    Ok((candidate.adapter, candidate.info))
}

fn check_capabilities(
    compute: bool,
    limits: &wgpu::Limits,
    validate: fn(&wgpu::Limits) -> Result<(), GpuError>,
) -> Result<(), GpuError> {
    if !compute {
        return Err(GpuError::new(
            GpuErrorKind::Capacity,
            "the adapter does not advertise compute shaders",
        ));
    }
    validate(limits)
}

async fn enumerate(backends: wgpu::Backends) -> Result<Vec<Candidate>, GpuError> {
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

/// The wgpu backend `api` opens, if this build compiles it.
fn requested_backends(api: GpuApi, enabled: wgpu::Backends) -> Result<wgpu::Backends, GpuError> {
    let requested = backends(api) & enabled;
    if requested.is_empty() {
        return Err(GpuError::new(
            GpuErrorKind::AdapterUnavailable,
            format!(
                "{} is not available in this build: zetesis compiles Metal on macOS and Vulkan on other platforms",
                api.name()
            ),
        ));
    }
    Ok(requested)
}

fn choose<'a>(
    infos: impl IntoIterator<Item = (usize, &'a GpuInfo)>,
    options: GpuOptions,
    selection: GpuSelection,
    mut validate: impl FnMut(usize, &GpuInfo) -> Result<(), GpuError>,
) -> Result<usize, GpuError> {
    let mut seen = false;
    let mut matched = false;
    let mut issue = None;
    let mut best: Option<(usize, &GpuInfo)> = None;
    for (index, info) in infos {
        seen = true;
        if !matches_selection(info, options, selection) {
            continue;
        }
        matched = true;
        if let Err(error) = validate(index, info) {
            if error.kind() != GpuErrorKind::Capacity {
                return Err(error);
            }
            issue.get_or_insert(error);
            continue;
        }
        if best.is_none_or(|(_, current)| compare_info(info, current).is_lt()) {
            best = Some((index, info));
        }
    }
    if let Some((index, _)) = best {
        return Ok(index);
    }
    let (kind, detail) = if !seen {
        (
            GpuErrorKind::AdapterUnavailable,
            format!("no {} adapter was found", selection.api.name()),
        )
    } else if matched {
        (
            GpuErrorKind::Capacity,
            format!(
                "matching adapters do not satisfy the requested compute profile: {}",
                issue.map_or_else(
                    || "unsupported capabilities".to_owned(),
                    |error| error.to_string()
                ),
            ),
        )
    } else {
        (
            GpuErrorKind::AdapterRefused,
            format!(
                "no exposed adapter matches api={}, require_gpu={}",
                selection.api, options.require_gpu,
            ),
        )
    };
    Err(GpuError::new(kind, detail))
}

fn matches_selection(info: &GpuInfo, options: GpuOptions, selection: GpuSelection) -> bool {
    backends(selection.api).contains(info.raw.backend.into())
        && (!options.require_gpu || info.is_hardware_gpu())
}

pub(crate) fn check_selection(
    info: &GpuInfo,
    options: GpuOptions,
    selection: GpuSelection,
) -> Result<(), GpuError> {
    if matches_selection(info, options, selection) {
        Ok(())
    } else {
        Err(GpuError::new(
            GpuErrorKind::AdapterRefused,
            format!(
                "supplied context adapter {} ({}, vendor_id=0x{:04x}, category={}) does not match api={}, require_gpu={}",
                info.name(),
                info.backend(),
                info.vendor_id(),
                info.device_type(),
                selection.api,
                options.require_gpu,
            ),
        ))
    }
}

/// Selection order: hardware before software, discrete before integrated, then
/// stable reported identity, so the choice never depends on enumeration order.
fn compare_info(left: &GpuInfo, right: &GpuInfo) -> Ordering {
    let key = |info: &GpuInfo| {
        (
            !info.is_hardware_gpu(),
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
    use super::{GpuInfo, GpuSelection, choose, reported_static_admission, requested_backends};
    use crate::{GpuErrorKind, GpuOptions};
    use zetesis_backend::GpuApi;

    fn report(name: &str, backend: wgpu::Backend, kind: wgpu::DeviceType, vendor: u32) -> GpuInfo {
        let mut raw = wgpu::AdapterInfo::new(kind, backend);
        raw.name = name.to_owned();
        raw.vendor = vendor;
        GpuInfo::from_report(raw, wgpu::Features::empty(), &wgpu::Limits::default(), true)
    }

    fn select(api: GpuApi) -> GpuSelection {
        GpuSelection { api }
    }

    fn pick(infos: &[GpuInfo], selection: GpuSelection) -> usize {
        choose(
            infos.iter().enumerate(),
            GpuOptions::default(),
            selection,
            reported_static_admission,
        )
        .expect("compatible physical adapter")
    }

    #[test]
    fn the_chosen_adapter_is_the_one_device_creation_would_open() {
        let infos = [
            report("software", wgpu::Backend::Vulkan, wgpu::DeviceType::Cpu, 0),
            report(
                "integrated",
                wgpu::Backend::Vulkan,
                wgpu::DeviceType::IntegratedGpu,
                1,
            ),
            report(
                "discrete",
                wgpu::Backend::Vulkan,
                wgpu::DeviceType::DiscreteGpu,
                2,
            ),
        ];
        let selection = select(GpuApi::Vulkan);
        let chosen = selection.chosen(&infos, GpuOptions::default()).unwrap();
        assert_eq!(chosen, &infos[pick(&infos, selection)]);
        assert_eq!(chosen.name(), "discrete");
    }

    #[test]
    fn no_chosen_adapter_carries_the_refusal() {
        let refusal = select(GpuApi::Metal)
            .chosen(&[], GpuOptions::default())
            .unwrap_err();
        assert_eq!(refusal.kind(), GpuErrorKind::AdapterUnavailable);
    }

    #[test]
    fn an_api_filter_never_enables_another_or_a_nonexecuting_backend() {
        for (api, mask) in [
            (GpuApi::Metal, wgpu::Backends::METAL),
            (GpuApi::Vulkan, wgpu::Backends::VULKAN),
        ] {
            assert_eq!(
                requested_backends(api, wgpu::Backends::all()).unwrap(),
                mask
            );
            let missing = requested_backends(api, wgpu::Backends::all() - mask).unwrap_err();
            assert_eq!(missing.kind(), GpuErrorKind::AdapterUnavailable);
            assert!(missing.to_string().contains(api.name()));
        }
    }

    #[test]
    fn the_default_selection_is_the_native_api() {
        assert_eq!(GpuSelection::default().api, GpuApi::native());
    }

    #[test]
    fn a_selection_considers_only_adapters_of_its_api() {
        let inventory = [
            report(
                "vulkan",
                wgpu::Backend::Vulkan,
                wgpu::DeviceType::DiscreteGpu,
                0x10de,
            ),
            report(
                "metal",
                wgpu::Backend::Metal,
                wgpu::DeviceType::IntegratedGpu,
                0x106b,
            ),
        ];
        assert_eq!(pick(&inventory, select(GpuApi::Metal)), 1);
        assert_eq!(pick(&inventory, select(GpuApi::Vulkan)), 0);
    }

    #[test]
    fn incapable_and_software_adapters_are_skipped_under_the_default_policy() {
        let mut incapable = report(
            "limited Metal",
            wgpu::Backend::Metal,
            wgpu::DeviceType::DiscreteGpu,
            0x1002,
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
                "capable Metal",
                wgpu::Backend::Metal,
                wgpu::DeviceType::IntegratedGpu,
                0x106b,
            ),
        ];
        assert_eq!(pick(&inventory, select(GpuApi::Metal)), 2);
    }

    #[test]
    fn the_hardware_filter_refuses_cpu_virtual_and_unknown_categories() {
        for kind in [
            wgpu::DeviceType::Cpu,
            wgpu::DeviceType::VirtualGpu,
            wgpu::DeviceType::Other,
        ] {
            let inventory = [report("adapter", wgpu::Backend::Vulkan, kind, 0x10de)];
            assert_eq!(
                choose(
                    inventory.iter().enumerate(),
                    GpuOptions::default(),
                    select(GpuApi::Vulkan),
                    reported_static_admission,
                )
                .unwrap_err()
                .kind(),
                GpuErrorKind::AdapterRefused
            );
            assert_eq!(
                choose(
                    inventory.iter().enumerate(),
                    GpuOptions { require_gpu: false },
                    select(GpuApi::Vulkan),
                    reported_static_admission,
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
                wgpu::Backend::Metal,
                wgpu::DeviceType::IntegratedGpu,
                0,
            ),
        ];
        assert_eq!(
            choose(
                inventory.iter().enumerate(),
                GpuOptions { require_gpu: false },
                select(GpuApi::Metal),
                reported_static_admission,
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn a_discrete_gpu_is_preferred_to_an_integrated_one() {
        let inventory = [
            report(
                "integrated",
                wgpu::Backend::Vulkan,
                wgpu::DeviceType::IntegratedGpu,
                0x8086,
            ),
            report(
                "discrete",
                wgpu::Backend::Vulkan,
                wgpu::DeviceType::DiscreteGpu,
                0x10de,
            ),
        ];
        assert_eq!(pick(&inventory, select(GpuApi::Vulkan)), 1);
    }

    #[test]
    fn nonnative_backends_are_never_selected_even_when_hardware_is_reported() {
        for backend in [
            wgpu::Backend::Noop,
            wgpu::Backend::BrowserWebGpu,
            wgpu::Backend::Dx12,
            wgpu::Backend::Gl,
        ] {
            let inventory = [report(
                "non-native",
                backend,
                wgpu::DeviceType::DiscreteGpu,
                0,
            )];
            for api in [GpuApi::Metal, GpuApi::Vulkan] {
                assert_eq!(
                    choose(
                        inventory.iter().enumerate(),
                        GpuOptions { require_gpu: false },
                        select(api),
                        reported_static_admission,
                    )
                    .unwrap_err()
                    .kind(),
                    GpuErrorKind::AdapterRefused
                );
            }
        }
    }

    #[test]
    fn empty_inventory_policy_refusal_and_capability_refusal_are_distinct() {
        assert_eq!(
            choose(
                [].iter().enumerate(),
                GpuOptions::default(),
                select(GpuApi::Metal),
                reported_static_admission,
            )
            .unwrap_err()
            .kind(),
            GpuErrorKind::AdapterUnavailable
        );
        let raw = wgpu::AdapterInfo::new(wgpu::DeviceType::IntegratedGpu, wgpu::Backend::Metal);
        let no_compute = GpuInfo::from_report(
            raw.clone(),
            wgpu::Features::empty(),
            &wgpu::Limits::default(),
            false,
        );
        assert!(!no_compute.supports_static_oracle());
        assert!(no_compute.capability_issue().unwrap().contains("compute"));
        let small_limits = wgpu::Limits {
            max_compute_invocations_per_workgroup: 1,
            ..wgpu::Limits::default()
        };
        let limited = GpuInfo::from_report(raw, wgpu::Features::empty(), &small_limits, true);
        assert!(!limited.supports_static_oracle());
        for info in [no_compute, limited] {
            assert_eq!(
                choose(
                    [(0, &info)],
                    GpuOptions::default(),
                    select(GpuApi::Metal),
                    reported_static_admission,
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
            0x10de,
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
        let index = pick(&inventory, select(GpuApi::Vulkan));
        assert_eq!(inventory[index], first);
        inventory.reverse();
        let reversed = pick(&inventory, select(GpuApi::Vulkan));
        assert_eq!(inventory[reversed], first);
    }
}

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
mod metadata_tests;

#[cfg(test)]
mod context_policy_tests;
#[cfg(test)]
mod test_support;

#[cfg(test)]
mod primitive_tests;
