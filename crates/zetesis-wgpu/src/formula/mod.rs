//! Resident original formulas and candidate-local frozen constraint propagation.

mod device;
mod packing;
mod preparation;
mod projection;
mod profile;
mod transport;

use std::time::Duration;

pub use device::GpuFormulaOracle;
pub use profile::GpuFormulaProfile;
pub use projection::GateProjection;

/// Bounds for one resident finite-formula propagation dispatch.
#[derive(Clone, Copy, Debug)]
pub struct FormulaLimits {
    /// Maximum candidate worlds, additionally bounded by device dispatch limits.
    pub max_candidates: usize,
    /// Conservative authored graph, transport, host packing and result bytes.
    /// Cold preparation first admits a minimum envelope, then the complete
    /// schedule and actual retained node/root upload-vector element capacities.
    /// These staging buffers coexist with device copies during upload and are
    /// released afterward. Seed/result host allowances remain requested sizes.
    /// Caller-owned inputs, allocator overhead and driver-private allocations
    /// or deferred retirement are excluded; this is not an RSS bound.
    pub max_batch_bytes: u64,
    /// Maximum complete propagation sweeps per candidate. Zero still checks
    /// original truth, then returns an explicit round-limit residual.
    pub max_rounds: u32,
    /// Maximum charged device work per candidate. Setup costs
    /// `2*nodes+atoms+roots+levels` for two node walks, semantic-atom
    /// initialization, root visits and dependency-level synchronization.
    /// Pure chains and empty graphs select serial truth and charge zero levels;
    /// other DAGs charge their number of occupied dependency levels.
    /// leaf nodes are visited but need no separate domain initialization store.
    /// A sweep reserves `9*nodes+atoms+65` units, including eight possible gate
    /// truth-table rows per node, 64 lane-summary merges and one strict-subset
    /// projection. These are policy units, not raw GPU instructions.
    /// Inactive nodes still consume their allowance.
    /// A budget below mandatory setup is refused before dispatch.
    pub max_work_per_candidate: u32,
    /// Maximum host wait for submitted work; not GPU preemption or a deadline
    /// for device initialization, compilation, allocation or error-scope drains.
    /// Must be positive for a nonempty batch; empty batches perform no wait.
    pub timeout: Duration,
}
impl Default for FormulaLimits {
    fn default() -> Self {
        Self {
            max_candidates: 1024,
            max_batch_bytes: 128 * 1024 * 1024,
            max_rounds: 64,
            max_work_per_candidate: 100_000_000,
            timeout: Duration::from_secs(30),
        }
    }
}

/// Why propagation leaves the exact proper-subset query unresolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualReason {
    /// A full no-change sweep left unknowns or a satisfiable query. Quiescence
    /// never proves stability or the existence of a proper-subset witness.
    FixedPoint,
    /// The configured complete-sweep ceiling was reached.
    RoundLimit,
    /// The next full sweep would exceed charged work.
    WorkLimit,
}

/// A partial decision about the original candidate and its frozen reduct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormulaVerdict {
    /// The candidate violates an original asserted root.
    NotModel,
    /// Original roots hold, and sound propagation refutes every proper subset.
    NoProperSubset,
    /// Original roots hold; exact residual search remains necessary.
    Residual(ResidualReason),
}

/// Charged work and completed sweeps for one candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormulaStatistics {
    /// Mandatory setup plus reserved per-sweep work, as specified by the limits.
    pub work: u32,
    /// Full cooperative propagation sweeps completed.
    pub rounds: u32,
}

/// Validated result for one candidate, preserving input order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormulaCheck {
    verdict: FormulaVerdict,
    statistics: FormulaStatistics,
}
impl FormulaCheck {
    /// Logical or residual result. No fixed-point result implies stability.
    #[must_use]
    pub const fn verdict(&self) -> FormulaVerdict {
        self.verdict
    }
    /// Per-candidate propagation accounting.
    #[must_use]
    pub const fn statistics(&self) -> FormulaStatistics {
        self.statistics
    }
}

/// Residency and allocation diagnostics for a successful nonempty batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormulaBatchStats {
    /// Whether this call uploaded a different immutable Theory instance.
    pub theory_uploaded: bool,
    /// Whether this call allocated its exact candidate-count transport shape.
    pub transport_allocated: bool,
    /// Requested immutable node and root buffer bytes, including padding.
    /// The root buffer also holds the dependency schedule when levels are used.
    pub resident_theory_bytes: u64,
    /// Requested parameters, candidates, masks, domains, results and readback.
    pub resident_transport_bytes: u64,
    /// Conservative authored allocation sum checked against the batch ceiling.
    pub accounted_bytes: u64,
}
