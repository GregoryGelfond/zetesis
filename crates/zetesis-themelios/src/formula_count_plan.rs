//! Optional source-derived count consequences for candidate generation.
//!
//! Complete source head groups supply the premises. Neither Boolean pattern
//! recognition nor a caller-populated premise record certifies this boundary.
//! Every restriction is separate from the immutable original reduct subject.

use std::fmt;

use crate::ProgramSite;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{AggregateLimits, Theory, partition};

mod capture;
mod derive;
mod emit;
pub(crate) use capture::{Bounds, Collector, Input};

/// Independent limits for optional capture, partition selection and emission.
/// Zero never means unlimited; ordinary source grounding has separate budgets.
#[derive(Clone, Copy, Debug)]
pub struct CountPlanLimits {
    /// Retained applicable grounded count groups.
    pub max_groups: usize,
    /// Total retained semantic head occurrences across groups.
    pub max_members: usize,
    /// Copied source origins across captured groups and emitted consequences.
    pub max_origins: usize,
    /// Attempted global-group partitions, including unsuccessful covers.
    pub max_attempts: usize,
    /// Conservative cumulative logical allocation payload across optional work.
    /// Existing source/theory/atom storage, allocator metadata and excess
    /// allocator capacity are excluded. Source validation transfers a stack-only
    /// certificate; optional capture retains only members and origins.
    pub max_bytes: u64,
    /// Cumulative capture, matching, partition and emission operations.
    pub max_work: u64,
    /// Per-attempt complete partition validation, also capped by remaining work.
    pub partition: partition::Limits,
    /// Final candidate restriction storage, independent of the original theory.
    pub theory: zetesis_ferraris::AdmissionLimits,
    /// Per-consequence exact count lowering, capped by remaining work/nodes.
    pub aggregate: AggregateLimits,
}
impl Default for CountPlanLimits {
    fn default() -> Self {
        Self {
            max_groups: 1_024,
            max_members: 65_536,
            max_origins: 16_384,
            max_attempts: 1_024,
            max_bytes: 64 * 1024 * 1024,
            max_work: 10_000_000,
            partition: partition::Limits::default(),
            theory: zetesis_ferraris::AdmissionLimits::default(),
            aggregate: AggregateLimits::default(),
        }
    }
}

/// Optional planning resource with an inclusive ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CountPlanResource {
    /// Captured complete groups.
    Groups,
    /// Captured semantic head occurrences.
    Members,
    /// Copied source locations.
    Origins,
    /// Attempted partitions.
    Attempts,
    /// Conservatively charged logical storage.
    Bytes,
    /// Charged planning work.
    Work,
}

/// Incomplete optional work; no variant denotes source refusal or logical UNSAT.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CountPlanFailureKind {
    /// An inclusive optional resource ceiling was exceeded.
    Limit(CountPlanResource),
    /// Checked dimensions or accounting overflowed.
    Overflow,
    /// Source-produced node/atom evidence failed its internal correspondence.
    SourceMapping,
    /// Independent partition validation stopped.
    Partition(partition::Error),
    /// Existing exact count translation stopped.
    Aggregate(zetesis_ferraris::AggregateError),
    /// Independent consequence theory admission stopped.
    Theory(zetesis_ferraris::AdmissionError),
    /// Optional control or fallible allocation stopped planning.
    Stopped(Stop),
}

/// Completed or attempted optional work, independent of source-grounding work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CountPlanStatistics {
    /// Charged operations, including an incomplete prefix.
    pub work: u64,
    /// Captured applicable groups.
    pub groups: usize,
    /// Captured head occurrences.
    pub members: usize,
    /// Copied source locations.
    pub origins: usize,
    /// Attempted complete covers.
    pub attempts: usize,
    /// Local lower bounds after complete consequence emission.
    pub consequences: usize,
    /// Cumulative conservative payload admitted against the optional allowance,
    /// including fitting native partition requests that later fail. A rejected
    /// oversized native request remains in its typed nested error statistics.
    /// This is neither RSS nor a claim of actual retained allocation.
    pub storage_bytes: u64,
}

/// Located optional failure with its accounted work prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CountPlanFailure {
    kind: CountPlanFailureKind,
    location: ProgramSite,
    statistics: CountPlanStatistics,
}
impl CountPlanFailure {
    /// Typed cause, separate from the original admission outcome.
    #[must_use]
    pub const fn kind(&self) -> CountPlanFailureKind {
        self.kind
    }
    /// Original program statement at the interrupted boundary.
    #[must_use]
    pub const fn site(&self) -> ProgramSite {
        self.location
    }
    /// Actual source coordinate, absent for constructed statements.
    #[must_use]
    pub const fn location(&self) -> Option<themelios_base::span::Location> {
        self.location.location()
    }
    /// Optional work attempted before the failure.
    #[must_use]
    pub const fn statistics(&self) -> CountPlanStatistics {
        self.statistics
    }
}
impl fmt::Display for CountPlanFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "optional count planning stopped: {:?}", self.kind)
    }
}
impl std::error::Error for CountPlanFailure {}

/// A source-derived candidate restriction with a closed construction boundary.
///
/// The initial profile uses complete ordinary-choice and bijective count-head
/// groups whose coalesced eligibility is canonical truth. An integer global
/// lower bound and a disjoint, complete partition of upper-bounded groups imply
/// stronger local lower bounds. Capacity groups must be unconditional or share
/// exactly the global activation formula; every consequence keeps that formula.
/// Disequality supplies no interval premise. A group with any nonnumeric logical
/// bound supplies no numeric certificate; independent numeric groups remain
/// eligible. Original source guard evaluation and numeric limits remain unchanged.
///
/// Discovery greedily visits retained grounded groups without backtracking.
/// It can miss a valid alternative cover. With G groups, M retained member
/// occurrences, R original roots and A original atoms, discovery charges at most
/// O(G M² + G² R + G A) work: complete membership probes and root evidence scans
/// are charged conservatively. Emission uses the bounded threshold compiler.
/// Temporary storage includes retained groups and coverage maps, a copied
/// activation DAG prefix, origins and the independently owned restriction.
/// Ordinary grounding retains none of these optional descriptors. Logical work
/// and conservative cumulative payload have separate inclusive ceilings.
///
/// The original theory entails this restriction under the retained source
/// premises. This implementation obligation is tested, not a Rust refinement
/// theorem. The plan proves neither stable membership nor exhaustive search.
#[derive(Debug)]
pub struct CountPlan {
    original: Theory,
    restriction: Theory,
    origins: Vec<ProgramSite>,
    statistics: CountPlanStatistics,
}
impl CountPlan {
    /// Exact original reduct subject. Its immutable storage is shared.
    #[must_use]
    pub fn original_theory(&self) -> &Theory {
        &self.original
    }
    /// Independent original-candidate consequence theory, using the original
    /// semantic atom indices. Apply before proposal, never as a reduct subject.
    #[must_use]
    pub fn restriction(&self) -> &Theory {
        &self.restriction
    }
    /// Source count premises supporting the emitted consequences.
    #[must_use]
    pub fn origins(&self) -> &[ProgramSite] {
        &self.origins
    }
    /// Completed optional planning accounting.
    #[must_use]
    pub const fn statistics(&self) -> CountPlanStatistics {
        self.statistics
    }
    /// Number of strictly stronger guarded local lower bounds.
    #[must_use]
    pub const fn consequence_count(&self) -> usize {
        self.statistics.consequences
    }
}

/// Borrowed outcome of independently requested source-count planning.
#[derive(Clone, Copy, Debug)]
pub enum CountPlanStatus<'a> {
    /// Ordinary grounding retained no optional descriptors or planning budget.
    NotRequested,
    /// The selected bounded planning policy emitted no stronger restriction.
    /// This does not establish that no other cover or consequence exists.
    NoPlan(CountPlanStatistics),
    /// Complete source-derived candidate restriction.
    Ready(&'a CountPlan),
    /// Optional work stopped; the successfully admitted original remains usable.
    Incomplete(&'a CountPlanFailure),
}

#[derive(Debug)]
pub(crate) enum Outcome {
    NotRequested,
    NoPlan(CountPlanStatistics),
    Ready(CountPlan),
    Incomplete(CountPlanFailure),
}
impl Outcome {
    pub(crate) fn view(&self) -> CountPlanStatus<'_> {
        match self {
            Self::NotRequested => CountPlanStatus::NotRequested,
            Self::NoPlan(stats) => CountPlanStatus::NoPlan(*stats),
            Self::Ready(plan) => CountPlanStatus::Ready(plan),
            Self::Incomplete(error) => CountPlanStatus::Incomplete(error),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Request<'a> {
    pub limits: CountPlanLimits,
    pub cancellation: &'a Cancellation,
}

struct Work {
    limits: CountPlanLimits,
    cancellation: Cancellation,
    statistics: CountPlanStatistics,
    location: ProgramSite,
}
impl Work {
    fn poll(&self) -> Result<(), CountPlanFailureKind> {
        self.cancellation
            .poll()
            .map_err(CountPlanFailureKind::Stopped)
    }
    fn charge(&mut self, count: u64) -> Result<(), CountPlanFailureKind> {
        self.poll()?;
        self.record(count)
    }
    fn record(&mut self, count: u64) -> Result<(), CountPlanFailureKind> {
        if count > self.limits.max_work - self.statistics.work {
            return Err(CountPlanFailureKind::Limit(CountPlanResource::Work));
        }
        self.statistics.work += count;
        Ok(())
    }
    fn payload(&mut self, bytes: u64) -> Result<(), CountPlanFailureKind> {
        let total = self
            .statistics
            .storage_bytes
            .checked_add(bytes)
            .ok_or(CountPlanFailureKind::Overflow)?;
        if total > self.limits.max_bytes {
            return Err(CountPlanFailureKind::Limit(CountPlanResource::Bytes));
        }
        self.statistics.storage_bytes = total;
        Ok(())
    }
    fn vector<T>(&mut self, count: usize) -> Result<Vec<T>, CountPlanFailureKind> {
        self.poll()?;
        self.payload(bytes::<T>(count)?)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(count)
            .map_err(|_| CountPlanFailureKind::Stopped(Stop::Allocation))?;
        Ok(output)
    }
    fn failure(&self, kind: CountPlanFailureKind) -> CountPlanFailure {
        CountPlanFailure {
            kind,
            location: self.location,
            statistics: self.statistics,
        }
    }
}
fn bytes<T>(count: usize) -> Result<u64, CountPlanFailureKind> {
    count
        .checked_mul(size_of::<T>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(CountPlanFailureKind::Overflow)
}
fn ceiling(
    actual: usize,
    maximum: usize,
    resource: CountPlanResource,
) -> Result<(), CountPlanFailureKind> {
    if actual > maximum {
        Err(CountPlanFailureKind::Limit(resource))
    } else {
        Ok(())
    }
}
