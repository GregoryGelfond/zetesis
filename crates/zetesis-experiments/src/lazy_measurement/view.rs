use serde::{Serialize, Serializer};
use zetesis_cpu::lazy;
use zetesis_wgpu::LazyGpuStatistics;

use super::{Case, Configuration};

/// Measured execution strategy; only Metal/Vulkan variants submit physical device work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Route {
    /// Independent scalar source checking of each occurrence.
    Scalar,
    /// The same scalar checks scheduled in an owned Rayon pool.
    Rayon,
    /// Union source scans with the portable round evaluator.
    PortableUnion,
    /// World-filtered source scans with the portable round evaluator.
    PortableWorlds,
    /// Union source scans with physical Metal consequences.
    MetalUnion,
    /// World-filtered source scans with physical Metal consequences.
    MetalWorlds,
    /// Union source scans with physical Vulkan consequences.
    VulkanUnion,
    /// World-filtered source scans with physical Vulkan consequences.
    VulkanWorlds,
}

impl Route {
    pub(super) const fn is_device(self) -> bool {
        matches!(
            self,
            Self::MetalUnion | Self::MetalWorlds | Self::VulkanUnion | Self::VulkanWorlds
        )
    }
}

/// Sample population; warmups and initial cases are never timed repetitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// First observed invocation of this route on this case, not a cold-cache run.
    Initial,
    /// Retained preparation observation outside the timing population.
    Warmup,
    /// Member of the requested repeated timing population.
    Timed,
}

/// Exact source progress for one completed batch, without process-memory claims.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct SourceWork {
    /// Completed immutable rounds.
    pub rounds: u64,
    /// Charged source operations, including mask operations.
    pub source_work: u64,
    /// Offered source instances, not instance/world products.
    pub instances: u64,
    /// Successfully evaluated nonempty chunks.
    pub chunks: u64,
    /// Distinct demanded catalog identities.
    pub catalog_atoms: usize,
    /// Mask operations already included in `source_work`.
    pub mask_words: u64,
    /// Empty current-world join prefixes omitted by Worlds.
    pub pruned_prefixes: u64,
    /// Peak requested membership/frame/index bytes; not RSS.
    pub peak_mask_bytes: usize,
}

impl From<lazy::Progress> for SourceWork {
    fn from(progress: lazy::Progress) -> Self {
        Self {
            rounds: progress.rounds,
            source_work: progress.source_work,
            instances: progress.instances,
            chunks: progress.chunks,
            catalog_atoms: progress.catalog_atoms,
            mask_words: progress.mask_words,
            pruned_prefixes: progress.pruned_prefixes,
            peak_mask_bytes: progress.peak_mask_bytes,
        }
    }
}

/// Actual submitted device work; transfers and retained buffer capacity are
/// separate quantities. Neither measures driver storage or process RSS.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct DeviceWork {
    /// Successfully submitted nonempty chunks.
    pub dispatches: u64,
    /// Sum of submitted instances times candidate occurrences.
    pub world_instances: u64,
    /// Uniform, source-record and truth bytes uploaded.
    pub uploaded_bytes: u64,
    /// Successfully decoded readback bytes.
    pub downloaded_bytes: u64,
    /// Submitted chunks requesting at least one fresh transport buffer.
    pub transport_allocations: u64,
    /// Submitted chunks reusing every buffer from an earlier chunk of the batch.
    pub transport_reuses: u64,
    /// Maximum requested GPU buffer payload, including inactive capacity.
    pub peak_transport_bytes: u64,
    /// Overlapping reasons the previous complete buffer set could not be reused.
    #[serde(serialize_with = "replacement_view")]
    pub transport_replacements: zetesis_wgpu::LazyTransportReplacements,
    /// Per-binding requests and prospective slack-release causes.
    #[serde(serialize_with = "usage_view")]
    pub transport_usage: zetesis_wgpu::LazyTransportUsage,
    /// Host submission/readback wait, not a shader timestamp.
    pub host_wait_ns: u128,
}

impl From<LazyGpuStatistics> for DeviceWork {
    fn from(statistics: LazyGpuStatistics) -> Self {
        Self {
            dispatches: statistics.dispatches,
            world_instances: statistics.world_instances,
            uploaded_bytes: statistics.uploaded_bytes,
            downloaded_bytes: statistics.downloaded_bytes,
            transport_allocations: statistics.transport_allocations,
            transport_reuses: statistics.transport_reuses,
            peak_transport_bytes: statistics.peak_transport_bytes,
            transport_replacements: statistics.transport_replacements,
            transport_usage: statistics.transport_usage,
            host_wait_ns: statistics.host_wait.as_nanos(),
        }
    }
}

fn replacement_view<S: Serializer>(
    reasons: &zetesis_wgpu::LazyTransportReplacements,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeStruct;
    let mut record = serializer.serialize_struct("LazyTransportReplacements", 8)?;
    record.serialize_field("initial", &reasons.initial)?;
    record.serialize_field("offsets_growth", &reasons.offsets_growth)?;
    record.serialize_field("records_growth", &reasons.records_growth)?;
    record.serialize_field("snapshots_growth", &reasons.snapshots_growth)?;
    record.serialize_field("seeds_growth", &reasons.seeds_growth)?;
    record.serialize_field("result_shape", &reasons.result_shape)?;
    record.serialize_field("budget", &reasons.budget)?;
    record.serialize_field("accounting_overflow", &reasons.accounting_overflow)?;
    record.end()
}

fn usage_view<S: Serializer>(
    usage: &zetesis_wgpu::LazyTransportUsage,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeStruct;
    let mut record = serializer.serialize_struct("LazyTransportUsage", 9)?;
    for (name, binding) in [
        ("uniform", usage.uniform),
        ("offsets", usage.offsets),
        ("records", usage.records),
        ("snapshots", usage.snapshots),
        ("seeds", usage.seeds),
        ("output", usage.output),
        ("readback", usage.readback),
    ] {
        record.serialize_field(name, &BufferUsage::from(binding))?;
    }
    record.serialize_field("budget_releases", &usage.budget_releases)?;
    record.serialize_field(
        "accounting_overflow_releases",
        &usage.accounting_overflow_releases,
    )?;
    record.end()
}

#[derive(Serialize)]
struct BufferUsage {
    allocations: u64,
    reuses: u64,
}

impl From<zetesis_wgpu::LazyBufferUsage> for BufferUsage {
    fn from(usage: zetesis_wgpu::LazyBufferUsage) -> Self {
        Self {
            allocations: usage.allocations,
            reuses: usage.reuses,
        }
    }
}

/// One successful route observation, after complete ordered scalar agreement.
#[derive(Clone, Debug, Serialize)]
pub struct Sample {
    /// Ordinal in the configured case list, including repeated cases.
    pub case_index: usize,
    /// Exact authored family and dimensions.
    pub case: Case,
    /// Population membership.
    pub phase: Phase,
    /// Zero-based iteration within that population.
    pub iteration: usize,
    /// Actual route position in the rotated iteration schedule.
    pub position: usize,
    /// Execution strategy actually called.
    pub route: Route,
    /// Host-monotonic complete checking duration, before parity comparison.
    pub elapsed_ns: u128,
    /// Complete checks, including duplicate occurrences.
    pub checked: usize,
    /// Accepted occurrences in that same ordered batch.
    pub accepted: usize,
    /// Round-source progress; absent for independent scalar/Rayon checking.
    pub source: Option<SourceWork>,
    /// Physical work; absent for every CPU route.
    pub device: Option<DeviceWork>,
}

/// Synchronous observation stream. A consumer may retain records or derive any
/// view; the experiment keeps only its current reference and result batches.
#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum Event<'a> {
    /// Full requested experiment configuration, before any worker/device setup.
    Configuration {
        /// Version of this JSON-lines view.
        schema: u32,
        /// Numeric ceilings and finite schedule.
        configuration: &'a Configuration,
        /// What the wall intervals include.
        scope: &'static str,
        /// Explicit absence of process RSS measurement.
        peak_rss: &'static str,
    },
    /// Worker/device initialization, outside case samples.
    Setup {
        /// Actual explicitly requested pool size.
        workers: usize,
        /// Pool construction interval.
        pool_init_ns: u128,
        /// Physical device name; absent in explicit CPU mode.
        adapter: Option<String>,
        /// Physical API name; absent in explicit CPU mode.
        backend: Option<String>,
        /// Device/pipeline initialization interval, if requested.
        device_init_ns: Option<u128>,
    },
    /// Fixture and reference construction, excluded from sample timers.
    Prepared {
        /// Ordinal in the caller's case list.
        case_index: usize,
        /// Admitted source program and seeds construction interval.
        fixture_ns: u128,
        /// Independent scalar reference construction interval.
        reference_ns: u128,
    },
    /// Successful measurement; no mismatch becomes a successful sample.
    Sample(Sample),
    /// A requested route failed; it is retained outside successful populations.
    Failed {
        /// Ordinal of the attempted case.
        case_index: usize,
        /// Population in which the failure occurred.
        phase: Phase,
        /// Iteration within that population.
        iteration: usize,
        /// Attempted position in the fixed schedule.
        position: usize,
        /// Execution strategy attempted.
        route: Route,
        /// Human view of the typed error returned by measure.
        message: String,
        /// Charged incomplete source progress, if available.
        source: Option<SourceWork>,
        /// Submitted physical work before failure, if this was a physical GPU route.
        device: Option<DeviceWork>,
    },
    /// All requested routes, cases and populations completed.
    Complete {
        /// Number of successful sample observations emitted.
        samples: usize,
    },
}

#[derive(Serialize)]
struct ConfigurationView<'a> {
    cases: &'a [Case],
    backend: &'static str,
    warmups: usize,
    repetitions: std::num::NonZeroUsize,
    workers: std::num::NonZeroUsize,
    #[serde(with = "CandidateLimits")]
    cpu_per_candidate: &'a zetesis_cpu::Limits,
    #[serde(with = "SourceLimits")]
    source_per_batch: &'a lazy::Limits,
    #[serde(with = "TransportLimits")]
    gpu_per_chunk: &'a zetesis_wgpu::GpuLimits,
}

#[derive(Serialize)]
#[serde(remote = "zetesis_cpu::Limits")]
struct CandidateLimits {
    max_work: u64,
    max_derived_atoms: usize,
}

#[derive(Serialize)]
#[serde(remote = "lazy::Limits")]
#[expect(
    clippy::struct_field_names,
    reason = "Serde remote fields must match the native resource limits; max_ denotes a ceiling."
)]
struct SourceLimits {
    max_candidates: usize,
    max_atoms: usize,
    max_rounds: u64,
    max_source_work: u64,
    max_chunk_rules: usize,
    max_chunk_words: usize,
    max_instance_bytes: usize,
    max_host_bytes: usize,
}

#[derive(Serialize)]
#[serde(remote = "zetesis_wgpu::GpuLimits")]
struct TransportLimits {
    max_candidates: usize,
    max_batch_bytes: u64,
    #[serde(rename = "timeout_ns", serialize_with = "duration")]
    timeout: std::time::Duration,
}

fn duration<S: Serializer>(value: &std::time::Duration, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_u128(value.as_nanos())
}

impl Serialize for Configuration {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ConfigurationView {
            cases: &self.cases,
            backend: self.backend.label(),
            warmups: self.warmups,
            repetitions: self.repetitions,
            workers: self.workers,
            cpu_per_candidate: &self.cpu_limits,
            source_per_batch: &self.source_limits,
            gpu_per_chunk: &self.gpu_limits,
        }
        .serialize(serializer)
    }
}
