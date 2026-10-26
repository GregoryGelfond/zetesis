//! Exact batched formula membership on the GPU.
//!
//! [`GpuFormulaOracle`] propagates a theory's truth over a batch of candidates
//! and leaves the undecided ones to exact CPU completion; [`GpuTightOracle`]
//! checks complete tight support. Both use integer truth over frozen
//! candidates. A returned rejection is logical; adapter, capacity, timeout,
//! validation and device errors remain distinct failures.
//! [`GpuAggregateOracle`] and [`GpuRelationExecutor`] are bounded reusable
//! primitives; their availability does not imply that a solve uses them.
//!
//! [`GpuContext`] allows distinct primitives to retain prepared subjects on one
//! device. Existing constructors create independent contexts; `from_context`
//! constructors share one context and its failure boundary. The solve-path
//! primitives' `from_source` constructors take a [`GpuDeviceSource`]: their own
//! selection, a selection over the adapter enumeration a [`GpuDiscovery`] began
//! earlier, or an existing context. Calls are serialized without waiting for a
//! competing call: overlap returns [`GpuErrorKind::Busy`]. Each primitive's
//! resource limit keeps its documented local scope.
//!
//! The shaders implement the monotone event/iteration schedules modeled in the
//! Lean specification. The Rust-to-WGSL packing and device implementation are
//! tested refinements, not mechanically verified implementations.
#![forbid(unsafe_code)]

mod adapter;
mod aggregate;
mod candidates;
mod checked_batch;
mod checked_slate;
mod context;
mod device_source;
mod discovery;
mod formula;
mod formula_graph;
mod kernel;
mod relation;
mod runtime;
mod selection;
#[cfg(test)]
mod spirv_translation;
mod tight;
mod timing;

use std::fmt;

pub use checked_batch::CheckedBatch;
pub use checked_slate::CheckedSlate;
pub use context::GpuContext;
pub use device_source::GpuDeviceSource;
pub use discovery::GpuDiscovery;
pub use kernel::{GpuKernelOptions, GpuPropagation, GpuWorkgroupSize, GpuWorkgroupSizing};

pub use formula::{
    FormulaBatchStats, FormulaCheck, FormulaLimits, FormulaStatistics, FormulaVerdict,
    GateProjection, GpuFormulaOracle, GpuFormulaProfile, PreparedFormulaSubmission, ResidualReason,
};

pub use adapter::{AdapterBackend, AdapterCategory, AdapterMetadata};
pub use aggregate::{
    AggregateGpuActivity, AggregateGpuBatchStats, AggregateGpuCapability, AggregateGpuError,
    AggregateGpuEvaluation, AggregateGpuLimits, AggregateGpuPlan, AggregateGpuPlanLimits,
    AggregateGpuReduction, AggregateGpuValue, GpuAggregateOracle,
};
pub use relation::{
    GpuRelationExecutor, PreparedGpuRelation, RelationGpuActivity, RelationGpuError,
    RelationGpuLimits, RelationGpuMasks, RelationGpuStats,
};
pub use tight::{
    GpuTightOracle, PreparedTightSubmission, TightGpuActivity, TightGpuBatchStats, TightGpuCheck,
    TightGpuError, TightGpuLimits, TightGpuRoutes, TightGpuUpdateUnavailable, TightGpuUpdates,
    TightSupport,
};

pub use selection::{GpuInfo, GpuSelection, compiled_apis, discover_adapters};

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
