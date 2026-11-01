//! Checked original satisfaction and ranked producer support on a real device.

mod admission;
mod device;
mod packing;
mod transport;
mod update;

use crate::GpuError;
pub use device::{GpuTightOracle, PreparedTightSubmission};
use std::{fmt, time::Duration};
use zetesis_algebra::{Cancellation, Stop};
use zetesis_ferraris::TightVerdict;

const SHADER: &str = concat!(
    include_str!("check.wgsl"),
    include_str!("../formula_graph/numeric.wgsl")
);

/// Physical construction of the same complete producer-support relation.
/// This selects a library primitive; it does not change answer-set semantics or
/// ordinary solver dispatch policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TightSupport {
    /// Producers independently set enabled head bits through atomic OR.
    #[default]
    Atomic,
    /// One invocation reduces every producer belonging to a support word.
    /// This avoids contended ORs but can serialize large or uneven groups.
    Grouped,
}

/// Bounds for one complete support-checking batch, independent of search.
#[derive(Clone, Copy, Debug)]
pub struct TightGpuLimits {
    /// Candidate occurrences, also bounded by granted device dispatch limits.
    pub max_candidates: usize,
    /// Conservative authored graph, transport, packing and result payload bytes.
    /// Candidate upload scratch is charged by actual element capacity and retained
    /// only while the subject and candidate count stay unchanged.
    /// Shared caller-owned theory/certificate/candidates, allocator overhead,
    /// driver-private allocations and deferred retirement are excluded. Not RSS.
    pub max_batch_bytes: u64,
    /// Each candidate reserves `nodes + edges + roots + producers + 2 * words`
    /// operations, plus `48*n+32` for each variable threshold and `24*n+16`
    /// for each variable extremum cut with `n` condition occurrences. These
    /// conservative units cover portable limb arithmetic; constant guards carry
    /// no numeric row charge. A cooperative truth schedule adds one unit per
    /// dependency level. Serial execution remains available when that schedule
    /// cannot fit the work or cold-storage allowance. Here `edges` counts child occurrences and
    /// `words = ceil(atoms / 32)`. One word scan initializes
    /// or constructs packed support under the selected [`TightSupport`] policy;
    /// the other finds unsupported candidate bits by words. This replaces the
    /// former accounting contract; native operand reads are included.
    /// Every scan completes, including initialization and scans after an original
    /// failure; this differs from scalar early-exit work. Insufficient work is
    /// refused before dispatch, never converted into a logical result.
    ///
    /// Reusing a wholly successful batch's original truth adds
    /// `2 * nodes + edges + 4` for change detection; it runs only when that
    /// fits. The update route (see [`Self::min_update_candidates`]) is
    /// allowed only when its derived worst case and dense reuse after its
    /// change count also fit; while it is allowed, the complete and dense
    /// routes add the retained candidate's writes, `words + 1`, and dense reuse
    /// after the change count adds the count's `2 * words`. Each check then
    /// reports the work its route performed, never more than was reserved.
    pub max_work_per_candidate: u64,
    /// The fewest candidates whose per-slot update state a batch must admit
    /// before the update route is used.
    ///
    /// Once a theory's batches of one count hold reuse authority, the oracle
    /// can retain, per candidate slot, the candidate it last evaluated and that
    /// evaluation's support summaries, and then re-evaluate only the nodes a
    /// new candidate changes. That state is counted in each batch's
    /// [`Self::max_batch_bytes`]. A prefix call admits it only when the
    /// state-inclusive layout still admits at least this many candidates; an
    /// exact call admits it only when that layout admits its whole count. The
    /// state never turns an admitted batch into a refusal: without it the dense
    /// routes run as before. Zero declines the update route.
    ///
    /// Independently of this limit, the route engages only on a schedule whose
    /// dense evaluation takes at least
    /// [`GpuTightOracle::UPDATE_ENGAGEMENT_STRIDES`](crate::GpuTightOracle::UPDATE_ENGAGEMENT_STRIDES)
    /// workgroup strides per dependency level. Narrower schedules report
    /// [`TightGpuUpdateUnavailable::NarrowSchedule`]. Once engaged, the oracle's
    /// runtime guard can decline the route for a few batches at a time after a
    /// batch whose slowest update's critical path exceeded dense evaluation's
    /// ([`TightGpuUpdateUnavailable::GuardDeclined`]).
    pub min_update_candidates: usize,
    /// Bounded submission wait, with control polling at most 50 ms apart while
    /// waiting. Not GPU preemption or a bound on device creation, allocation,
    /// shader compilation or error-scope drainage.
    /// Must be positive for a nonempty batch; empty batches perform no wait.
    pub timeout: Duration,
}

impl Default for TightGpuLimits {
    fn default() -> Self {
        Self {
            max_candidates: 1024,
            max_batch_bytes: 128 * 1024 * 1024,
            max_work_per_candidate: 100_000_000,
            min_update_candidates: 1,
            timeout: Duration::from_secs(30),
        }
    }
}

/// A complete device support check for one input occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TightGpuCheck {
    verdict: TightVerdict,
    work: u64,
    critical_path: u64,
}

impl TightGpuCheck {
    /// Exact scalar certificate verdict, including its ordered witness.
    /// A residual names an unsupported present atom. A caller requiring a
    /// concrete reduct witness must complete it; a proof-based protocol may
    /// reject it using the complete certificate's support-necessity theorem.
    #[must_use]
    pub const fn verdict(&self) -> TightVerdict {
        self.verdict
    }

    /// Operations the check's route performed, under the units of
    /// [`TightGpuLimits::max_work_per_candidate`]: the complete and dense
    /// routes charge their fixed allowance, the update route what it ran.
    #[must_use]
    pub const fn work(&self) -> u64 {
        self.work
    }

    /// The update route's critical path for this check, by the runtime
    /// guard's cost table, which counts what [`Self::work`] counts save that a
    /// wide conjunction or disjunction costs the operands it read: the costliest
    /// invocation of the candidate's workgroup in each phase after the
    /// summary, and in each phase of the summary when the slot took dense reuse
    /// beyond the change threshold in its previous batch, summed, with each
    /// invocation's share of the update's uniform scans and of the record. A
    /// summary after a batch that seeded every slot is left out. Zero for every
    /// other route. Deterministic for the theory, the schedule and the
    /// candidates.
    #[must_use]
    pub const fn critical_path(&self) -> u64 {
        self.critical_path
    }
}

/// Resource evidence for the last successful nonempty batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct TightGpuBatchStats {
    /// A different immutable theory required graph upload.
    pub theory_uploaded: bool,
    /// Exact candidate-count transport was allocated or replaced.
    pub transport_allocated: bool,
    /// Requested immutable header/operand, root and producer buffer bytes, with
    /// padding; the producer buffer includes a built readers index.
    pub resident_theory_bytes: u64,
    /// Requested uniform, candidate, truth, support, result and readback
    /// bytes, and the per-slot update state once admitted: bound when the
    /// update route is allowed, and otherwise allocated but unbound, on the
    /// kernel without the update route.
    pub resident_transport_bytes: u64,
    /// Conservative authored payload checked against the batch byte ceiling.
    pub accounted_bytes: u64,
    /// Fully checked input occurrences, preserving repetitions.
    pub candidates: u64,
    /// Sum of the operations each candidate's route performed.
    pub work: u64,
    /// Actual compute submissions; one for this nonempty successful batch.
    pub dispatches: u64,
    /// Dependency levels evaluated cooperatively; zero selects serial original truth.
    pub truth_levels: u32,
    /// The workgroup size dispatched, in invocations per candidate: the fixed
    /// size, or the size the theory's dependency schedule fits.
    pub workgroup_size: u32,
    /// A wholly successful same-subject/count batch supplied original truth.
    /// This selects change detection; it does not assert that any gate was skipped.
    pub original_truth_reused: bool,
    /// Candidate slots per route.
    pub routes: TightGpuRoutes,
    /// Nodes re-evaluated by update-route slots, summed over the batch.
    pub nodes_evaluated: u64,
    /// The largest critical path among the batch's update slots (see
    /// [`TightGpuCheck::critical_path`]), or zero without update slots: the
    /// runtime guard's measure of the update route.
    pub update_critical_path: u64,
    /// Dense reuse's least critical path for one candidate at the dispatched
    /// workgroup size, by the same cost table as an update's: each dependency
    /// level's and each scan's busiest invocation, summed, where every
    /// candidate-dependent cost takes its least. The runtime guard declines
    /// the route after a batch whose `update_critical_path` exceeds it. Zero
    /// for a schedule the update route does not engage.
    pub dense_critical_path: u64,
    /// Whether the update route was available to this batch, and why not.
    pub updates: TightGpuUpdates,
    /// Authored initialized/write-buffer payload, not measured bus traffic,
    /// including the rebuilt producer buffer of a batch that builds the index.
    pub uploaded_bytes: u64,
    /// Successfully decoded readback payload, not measured bus traffic.
    pub downloaded_bytes: u64,
}

/// Candidate slots of one batch by the route that evaluated them.
///
/// Each slot takes exactly one route. The complete route evaluates everything;
/// dense reuse re-evaluates every node against a retained evaluation; the
/// update route re-evaluates only the nodes a candidate's change reaches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct TightGpuRoutes {
    /// Slots without reuse authority.
    pub complete: u64,
    /// Slots that reused a retained evaluation and re-evaluated every node.
    pub dense_reuse: u64,
    /// Dense-reuse slots that first counted their change and found it beyond
    /// the update route's threshold; included in `dense_reuse`.
    pub beyond_threshold: u64,
    /// Slots that brought a retained evaluation to their new candidate.
    pub update: u64,
}

/// The update route's availability to one batch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum TightGpuUpdates {
    /// The batch held no reuse authority at its stateless count, which the
    /// route needs before it builds the theory's readers index. A fresh
    /// theory's first batch reports this unless its schedule or limits
    /// already decline the route. The batch ran the kernel without the update
    /// route.
    #[default]
    Pending,
    /// Per-slot state was bound and the route was allowed: the batch ran the
    /// kernel with the update route.
    Admitted {
        /// Bits of each per-head support count: 8, 16 or 32.
        count_bits: u32,
        /// This batch uploaded the theory's readers index.
        index_built: bool,
    },
    /// The route did not run in this batch, for the given reason; the batch
    /// ran the kernel without the update route.
    Unavailable(TightGpuUpdateUnavailable),
}

/// Why the update route was unavailable to a batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TightGpuUpdateUnavailable {
    /// The batch evaluates original truth serially, which has no levels.
    SerialSchedule,
    /// Dense evaluation of the resident schedule takes fewer than
    /// [`GpuTightOracle::UPDATE_ENGAGEMENT_STRIDES`](crate::GpuTightOracle::UPDATE_ENGAGEMENT_STRIDES)
    /// workgroup strides per dependency level on average, below which the
    /// route is not expected to pay. Decided once per resident theory and
    /// workgroup size.
    NarrowSchedule,
    /// A prefix call admits fewer candidates than
    /// [`TightGpuLimits::min_update_candidates`] before any state is
    /// considered: its input or its `max_candidates` is shorter.
    CandidateCeiling,
    /// The theory exceeds the readers index's intrinsic bounds; this holds
    /// for as long as the theory stays resident.
    IndexBounds,
    /// The state-inclusive layout did not admit enough candidates within the
    /// batch's byte allowance.
    Bytes,
    /// The route's worst case did not fit the work allowance.
    Work,
    /// [`TightGpuLimits::min_update_candidates`] was zero.
    Declined,
    /// The oracle's runtime guard declined the route for this batch, which ran
    /// the complete or dense routes on the kernel without the update route,
    /// its state allocated and accounted. The guard decides which route
    /// evaluates a batch, never a verdict.
    GuardDeclined,
}

/// Evidence retained even when a batch fails. Reset at each new check.
///
/// Submitted work is a scheduled allowance, not a claim that an interrupted GPU
/// completed it. Completion counters require a validated full readback. Upload
/// bytes count authored initialization/write calls, not driver bus measurements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TightGpuActivity {
    /// Actual compute queue submissions.
    pub submissions: u64,
    /// Input occurrences included in those submissions.
    pub submitted_candidates: u64,
    /// Work allowance reserved by submitted commands: each candidate's
    /// allowance for the routes it may take.
    pub scheduled_work: u64,
    /// Occurrences whose complete records were validated before return.
    pub completed_candidates: u64,
    /// Work the validated device records report as performed.
    pub completed_work: u64,
    /// Authored initialization/write-buffer bytes issued in this attempt.
    pub uploaded_bytes: u64,
    /// Readback bytes whose complete records were validated.
    pub downloaded_bytes: u64,
}

/// An incomplete invocation or device failure; never a membership verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TightGpuError {
    /// Shared cancellation or deadline was observed.
    Stopped(Stop),
    /// Adapter, identity, capacity, allocation, execution or readback failure.
    Gpu(GpuError),
}

impl fmt::Display for TightGpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopped(stop) => stop.fmt(f),
            Self::Gpu(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for TightGpuError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Stopped(stop) => stop,
            Self::Gpu(error) => error,
        })
    }
}
impl From<GpuError> for TightGpuError {
    fn from(error: GpuError) -> Self {
        match error.interruption {
            Some(stop) => Self::Stopped(stop),
            None => Self::Gpu(error),
        }
    }
}

fn poll(cancellation: &Cancellation) -> Result<(), GpuError> {
    cancellation.poll().map_err(|stop| GpuError {
        kind: crate::GpuErrorKind::Device,
        detail: "tight-support operation interrupted".to_owned(),
        interruption: Some(stop),
    })
}

#[cfg(test)]
mod tests;
