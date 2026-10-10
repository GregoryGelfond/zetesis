//! Parallel production of classical candidates, with membership left to the caller.
//!
//! The frontier contains disjoint regions of the original candidate space. A
//! worker removes one region, applies the same readings as the scalar traversal,
//! then refutes it, returns its two children, or emits its classical leaf. No
//! worker checks answer-set membership. A bounded round reserves one output slot
//! per active region, so completed candidates plus active regions never exceed
//! the batch allowance. A split or refutation returns that reservation.
//!
//! All workers join before the next stage runs. Their work leases then settle
//! into the coordinator's cumulative budget, including failed attempts. Retained
//! frontier regions supply the next round; a stop never establishes coverage.
//! Each producer slot retains its checker and worklists between rounds. These
//! are scratch for the same immutable source, not knowledge about a candidate:
//! the latter travels with its frontier region.
//! Candidate order depends on scheduling, while identity and coverage do not.

use std::mem::size_of;
use std::num::NonZeroUsize;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use rayon::prelude::*;
use zetesis_cpu::regions::{Narrowing, Region};
use zetesis_ferraris::{Interpretation, Narrower, NarrowingScratch, Producers, Theory};

use super::conditions::{Bound, CandidateKnowledge, Conditions};
use super::regions::{self, RegionCounts, RegionFrontierStatistics, RegionSearchStatistics};
use crate::search::{Budget, SharedBudget, WorkLease};
use crate::{Cancellation, Incomplete, SearchStatistics};

/// Idle producers poll control while another producer owns the last region.
const CONTROL_WAIT: Duration = Duration::from_millis(1);

type PendingRegion = (Region, CandidateKnowledge);

#[derive(Default)]
struct Workspace<'a> {
    filter: Option<crate::region_filter::Worker<'a>>,
    scratch: NarrowingScratch,
}

type Workspaces<'a> = Vec<Workspace<'a>>;

self_cell::self_cell! {
    /// The filter remains at a stable address until every borrowed checker has
    /// been dropped. Exclusive access to the dependent vector lends distinct
    /// slots to the existing executor; no lock protects a checker's hot path.
    /// There is at most one slot per configured producer. Slot storage is
    /// reserved fallibly; the owner's small allocation follows the existing
    /// infallible boxed-checker contract, not a universal OOM guarantee.
    struct Workers {
        owner: Option<crate::region_filter::Filter>,
        #[not_covariant]
        dependent: Workspaces,
    }
}

/// The same LIFO frontier, with incremental ownership observations. A region's
/// payload changes only while a worker owns it outside this frontier. Count it
/// on insertion/removal, visiting its restriction knowledges but no other entry.
/// Actual disjoint allocations and their headers fit comfortably in u128 on the
/// supported 32/64-bit hosts; no search counter or resource limit is changed.
struct Frontier {
    entries: Vec<PendingRegion>,
    payload_bytes: u128,
    statistics: RegionFrontierStatistics,
}

impl Default for Frontier {
    fn default() -> Self {
        let mut frontier = Self {
            entries: Vec::new(),
            payload_bytes: 0,
            statistics: RegionFrontierStatistics::default(),
        };
        frontier.record();
        frontier
    }
}

impl Frontier {
    fn new(entry: PendingRegion) -> Result<Self, Incomplete> {
        let mut frontier = Self {
            entries: crate::search::storage(1)?,
            ..Self::default()
        };
        frontier.push(entry);
        Ok(frontier)
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn try_reserve(&mut self, additional: usize) -> Result<(), std::collections::TryReserveError> {
        let result = self.entries.try_reserve(additional);
        self.record();
        result
    }

    fn push(&mut self, entry: PendingRegion) {
        self.payload_bytes += Self::payload(&entry);
        self.entries.push(entry);
        self.record();
    }

    fn pop(&mut self) -> Option<PendingRegion> {
        let entry = self.entries.pop()?;
        self.payload_bytes -= Self::payload(&entry);
        self.record();
        Some(entry)
    }

    /// Entry headers already occupy slots in the outer vector. Count only
    /// their owned allocations here, including unused knowledge slots.
    fn payload((region, knowledge): &PendingRegion) -> u128 {
        region.retained_bytes() - size_of::<Region>() as u128 + knowledge.allocated_bytes()
    }

    fn record(&mut self) {
        let statistics = &mut self.statistics;
        statistics.regions = self.entries.len();
        statistics.capacity = self.entries.capacity();
        statistics.retained_bytes = size_of::<Vec<PendingRegion>>() as u128
            + self.entries.capacity() as u128 * size_of::<PendingRegion>() as u128
            + self.payload_bytes;
        statistics.peak_regions = statistics.peak_regions.max(statistics.regions);
        statistics.peak_capacity = statistics.peak_capacity.max(statistics.capacity);
        statistics.peak_retained_bytes = statistics
            .peak_retained_bytes
            .max(statistics.retained_bytes);
    }
}

/// A reusable candidate frontier and an owned Rayon executor. Its immutable
/// preparation, and the original index each round is lent, are shared across
/// workers and every bounded production round.
pub(crate) struct RegionProposals {
    producers: Option<Producers>,
    restrictions: Conditions<(Theory, Narrower)>,
    pending: Frontier,
    pool: rayon::ThreadPool,
    statistics: RegionSearchStatistics,
    workers: Workers,
    /// The worklists of the coordinator's own positive checks.
    scratch: NarrowingScratch,
}

impl std::fmt::Debug for RegionProposals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegionProposals")
            .field("workers", &self.pool.current_num_threads())
            .field("pending", &self.pending.len())
            .finish_non_exhaustive()
    }
}

/// Joined production retains its completed candidates even if another region
/// stopped. Only an empty frontier with no stop establishes exhaustion.
pub(crate) struct Produced {
    pub(crate) exhausted: bool,
    pub(crate) stopped: Option<Incomplete>,
    pub(crate) original_validation: crate::PhaseMeasurement,
}

impl RegionProposals {
    /// Extract the producers and queue the root region with no knowledge;
    /// its first narrowing creates each knowledge slot.
    pub(crate) fn new(
        theory: &Theory,
        workers: NonZeroUsize,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Incomplete> {
        let regions::Opened {
            producers,
            statistics,
        } = regions::open(theory, budget)?;
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers.get())
            .thread_name(|index| format!("zetesis-proposal-{index}"))
            .build()
            .map_err(|_| Incomplete::Allocation)?;
        let pending = Frontier::new((
            Region::all_open(theory.atom_count()),
            CandidateKnowledge::default(),
        ))?;
        Ok(Self {
            producers,
            restrictions: Conditions::default(),
            pending,
            pool,
            statistics,
            workers: Workers::new(None, |_| Vec::new()),
            scratch: NarrowingScratch::default(),
        })
    }

    pub(crate) fn counts_mut(&mut self) -> &mut RegionCounts {
        &mut self.statistics.counts
    }

    pub(super) fn filter(&self) -> Option<&crate::region_filter::Filter> {
        self.workers.borrow_owner().as_ref()
    }

    /// The caller permits installation only before traversal, when no slot
    /// has prepared a checker or retained a worklist.
    pub(super) fn set_filter(&mut self, filter: crate::region_filter::Filter) {
        self.workers = Workers::new(Some(filter), |_| Vec::new());
    }

    /// No producer runs outside a joined round. Discard its retained scratch
    /// on completion or stop, preserving factory receipts and pending regions.
    pub(super) fn finish(&mut self) -> Result<(), Incomplete> {
        self.workers.with_dependent_mut(|_, workspaces| {
            let mut stopped = None;
            // Isolate each callback destructor, as the per-round worker catch
            // did before retention. Clearing the whole vector inside one catch
            // could abort if a second destructor panicked during unwinding.
            while let Some(workspace) = workspaces.pop() {
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(workspace)))
                    .is_err()
                {
                    stopped.get_or_insert(Incomplete::WorkerPanicked);
                }
            }
            stopped.map_or(Ok(()), Err)
        })
    }

    pub(crate) const fn statistics(&self) -> RegionSearchStatistics {
        RegionSearchStatistics {
            frontier: Some(self.pending.statistics),
            ..self.statistics
        }
    }

    pub(crate) fn restrict(
        &mut self,
        restriction: &Theory,
        budget: &mut Budget<'_>,
    ) -> Result<(), Incomplete> {
        self.restrictions
            .permanent
            .try_reserve(1)
            .map_err(|_| Incomplete::Allocation)?;
        let narrower = Narrower::try_new(restriction).map_err(super::regions::stopped)?;
        budget.charge(narrower.work())?;
        self.statistics.counts.work = self
            .statistics
            .counts
            .work
            .checked_add(narrower.work())
            .ok_or(Incomplete::CounterOverflow)?;
        self.restrictions
            .permanent
            .push((restriction.clone(), narrower));
        Ok(())
    }

    /// Commit only after preparation and every charge succeeds.
    pub(crate) fn tighten(
        &mut self,
        restriction: &Theory,
        generation: u64,
        budget: &mut Budget<'_>,
    ) -> Result<(), Incomplete> {
        budget.cancellation.poll()?;
        let bound = Bound::prepare(restriction, generation)?;
        budget.charge(bound.work())?;
        let work = self
            .statistics
            .counts
            .work
            .checked_add(bound.work())
            .ok_or(Incomplete::CounterOverflow)?;
        self.statistics.counts.work = work;
        budget.cancellation.poll()?;
        self.restrictions.bound = Some(bound);
        Ok(())
    }

    pub(super) fn permits_determined(
        &mut self,
        candidate: &Interpretation,
        budget: &mut Budget<'_>,
        timings: &mut Option<crate::SearchPhaseTimings>,
    ) -> Result<bool, Incomplete> {
        regions::permits(
            &self.restrictions,
            self.workers.borrow_owner().as_ref(),
            candidate,
            &mut self.scratch,
            budget,
            &mut self.statistics.counts,
            timings,
        )
    }

    /// Fill pre-admitted output slots without evaluating a reduct. The output
    /// is empty on entry; its allocation remains the caller's batch ownership.
    /// A zero remaining candidate allowance still permits coverage work until
    /// either the frontier is refuted or another classical leaf is encountered.
    /// The round reads the original theory through `subject`, the
    /// enumeration's one shared index, authenticated by the caller.
    pub(crate) fn fill(
        &mut self,
        (theory, narrower): (&Theory, &Narrower),
        maximum: usize,
        remaining: u64,
        budget: &mut Budget<'_>,
        output: &mut Vec<Interpretation>,
        timed: bool,
    ) -> Produced {
        debug_assert!(output.is_empty());
        debug_assert!(maximum > 0 && output.capacity() >= maximum);
        let allowance = SharedBudget::new(budget.limits, budget.statistics);
        let state = self.workers.with_dependent_mut(|filter, workspaces| {
            if workspaces.is_empty() {
                let count = self.pool.current_num_threads();
                workspaces
                    .try_reserve_exact(count)
                    .map_err(|_| Incomplete::Allocation)?;
                workspaces.resize_with(count, Workspace::default);
            }
            let round = Round {
                theory,
                filter: filter.as_ref(),
                timed,
                producers: self.producers.as_ref(),
                narrower,
                restrictions: &self.restrictions,
                allowance: &allowance,
                limits: budget.limits,
                cancellation: budget.cancellation,
                maximum: maximum.min(usize::try_from(remaining).unwrap_or(usize::MAX).max(1)),
                remaining,
                changed: Condvar::new(),
                state: Mutex::new(State {
                    pending: std::mem::take(&mut self.pending),
                    output: std::mem::take(output),
                    active: 0,
                    counts: RegionCounts::default(),
                    stopped: None,
                    original_validation: crate::PhaseMeasurement::default(),
                }),
            };
            self.pool.install(|| {
                workspaces.par_iter_mut().for_each(|workspace| {
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        round.work(workspace);
                    }))
                    .is_err()
                    {
                        round.stop(Incomplete::WorkerPanicked);
                    }
                });
            });
            Ok(round
                .state
                .into_inner()
                .unwrap_or_else(PoisonError::into_inner))
        });
        allowance.record(&mut budget.statistics);
        let mut state = match state {
            Ok(state) => state,
            Err(error) => {
                let _ = self.finish();
                return Produced {
                    exhausted: false,
                    stopped: Some(error),
                    original_validation: crate::PhaseMeasurement::default(),
                };
            }
        };
        if let Err(error) = self.statistics.counts.add(state.counts) {
            state.stopped.get_or_insert(error);
        }
        self.pending = state.pending;
        *output = state.output;
        if (state.stopped.is_some() || self.pending.is_empty())
            && let Err(error) = self.finish()
        {
            state.stopped.get_or_insert(error);
        }
        Produced {
            exhausted: state.stopped.is_none() && self.pending.is_empty(),
            stopped: state.stopped,
            original_validation: state.original_validation,
        }
    }
}

impl Drop for RegionProposals {
    fn drop(&mut self) {
        // Drop has no result channel. Explicit stop reports a teardown fault;
        // implicit drop must still dispose of every checker before its owner.
        let _ = self.finish();
    }
}

struct State {
    pending: Frontier,
    output: Vec<Interpretation>,
    /// Each active region reserves a possible output slot.
    active: usize,
    counts: RegionCounts,
    stopped: Option<Incomplete>,
    original_validation: crate::PhaseMeasurement,
}

#[cfg(test)]
mod tests;

struct Round<'a, 'filter> {
    theory: &'a Theory,
    filter: Option<&'filter crate::region_filter::Filter>,
    timed: bool,
    producers: Option<&'a Producers>,
    narrower: &'a Narrower,
    restrictions: &'a Conditions<(Theory, Narrower)>,
    allowance: &'a SharedBudget,
    limits: crate::SearchLimits,
    cancellation: &'a Cancellation,
    maximum: usize,
    remaining: u64,
    changed: Condvar,
    state: Mutex<State>,
}

enum Step {
    Refuted,
    Split(PendingRegion, PendingRegion),
    Candidate(Interpretation),
}

impl<'filter> Round<'_, 'filter> {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn stop(&self, error: Incomplete) {
        self.lock().stopped.get_or_insert(error);
        self.changed.notify_all();
    }

    fn take(&self) -> Option<PendingRegion> {
        let mut state = self.lock();
        loop {
            if state.stopped.is_some() || state.output.len() == self.maximum {
                return None;
            }
            if let Err(error) = self.cancellation.poll() {
                state.stopped.get_or_insert(error.into());
                self.changed.notify_all();
                return None;
            }
            if state.active + state.output.len() < self.maximum {
                if let Some(entry) = state.pending.pop() {
                    state.active += 1;
                    return Some(entry);
                }
                if state.active == 0 {
                    return None;
                }
            }
            state = self
                .changed
                .wait_timeout(state, CONTROL_WAIT)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    fn work(&self, workspace: &mut Workspace<'filter>) {
        let mut timings = self.timed.then(crate::SearchPhaseTimings::default);
        while let Some(mut entry) = self.take() {
            let mut counts = RegionCounts {
                regions: 1,
                ..Default::default()
            };
            // Settle permits before touching the frontier mutex or waiting;
            // an idle producer must not strand another worker's allowance.
            let result = {
                let mut budget = Budget {
                    quota: self.allowance.lease(self.cancellation),
                    limits: self.limits,
                    cancellation: self.cancellation,
                    statistics: SearchStatistics::default(),
                };
                self.step(
                    &mut entry,
                    &mut workspace.scratch,
                    &mut budget,
                    &mut counts,
                    &mut workspace.filter,
                    &mut timings,
                )
            };
            let mut state = self.lock();
            state.active -= 1;
            if let Err(error) = state.counts.add(counts) {
                state.stopped.get_or_insert(error);
            }
            match result {
                Ok(Step::Refuted) => {}
                Ok(Step::Candidate(candidate)) => state.output.push(candidate),
                Ok(Step::Split(cut, held)) => {
                    if state.pending.try_reserve(2).is_ok() {
                        state.pending.push(held);
                        state.pending.push(cut);
                    } else {
                        state.stopped.get_or_insert(Incomplete::Allocation);
                    }
                }
                Err(error) => {
                    // This region remains unresolved. Retain it when storage
                    // permits; the explicit stop is authoritative either way.
                    if state.pending.try_reserve(1).is_ok() {
                        state.pending.push(entry);
                    }
                    state.stopped.get_or_insert(error);
                }
            }
            self.changed.notify_all();
        }
        if let Some(timings) = timings {
            self.lock()
                .original_validation
                .merge(timings.original_validation);
        }
    }

    fn step<'a>(
        &'a self,
        (region, knowledge): &mut PendingRegion,
        scratch: &mut NarrowingScratch,
        budget: &mut Budget<'a, WorkLease<'a>>,
        counts: &mut RegionCounts,
        filter: &mut Option<crate::region_filter::Worker<'filter>>,
        timings: &mut Option<crate::SearchPhaseTimings>,
    ) -> Result<Step, Incomplete> {
        let mut check = self.filter.map(|factory| crate::region_filter::Check {
            filter: factory,
            worker: filter,
            timings,
        });
        if regions::narrow(
            (self.theory, self.narrower, self.producers),
            self.restrictions,
            (region, knowledge),
            scratch,
            budget,
            counts,
            check.as_mut(),
        )? == Narrowing::Refuted
        {
            counts.refuted = 1;
            return Ok(Step::Refuted);
        }
        if let Some(atom) = region.split_atom() {
            budget.decide()?;
            // Take the region out to split it by value; the emptied placeholder
            // (no atoms, no allocation) is discarded with this pending entry.
            let (cut, held) = std::mem::replace(region, Region::all_open(0)).split(atom);
            let cut = (cut, knowledge.clone());
            let held = (held, std::mem::take(knowledge));
            return Ok(Step::Split(cut, held));
        }
        if self.remaining == 0 {
            return Err(Incomplete::CandidateLimit);
        }
        counts.leaves = 1;
        regions::leaf_interpretation(self.theory, region).map(Step::Candidate)
    }
}
