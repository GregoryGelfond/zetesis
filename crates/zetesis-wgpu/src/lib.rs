//! Exact batched reduct checking with static and lazy source executors.
//!
//! Static workgroups own complete candidate closures. [`GpuLazyOracle`] instead
//! evaluates bounded source instances over immutable per-world snapshots, with
//! host joins and round barriers. Both use integer truth and frozen seed gates.
//! A returned rejection is logical; adapter, capacity, timeout, validation and
//! device errors remain distinct failures.
//!
//! The shader implements the monotone event/iteration schedules modeled in the
//! Lean specification. The Rust-to-WGSL packing and device implementation are
//! tested refinements, not mechanically verified implementations.
#![forbid(unsafe_code)]

mod formula;
mod packing;
mod residency;
mod runtime;
mod selection;
mod adapter;
mod lazy;
mod tight;
mod aggregate;
mod relation;

use std::fmt;
use std::time::Duration;

use zetesis_core::{GroundProgram, Seed};

use packing::{BatchPlan, GraphPlan, PackedGraph, PackedSeeds};
use residency::ResidentGraph;
use runtime::{DeviceProfile, ErrorScopes, Runtime};

pub use formula::{
    FormulaBatchStats, FormulaCheck, FormulaLimits, FormulaStatistics, FormulaVerdict,
    GateProjection, GpuFormulaOracle, ResidualReason,
};

pub use adapter::{AdapterBackend, AdapterCategory, AdapterMetadata};
pub use aggregate::{
    AggregateGpuActivity, AggregateGpuBatchStats, AggregateGpuCapability, AggregateGpuError,
    AggregateGpuEvaluation, AggregateGpuLimits, AggregateGpuPlan, AggregateGpuPlanLimits,
    AggregateGpuReduction, AggregateGpuValue, GpuAggregateOracle,
};
pub use lazy::{
    GpuLazyOracle, LazyBufferUsage, LazyGpuStatistics, LazyTransportReplacements,
    LazyTransportUsage,
};
pub use relation::{
    GpuRelationExecutor, PreparedGpuRelation, RelationGpuActivity, RelationGpuError,
    RelationGpuLimits, RelationGpuMasks, RelationGpuStats,
};
pub use tight::{
    GpuTightOracle, TightGpuActivity, TightGpuBatchStats, TightGpuCheck, TightGpuError,
    TightGpuLimits, TightSupport,
};

pub use selection::{
    GpuBackendPreference, GpuInfo, GpuSelection, NVIDIA_VENDOR_ID, compiled_backends,
    discover_adapters,
};

/// Maximum dense atoms in this shader's workgroup closure bitset.
pub const MAX_ATOMS: usize = 4096;
/// Invocations sharing ownership of one frozen candidate.
pub const WORKGROUP_SIZE: u32 = 64;
const SHADER: &str = include_str!("oracle.wgsl");
const UNIFORM_BYTES: u64 = 16;
const WORKGROUP_BYTES: u32 = 524;

/// Adapter admission policy. Defaults to requiring a physical GPU category.
#[derive(Clone, Copy, Debug)]
pub struct GpuOptions {
    /// Admit only adapters reported as integrated or discrete GPUs. CPU,
    /// virtual, and unknown categories are refused under this policy.
    pub require_gpu: bool,
}

impl Default for GpuOptions {
    fn default() -> Self {
        Self { require_gpu: true }
    }
}

/// Per-dispatch limits, checked before packing or creating GPU buffers.
#[derive(Clone, Copy, Debug)]
pub struct GpuLimits {
    /// Maximum candidates in one dispatch, additionally bounded by the device.
    pub max_candidates: usize,
    /// Maximum requested bytes for resident GPU buffers, new host packing,
    /// returned closure arrays, and their metadata. Every nonempty call counts its
    /// resident graph and transport, even when reused. Existing caller-owned
    /// objects, allocator rounding, and wgpu/driver-private allocations and
    /// deferred resource retirement are outside this sum.
    pub max_batch_bytes: u64,
    /// Maximum host wait for the submitted GPU work. A timeout invalidates this
    /// oracle instance and returns an error, never a candidate rejection.
    pub timeout: Duration,
}

impl Default for GpuLimits {
    fn default() -> Self {
        Self {
            max_candidates: 1024,
            max_batch_bytes: 128 * 1024 * 1024,
            timeout: Duration::from_secs(30),
        }
    }
}

/// A failure outside the logical accept/reject relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuErrorKind {
    /// No compatible adapter could be obtained.
    AdapterUnavailable,
    /// The adapter did not satisfy the caller's hardware policy.
    AdapterRefused,
    /// A static dimension or caller/device resource limit was exceeded.
    Capacity,
    /// Host or device memory could not be allocated.
    Allocation,
    /// A candidate does not belong to this compiled program.
    Seed,
    /// Shader or command validation failed.
    Validation,
    /// Device creation, execution, or a device callback reported failure.
    Device,
    /// The bounded host wait expired.
    Timeout,
    /// Readback mapping, shape, status, or tail-bit validation failed.
    Readback,
}

/// Typed backend failure with explanatory detail; never a logical rejection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuError {
    kind: GpuErrorKind,
    detail: String,
    interruption: Option<zetesis_cpu::Stop>,
}

impl GpuError {
    fn new(kind: GpuErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
            interruption: None,
        }
    }

    fn interrupted(stop: zetesis_cpu::Stop) -> Self {
        Self {
            kind: GpuErrorKind::Device,
            detail: "lazy readback interrupted; in-flight lifecycle invalidated".to_owned(),
            interruption: Some(stop),
        }
    }

    /// Machine-readable failure class.
    #[must_use]
    pub fn kind(&self) -> GpuErrorKind {
        self.kind
    }

    /// Context identifying the failed operation or dimension.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for GpuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for GpuError {}

/// One complete static reduct closure and its exact gate/constraint verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuCheck {
    closure_words: Vec<u32>,
    status: u32,
}

impl GpuCheck {
    /// True exactly when the closure's gate-carrier projection equals the
    /// seed and every enabled constraint is false.
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.status == 0
    }

    /// Whether an enabled constraint's positive body holds in the closure.
    #[must_use]
    pub fn constraint_violated(&self) -> bool {
        self.status & 1 != 0
    }

    /// Whether the closure's projection differs from the frozen seed.
    #[must_use]
    pub fn seed_mismatch(&self) -> bool {
        self.status & 2 != 0
    }

    /// Dense closure words in this `GroundProgram`'s atom order. The caller
    /// may reconstruct a symbolic model through `GroundProgram::model_from_words`.
    #[must_use]
    pub fn closure_words(&self) -> &[u32] {
        &self.closure_words
    }
}

/// Resource reuse and accounted memory for the last successful nonempty batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpuBatchStats {
    /// Whether this call packed and uploaded a new immutable graph.
    pub graph_uploaded: bool,
    /// Whether this call allocated transport buffers and their bind group.
    pub transport_allocated: bool,
    /// Requested bytes of the retained rule, antecedent, and carrier buffers.
    pub resident_graph_bytes: u64,
    /// Requested bytes of retained parameter, seed, result, and readback buffers.
    pub resident_transport_bytes: u64,
    /// Conservative authored allocation sum checked against `max_batch_bytes`.
    pub accounted_batch_bytes: u64,
}

/// Reusable device and pipeline for the static reduct profile.
///
/// `check_batch` takes an exclusive borrow so one instance has one active
/// submission/readback lifecycle. Candidate worlds share immutable rule data
/// but never share closure latches.
pub struct GpuOracle {
    runtime: Runtime,
    resident: Option<ResidentGraph>,
    last_batch_stats: Option<GpuBatchStats>,
}

impl GpuOracle {
    /// Create the actual adapter/device and validate the compute pipeline.
    ///
    /// # Errors
    /// Returns a typed adapter, capability, device, allocation, or shader
    /// validation failure. It never substitutes the CPU reference oracle.
    pub fn new(options: GpuOptions) -> Result<Self, GpuError> {
        Self::new_selected(options, GpuSelection::default())
    }

    /// Create an oracle using only the Metal backend, with no API fallback.
    /// The returned adapter identity is checked to report Metal explicitly.
    ///
    /// # Errors
    /// Returns the same failure classes as [`Self::new`], including adapter
    /// absence on platforms or execution environments without accessible Metal.
    pub fn new_metal(options: GpuOptions) -> Result<Self, GpuError> {
        Self::new_selected(
            options,
            GpuSelection {
                backend: GpuBackendPreference::Metal,
                ..GpuSelection::default()
            },
        )
    }

    /// Select an adapter using explicit backend/vendor filters and validate its
    /// compute device. Auto selects among compatible native wgpu backends;
    /// an explicit backend or vendor is never silently replaced.
    ///
    /// # Errors
    /// Returns typed adapter absence, policy refusal, capability, device,
    /// allocation, or validation errors. CPU reference fallback is a caller policy.
    pub fn new_selected(options: GpuOptions, selection: GpuSelection) -> Result<Self, GpuError> {
        let runtime = pollster::block_on(Runtime::new(
            options,
            selection,
            DeviceProfile {
                device_label: "zetesis static reduct oracle",
                shader_label: "zetesis exact integer reduct",
                pipeline_label: "zetesis static batch",
                shader: SHADER.into(),
                entry_point: "check",
                validate_limits: check_adapter_limits,
            },
        ))?;
        Ok(Self {
            runtime,
            resident: None,
            last_batch_stats: None,
        })
    }

    /// Identity of the adapter actually selected for this oracle.
    #[must_use]
    pub fn info(&self) -> &GpuInfo {
        &self.runtime.info
    }

    /// Reuse diagnostics for the last successful nonempty batch. Cleared when
    /// any new check starts, including a failed or empty check.
    #[must_use]
    pub fn last_batch_stats(&self) -> Option<&GpuBatchStats> {
        self.last_batch_stats.as_ref()
    }

    /// Release this oracle's retained graph and transport handles. Driver
    /// retirement may occur later; this does not cancel submitted GPU work.
    pub fn clear_residency(&mut self) {
        self.resident = None;
        self.last_batch_stats = None;
    }

    /// Validate, pack, dispatch, and read back a bounded static candidate batch.
    /// Results preserve the input seed order. No source grounding occurs here.
    /// One immutable Program instance remains resident between calls; changing
    /// Program identity replaces it. Transport is reused for equal batch sizes
    /// and replaced for a changed size. Completed reads are unmapped before reuse.
    ///
    /// # Errors
    /// Reports seed identity, capacity, allocation, device, timeout, validation,
    /// or readback failures. A failing dispatch returns no partial batch.
    /// Device/readback failures invalidate this instance; create a fresh oracle
    /// before retrying. Capacity and seed errors leave it reusable.
    pub fn check_batch(
        &mut self,
        program: &GroundProgram,
        seeds: &[Seed],
        limits: GpuLimits,
    ) -> Result<Vec<GpuCheck>, GpuError> {
        self.last_batch_stats = None;
        self.runtime.check_health()?;
        let fresh_graph = if self
            .resident
            .as_ref()
            .is_some_and(|resident| resident.plan.matches(program))
        {
            None
        } else {
            Some(GraphPlan::new(program, &self.runtime.limits)?)
        };
        let graph = fresh_graph
            .as_ref()
            .or_else(|| self.resident.as_ref().map(|resident| &resident.plan))
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing graph plan"))?;
        let plan = BatchPlan::for_graph(
            graph,
            seeds.len(),
            limits,
            &self.runtime.limits,
            fresh_graph.is_some(),
        )?;
        if seeds.is_empty() {
            return Ok(Vec::new());
        }
        // Refuse foreign seeds before replacing resident resources or packing.
        if seeds
            .iter()
            .any(|seed| !program.program().same_instance(seed.program()))
        {
            return Err(GpuError::new(
                GpuErrorKind::Seed,
                "candidate belongs to another Program",
            ));
        }
        let stats = GpuBatchStats {
            graph_uploaded: fresh_graph.is_some(),
            transport_allocated: fresh_graph.is_some()
                || self
                    .resident
                    .as_ref()
                    .is_none_or(|resident| !resident.has_transport(&plan)),
            resident_graph_bytes: graph.resident_bytes,
            resident_transport_bytes: plan.transport_bytes,
            accounted_batch_bytes: plan.accounted_bytes,
        };
        // Evict before new host/device allocations so the old authored buffers
        // cannot silently enlarge this call's accounted resident working set.
        if fresh_graph.is_some() {
            self.resident = None;
        } else if stats.transport_allocated
            && let Some(resident) = self.resident.as_mut()
        {
            resident.clear_transport();
        }
        let packed = PackedSeeds::new(program, seeds, &plan)?;
        let packed_graph = fresh_graph
            .as_ref()
            .map(|graph| PackedGraph::new(program, graph))
            .transpose()?;
        let scopes = ErrorScopes::new(&self.runtime.device);
        if let Some((graph, packed)) = fresh_graph.zip(packed_graph) {
            self.resident = Some(ResidentGraph::new(&self.runtime.device, graph, &packed));
        }
        let outcome = self
            .resident
            .as_mut()
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing resident graph"))
            .and_then(|resident| {
                resident.dispatch(
                    &self.runtime.device,
                    &self.runtime.queue,
                    &self.runtime.pipeline,
                    &packed,
                    &plan,
                    limits.timeout,
                )
            });
        let result = self.runtime.complete(scopes, outcome);
        if result.is_ok() {
            self.last_batch_stats = Some(stats);
        }
        result
    }
}

fn check_adapter_limits(limits: &wgpu::Limits) -> Result<(), GpuError> {
    for (name, required, available) in [
        (
            "workgroup invocations",
            WORKGROUP_SIZE,
            limits.max_compute_invocations_per_workgroup,
        ),
        (
            "workgroup width",
            WORKGROUP_SIZE,
            limits.max_compute_workgroup_size_x,
        ),
        (
            "workgroup bytes",
            WORKGROUP_BYTES,
            limits.max_compute_workgroup_storage_size,
        ),
        (
            "storage bindings",
            5,
            limits.max_storage_buffers_per_shader_stage,
        ),
        (
            "uniform bindings",
            1,
            limits.max_uniform_buffers_per_shader_stage,
        ),
        ("bindings per group", 6, limits.max_bindings_per_bind_group),
    ] {
        if available < required {
            return Err(GpuError::new(
                GpuErrorKind::Capacity,
                format!("{name}: need {required}, device provides {available}"),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn wgsl_validates_without_optional_shader_capabilities() {
        let module = naga::front::wgsl::parse_str(super::SHADER).expect("WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("workgroup barriers and all shader expressions validate");
        let entry = module
            .entry_points
            .iter()
            .find(|entry| entry.name == "check")
            .expect("oracle entry point");
        assert_eq!(entry.workgroup_size, [super::WORKGROUP_SIZE, 1, 1]);
        let (_, known) = module
            .global_variables
            .iter()
            .find(|(_, variable)| variable.name.as_deref() == Some("known"))
            .expect("closure latches");
        let naga::TypeInner::Array {
            size: naga::ArraySize::Constant(length),
            stride,
            ..
        } = &module.types[known.ty].inner
        else {
            panic!("closure storage is a fixed array");
        };
        assert_eq!(*stride, 4);
        assert_eq!(
            usize::try_from(length.get()).expect("small array") * 32,
            super::MAX_ATOMS
        );
    }
}
