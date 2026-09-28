//! Exact batched reduct checking with static and lazy source executors.
//!
//! Static workgroups own complete candidate closures. [`GpuLazyOracle`] instead
//! evaluates bounded source instances over immutable per-world snapshots, with
//! host joins and round barriers. Both use integer truth and frozen seed gates.
//! A returned rejection is logical; adapter, capacity, timeout, validation and
//! device errors remain distinct failures.
//!
//! [`GpuContext`] allows distinct primitives to retain prepared subjects on one
//! device. Existing constructors create independent contexts; `from_context`
//! constructors share one context and its failure boundary. Calls are serialized
//! without waiting for a competing call: overlap returns [`GpuErrorKind::Busy`].
//! Each primitive's resource limit keeps its documented local scope.
//!
//! The shader implements the monotone event/iteration schedules modeled in the
//! Lean specification. The Rust-to-WGSL packing and device implementation are
//! tested refinements, not mechanically verified implementations.
#![forbid(unsafe_code)]

mod formula;
mod packing;
mod residency;
mod runtime;
mod context;
mod selection;
mod adapter;
mod lazy;
mod tight;
mod aggregate;
mod relation;

use std::fmt;
use std::time::Duration;

use zetesis_core::{GroundProgram, Seed, SeedView};
use zetesis_cpu::Cancellation;

use packing::{BatchPlan, GraphPlan, PackedGraph, PackedSeeds};
use residency::ResidentGraph;
use runtime::{DeviceProfile, ErrorScopes, Runtime};

pub use context::GpuContext;

pub use formula::{
    FormulaBatchStats, FormulaCheck, FormulaLimits, FormulaStatistics, FormulaVerdict,
    GateProjection, GpuFormulaOracle, GpuFormulaProfile, ResidualReason,
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

pub use selection::{GpuInfo, GpuSelection, compiled_apis, discover_adapters};

/// Maximum dense atoms in this shader's workgroup closure bitset.
pub const MAX_ATOMS: usize = 4096;
/// Invocations sharing ownership of one frozen candidate.
pub const WORKGROUP_SIZE: u32 = 64;
const SHADER: &str = include_str!("oracle.wgsl");
const UNIFORM_BYTES: u64 = 32;
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
    /// Maximum host wait for the submitted GPU work. A timeout invalidates the
    /// shared device context and returns an error, never a candidate rejection.
    /// Must be positive for a nonempty batch; empty batches perform no wait.
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
    /// Another operation holds this shared context; retry after it completes.
    /// No work was submitted and context health remains unchanged.
    Busy,
    /// A candidate does not belong to this compiled program.
    Seed,
    /// Shader or command validation failed.
    Validation,
    /// Device creation, execution, or a device callback reported failure.
    Device,
    /// The bounded host wait expired.
    Timeout,
    /// Caller cancellation or deadline was observed; no complete batch is returned.
    Interrupted,
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
            kind: GpuErrorKind::Interrupted,
            detail: stop.to_string(),
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

    /// Exact control stop, when this error reports an interrupted operation.
    /// A preceding scope/device fault replaces the stop and returns `None`.
    #[must_use]
    pub const fn interruption(&self) -> Option<zetesis_cpu::Stop> {
        self.interruption
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
/// `check_batch` takes an exclusive borrow and leases its context through
/// submission/readback completion. Candidate worlds share immutable rule data
/// but never share closure latches.
pub struct GpuOracle {
    runtime: Runtime,
    resident: Option<ResidentGraph>,
    last_batch_stats: Option<GpuBatchStats>,
    epoch: u32,
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
                api: zetesis_backend::GpuApi::Metal,
            },
        )
    }

    /// Select an adapter of the requested API and validate its compute device.
    /// The requested API is never silently replaced by another.
    ///
    /// # Errors
    /// Returns typed adapter absence, policy refusal, capability, device,
    /// allocation, or validation errors. CPU reference fallback is a caller policy.
    pub fn new_selected(options: GpuOptions, selection: GpuSelection) -> Result<Self, GpuError> {
        let runtime = pollster::block_on(Runtime::new(options, selection, Self::profile()))?;
        Ok(Self::with_runtime(runtime))
    }

    /// Compile this primitive on an existing device context.
    ///
    /// Prepared subjects belong to this primitive; device health and execution
    /// serialization are shared with all primitives using the context.
    ///
    /// # Errors
    /// Refuses an active or invalidated context, unsupported granted limits,
    /// allocation and shader validation failure. No adapter selection occurs.
    pub fn from_context(context: &crate::GpuContext) -> Result<Self, GpuError> {
        let runtime = pollster::block_on(Runtime::from_context(context, Self::profile()))?;
        Ok(Self::with_runtime(runtime))
    }

    /// Exact device context retained by this primitive.
    #[must_use]
    pub fn context(&self) -> &crate::GpuContext {
        &self.runtime.context
    }

    fn with_runtime(runtime: Runtime) -> Self {
        Self {
            runtime,
            resident: None,
            last_batch_stats: None,
            epoch: 0,
        }
    }

    fn profile() -> DeviceProfile {
        DeviceProfile {
            device_label: "zetesis static reduct oracle",
            shader_label: "zetesis exact integer reduct",
            pipeline_label: "zetesis static batch",
            shader: SHADER.into(),
            entry_point: "check",
            validate_limits: check_adapter_limits,
        }
    }

    /// Identity of the adapter actually selected for this oracle.
    #[must_use]
    pub fn info(&self) -> &GpuInfo {
        self.runtime.context.info()
    }

    /// Reuse diagnostics for the last successful nonempty batch. Cleared when
    /// any new check starts, including a failed or empty check.
    #[must_use]
    pub fn last_batch_stats(&self) -> Option<&GpuBatchStats> {
        self.last_batch_stats.as_ref()
    }

    /// Release this oracle's retained graph and transport handles. Driver
    /// retirement may occur later; this does not cancel submitted GPU work.
    /// Submission identities are not reset.
    pub fn clear_residency(&mut self) {
        self.resident = None;
        self.last_batch_stats = None;
    }

    /// Validate, pack, dispatch, and read back a bounded static candidate batch.
    /// Results preserve the input seed order. No source grounding occurs here.
    /// One immutable Program instance remains resident between calls; changing
    /// Program identity replaces it. Transport is reused for equal batch sizes
    /// and replaced for a changed size. Completed reads are unmapped before reuse.
    /// Every returned record must identify its submission and input position,
    /// carry a nonzero completion marker, and agree with the seed projection.
    /// Per candidate, host validation visits the closure words and gate atoms;
    /// the original gate IDs are borrowed without allocating another mask.
    /// An empty batch validates device health and graph admission, then returns
    /// without checking dispatch limits, allocating transport or advancing the
    /// submission epoch. It leaves existing residency intact.
    ///
    /// # Errors
    /// Reports seed identity, capacity, allocation, device, timeout, validation,
    /// or readback failures. A failing dispatch returns no partial batch.
    /// Device/readback failures invalidate the entire context; retry requires a
    /// fresh context. Capacity, seed and Busy refusals leave the context reusable.
    /// The checked 32-bit submission sequence never wraps; exhausting it requires
    /// a new oracle, even after clearing residency.
    pub fn check_batch(
        &mut self,
        program: &GroundProgram,
        seeds: &[Seed],
        limits: GpuLimits,
    ) -> Result<Vec<GpuCheck>, GpuError> {
        self.check_batch_views(program, seeds.iter().map(Seed::view), limits)
    }

    /// Check owned seeds with caller cancellation and deadline observation.
    ///
    /// # Errors
    /// Preserves [`Self::check_batch_views_with_cancellation`]'s failure contract.
    pub fn check_batch_with_cancellation(
        &mut self,
        program: &GroundProgram,
        seeds: &[Seed],
        limits: GpuLimits,
        cancellation: &Cancellation,
    ) -> Result<Vec<GpuCheck>, GpuError> {
        self.check_batch_views_with_cancellation(
            program,
            seeds.iter().map(Seed::view),
            limits,
            cancellation,
        )
    }

    /// Check the same static candidate occurrences through borrowed seed views.
    /// The cloneable exact-size iterator preserves input order across admission
    /// and packing. Wrapping owned seeds or shared selections allocates no view
    /// vector and copies no atom payload. No source grounding occurs here.
    /// Residency, submission identity and independent projection validation are
    /// the same as [`Self::check_batch`].
    ///
    /// # Errors
    /// Preserves [`Self::check_batch`]'s identity, capacity and device failures.
    pub fn check_batch_views<'seed>(
        &mut self,
        program: &GroundProgram,
        seeds: impl ExactSizeIterator<Item = SeedView<'seed>> + Clone,
        limits: GpuLimits,
    ) -> Result<Vec<GpuCheck>, GpuError> {
        self.check_batch_views_with_cancellation(program, seeds, limits, &Cancellation::default())
    }

    /// Check borrowed seeds with cooperative caller control. Busy admission
    /// precedes control; control precedes health and graph admission, even for
    /// empty batches. Polls also follow host packing and occur between device
    /// waits of at most 50 ms. Host operations, drivers and scope settlement are
    /// not preempted, so this is not a hard wall-clock deadline.
    ///
    /// # Errors
    /// Returns [`GpuErrorKind::Interrupted`] with the exact stop and no partial
    /// batch. Pre-submission stops leave health reusable; an interrupted wait
    /// invalidates it. Scope/device faults retain priority over interruption.
    /// Other failures follow [`Self::check_batch`].
    pub fn check_batch_views_with_cancellation<'seed>(
        &mut self,
        program: &GroundProgram,
        seeds: impl ExactSizeIterator<Item = SeedView<'seed>> + Clone,
        limits: GpuLimits,
        cancellation: &Cancellation,
    ) -> Result<Vec<GpuCheck>, GpuError> {
        self.last_batch_stats = None;
        let context = self.runtime.context.clone();
        let _lease = context.lease()?;
        cancellation.poll().map_err(GpuError::interrupted)?;
        self.runtime.check_health()?;
        let fresh_graph = if self
            .resident
            .as_ref()
            .is_some_and(|resident| resident.plan.matches(program))
        {
            None
        } else {
            Some(GraphPlan::new(program, self.runtime.limits())?)
        };
        let graph = fresh_graph
            .as_ref()
            .or_else(|| self.resident.as_ref().map(|resident| &resident.plan))
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing graph plan"))?;
        let candidate_count = seeds.len();
        if candidate_count == 0 {
            return Ok(Vec::new());
        }
        let epoch = packing::next_epoch(self.epoch)?;
        let plan = BatchPlan::for_graph(
            graph,
            candidate_count,
            limits,
            self.runtime.limits(),
            fresh_graph.is_some(),
            epoch,
        )?;
        // Refuse foreign seeds before replacing resident resources or packing.
        if seeds
            .clone()
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
        cancellation.poll().map_err(GpuError::interrupted)?;
        let scopes = ErrorScopes::new(self.runtime.device());
        if let Some((graph, packed)) = fresh_graph.zip(packed_graph) {
            self.resident = Some(ResidentGraph::new(self.runtime.device(), graph, &packed));
        }
        // Never reuse an identity after a dispatch attempt, including one whose
        // readback fails. Clearing residency does not reset this sequence.
        self.epoch = epoch.get();
        let outcome = self
            .resident
            .as_mut()
            .ok_or_else(|| GpuError::new(GpuErrorKind::Device, "missing resident graph"))
            .and_then(|resident| {
                resident.dispatch(&self.runtime, &packed, &plan, limits.timeout, cancellation)
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
    fn static_parameters_match_the_uniform_layout() {
        let module = naga::front::wgsl::parse_str(super::SHADER).expect("WGSL parses");
        let (_, params) = module
            .types
            .iter()
            .find(|(_, value)| value.name.as_deref() == Some("Params"))
            .expect("static parameter type");
        let naga::TypeInner::Struct { members, span } = &params.inner else {
            panic!("parameters are a uniform struct");
        };
        assert_eq!(u64::from(*span), super::UNIFORM_BYTES);
        let fields = members
            .iter()
            .map(|member| (member.name.as_deref(), member.offset))
            .collect::<Vec<_>>();
        assert_eq!(
            fields,
            [
                (Some("atom_count"), 0),
                (Some("word_count"), 4),
                (Some("rule_count"), 8),
                (Some("world_count"), 12),
                (Some("epoch"), 16),
                (Some("reserved0"), 20),
                (Some("reserved1"), 24),
                (Some("reserved2"), 28),
            ]
        );
    }

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
