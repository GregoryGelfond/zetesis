//! Parallel production of classical candidates, with membership left to the caller.
//!
//! The frontier contains disjoint regions of the original candidate space, in
//! one deque per producer, retained between slates. A producer takes a region,
//! applies the same readings as the scalar traversal, then refutes it, splits
//! it, or emits its classical leaf. No producer checks answer-set membership.
//!
//! A slate is the candidates the stream proposes together for one membership
//! check and commit, filled by one bounded run of the producers. A producer
//! holds a region only under a reserved output slot, so the regions in hand
//! plus the emitted candidates never exceed the slate's allowance and the
//! output never reallocates. A producer works depth first: a split publishes
//! the older child on its own deque, where an idle peer can steal it, and keeps
//! the newer child in hand under the same output slot. After a refutation it
//! keeps the slot while it finds another region at once, its own newest or,
//! with an empty deque, a peer's oldest; otherwise it releases the slot. A
//! leaf's slot becomes its candidate's, and the emission that fills the slate
//! closes it. A producer steals only with an empty deque: under the idle gate,
//! where its own deque is only tried, a busy deque ends the attempt instead.
//! The deques, the count of unresolved regions, the idle gate and the idle
//! back-off are the frontier protocols the CPU walker shares
//! (`region_frontier`).
//!
//! That is the order for an enumeration, which visits every region whatever the
//! order: producers work apart and steal rarely. A caller that tightens
//! candidate bounds as it commits answers declares so, and its producers then
//! share one deque instead: every producer publishes there and takes the region
//! any producer published most recently. Production stays where the latest
//! splits are, so a slate fills from the area that is yielding leaves, and
//! older, shallower regions wait for the tighter bounds that refute them near
//! the root. Kept apart, producers would each descend a large subtree under the
//! bound in force when they took it, or under none before the first. The shared
//! deque gives up the per-deque bound on retained regions: a schedule in which
//! producers keep leaving their paths for a newer region can queue far more
//! than `w` paths' siblings, bounded only by the tree, as in the single stack
//! the producers had before they kept deques.
//!
//! A slate ends full, stopped or with every region resolved, after finitely
//! many steps. Each step consumes the one region it took: it refutes it, emits
//! its leaf, or splits it on an atom the region leaves open, so no region is
//! stepped twice, and the regions of `n` atoms form a tree of at most
//! `2^(n+1) - 1` regions; a step that fails stops the slate. Idle producers
//! then leave within one bounded wait.
//!
//! All producers join before the next stage runs. Each holds one work lease
//! for the slate, settled before it steals, before it waits and at its exit,
//! so the coordinator's cumulative budget receives every charge, including
//! failed attempts. A stop closes the slate before the stopping producer
//! returns its region to a deque, and every producer checks for the close
//! after it takes a region and before it steps it. So a region returned after
//! a stop, a failed region included, is never stepped again, and every region
//! in hand but a panicking producer's returns to a deque. A stop never
//! establishes coverage. A cancellation or deadline that a producer observes
//! once the slate is full is left to the next slate's first poll, which
//! observes it again, so no polled stop follows the emission that fills a
//! slate. Each producer retains its checker and worklists between slates.
//! These are scratch for the same immutable source, not knowledge about a
//! candidate: the latter travels with its frontier region. Candidate order
//! depends on scheduling, while identity and coverage do not.

use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use rayon::prelude::*;
use zetesis_cpu::regions::{Narrowing, Region};
use zetesis_ferraris::{Interpretation, Narrower, NarrowingScratch, Theory};

use super::conditions::{Bound, CandidateKnowledge, Conditions};
use super::region_frontier::{Attempt, Busy, Entry, RegionFrontier, Termination, reserve_regions};
use super::regions::{self, FrontierOrder, RegionCounts, RegionSearchStatistics};
use crate::search::{Budget, SharedBudget, WorkLease};
use crate::{Cancellation, Incomplete, SearchLimits, SearchPhaseTimings, SearchStatistics};

/// Idle producers poll control while another producer owns the last region.
const CONTROL_WAIT: Duration = Duration::from_millis(1);

/// One producer's retained scratch and its receipts for the current slate.
/// The receipts live here, beside the scratch, so a panicking producer's
/// counts and timings still reach the merge after the join.
#[derive(Default)]
struct Workspace<'a> {
    filter: Option<crate::region_filter::Worker<'a>>,
    scratch: NarrowingScratch,
    counts: RegionCounts,
    timings: Option<SearchPhaseTimings>,
}

type Workspaces<'a> = Vec<Workspace<'a>>;

self_cell::self_cell! {
    /// The filter remains at a stable address until every borrowed checker has
    /// been dropped. Exclusive access to the dependent vector lends distinct
    /// workspaces to the existing executor; no lock protects a checker's hot
    /// path. There is at most one workspace per configured producer. Workspace
    /// storage is reserved fallibly; the owner's small allocation follows the
    /// existing infallible boxed-checker contract, not a universal OOM
    /// guarantee.
    struct Workers {
        owner: Option<crate::region_filter::Filter>,
        #[not_covariant]
        dependent: Workspaces,
    }
}

/// A reusable candidate frontier and an owned Rayon executor. Its immutable
/// preparation, and the original index each slate is lent, are shared across
/// producers and every bounded slate.
pub(crate) struct RegionProposals {
    /// What the walk reads of the original theory beyond the theory itself.
    parts: regions::OriginalParts,
    restrictions: Conditions<(Theory, Narrower)>,
    /// One deque per producer, with exact receipts, retained between slates.
    frontier: RegionFrontier,
    /// Whether producers keep their own deques or share deque 0.
    order: FrontierOrder,
    pool: rayon::ThreadPool,
    statistics: RegionSearchStatistics,
    workers: Workers,
    /// The worklists of the coordinator's own positive checks.
    scratch: NarrowingScratch,
    native_byte_limit: u64,
}

impl std::fmt::Debug for RegionProposals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegionProposals")
            .field("producers", &self.pool.current_num_threads())
            .field("unresolved", &self.frontier.termination.unresolved())
            .finish_non_exhaustive()
    }
}

/// Joined production retains its completed candidates even if another region
/// stopped. Only a fully resolved frontier with no stop establishes exhaustion.
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
        native_byte_limit: u64,
        order_byte_limit: u64,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Incomplete> {
        let regions::Opened { parts, statistics } =
            regions::open(theory, order_byte_limit, budget)?;
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers.get())
            .thread_name(|index| format!("zetesis-proposal-{index}"))
            .build()
            .map_err(|_| Incomplete::Allocation)?;
        let frontier = RegionFrontier::measured(
            workers.get(),
            (
                Region::all_open(theory.atom_count()),
                CandidateKnowledge::default(),
            ),
        )?;
        Ok(Self {
            parts,
            restrictions: Conditions::default(),
            frontier,
            order: FrontierOrder::PerProducer,
            pool,
            statistics,
            workers: Workers::new(None, |_| Vec::new()),
            scratch: NarrowingScratch::default(),
            native_byte_limit,
        })
    }

    pub(crate) fn counts_mut(&mut self) -> &mut RegionCounts {
        &mut self.statistics.counts
    }

    pub(super) fn filter(&self) -> Option<&crate::region_filter::Filter> {
        self.workers.borrow_owner().as_ref()
    }

    /// The caller permits installation only before traversal, when no
    /// producer has prepared a checker or retained a worklist.
    pub(super) fn set_filter(&mut self, filter: crate::region_filter::Filter) {
        self.workers = Workers::new(Some(filter), |_| Vec::new());
    }

    /// No producer runs outside a joined slate. Discard its retained scratch
    /// on completion or stop, preserving factory receipts and pending regions.
    pub(super) fn finish(&mut self) -> Result<(), Incomplete> {
        self.workers.with_dependent_mut(|_, workspaces| {
            let mut stopped = None;
            // Isolate each callback destructor, as the per-slate producer
            // catch did before retention. Clearing the whole vector inside one
            // catch could abort if a second destructor panicked during
            // unwinding.
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

    pub(crate) fn statistics(&self) -> RegionSearchStatistics {
        RegionSearchStatistics {
            frontier: self.frontier.statistics(),
            frontier_order: Some(self.order),
            ..self.statistics
        }
    }

    /// From the next slate on, every producer shares deque 0, the deque that
    /// holds the root: the order for a caller that tightens candidate bounds.
    /// Regions already queued in other deques stay reachable, since a producer
    /// whose shared deque is empty steals from the others.
    pub(crate) fn share_frontier(&mut self) {
        self.order = FrontierOrder::Shared;
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
        let narrower = super::original_index::prepare(
            restriction,
            self.native_byte_limit,
            budget.cancellation,
        )?;
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
        let bound = Bound::tightened(
            restriction,
            generation,
            self.native_byte_limit,
            budget,
            &mut self.statistics.counts.work,
        )?;
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
    /// The slate reads the original theory through `subject`, the
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
        // No producer runs between slates, so the count is carried exactly.
        self.frontier.termination = Termination::resuming(self.frontier.termination.unresolved());
        let joined = self.workers.with_dependent_mut(|filter, workspaces| {
            if workspaces.is_empty() {
                let count = self.pool.current_num_threads();
                workspaces
                    .try_reserve_exact(count)
                    .map_err(|_| Incomplete::Allocation)?;
                workspaces.resize_with(count, Workspace::default);
            }
            for workspace in workspaces.iter_mut() {
                workspace.counts = RegionCounts::default();
                workspace.timings = timed.then(SearchPhaseTimings::default);
            }
            let slate = SlateFill {
                theory,
                filter: filter.as_ref(),
                parts: &self.parts,
                narrower,
                restrictions: &self.restrictions,
                allowance: &allowance,
                limits: budget.limits,
                cancellation: budget.cancellation,
                frontier: &self.frontier,
                maximum: maximum.min(usize::try_from(remaining).unwrap_or(usize::MAX).max(1)),
                remaining,
                output_slots: AtomicUsize::new(0),
                outcome: Mutex::new(Outcome {
                    candidates: std::mem::take(output),
                    stopped: None,
                }),
                order: self.order,
            };
            self.pool.install(|| {
                workspaces
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(index, workspace)| {
                        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            slate.produce(index, workspace);
                        }))
                        .is_err()
                        {
                            slate.stop(Incomplete::WorkerPanicked);
                        }
                    });
            });
            let Outcome {
                candidates,
                mut stopped,
            } = slate
                .outcome
                .into_inner()
                .unwrap_or_else(PoisonError::into_inner);
            let mut counts = RegionCounts::default();
            let mut original_validation = crate::PhaseMeasurement::default();
            for workspace in workspaces.iter() {
                if let Err(error) = counts.add(workspace.counts) {
                    stopped.get_or_insert(error);
                }
                if let Some(timings) = &workspace.timings {
                    original_validation.merge(timings.original_validation);
                }
            }
            Ok((candidates, stopped, counts, original_validation))
        });
        allowance.record(&mut budget.statistics);
        let (produced, mut stopped, counts, original_validation) = match joined {
            Ok(joined) => joined,
            Err(error) => {
                let _ = self.finish();
                return Produced {
                    exhausted: false,
                    stopped: Some(error),
                    original_validation: crate::PhaseMeasurement::default(),
                };
            }
        };
        *output = produced;
        if let Err(error) = self.statistics.counts.add(counts) {
            stopped.get_or_insert(error);
        }
        let resolved = self.frontier.termination.is_resolved();
        if (stopped.is_some() || resolved)
            && let Err(error) = self.finish()
        {
            stopped.get_or_insert(error);
        }
        Produced {
            exhausted: stopped.is_none() && resolved,
            stopped,
            original_validation,
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

#[cfg(test)]
mod tests;

/// What a slate has produced: its emitted candidates and its first stop. One
/// lock guards both, so a stop that polling control observed is recorded only
/// while the slate is not yet full.
struct Outcome {
    candidates: Vec<Interpretation>,
    stopped: Option<Incomplete>,
}

/// One slate's shared state: the slate's allowance of output slots and its
/// outcome.
struct SlateFill<'a, 'filter> {
    theory: &'a Theory,
    filter: Option<&'filter crate::region_filter::Filter>,
    parts: &'a regions::OriginalParts,
    narrower: &'a Narrower,
    restrictions: &'a Conditions<(Theory, Narrower)>,
    allowance: &'a SharedBudget,
    limits: SearchLimits,
    cancellation: &'a Cancellation,
    frontier: &'a RegionFrontier,
    maximum: usize,
    remaining: u64,
    /// Output slots reserved: the regions producers hold plus the emitted
    /// candidates, never more than `maximum`.
    output_slots: AtomicUsize,
    /// A leaf lock: the slate closes only after it is released.
    outcome: Mutex<Outcome>,
    order: FrontierOrder,
}

enum Step {
    Refuted,
    Split(Entry, Entry),
    Candidate(Interpretation),
}

impl<'filter> SlateFill<'_, 'filter> {
    /// The deque producer `index` publishes to, takes from first and returns
    /// regions to: its own, or the one deque all producers share.
    fn home(&self, index: usize) -> usize {
        match self.order {
            FrontierOrder::PerProducer => index,
            FrontierOrder::Shared => 0,
        }
    }

    fn outcome(&self) -> MutexGuard<'_, Outcome> {
        self.outcome.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Record the first stop and close the slate: producers take no further
    /// region, return the regions they hold and leave.
    fn stop(&self, error: Incomplete) {
        self.outcome().stopped.get_or_insert(error);
        self.frontier.termination.close();
    }

    /// Record a stop that polling control observed, unless the slate is
    /// already full, and close the slate. The emission that fills the slate
    /// ends it without a stop, and a requested stop stays requested, so the
    /// next slate's first poll observes it. The check and the record share the
    /// lock the emission takes, so no such stop follows the filling emission.
    fn stop_unless_full(&self, error: Incomplete) {
        let recorded = {
            let mut outcome = self.outcome();
            let open = outcome.candidates.len() < self.maximum;
            if open {
                outcome.stopped.get_or_insert(error);
            }
            open
        };
        if recorded {
            self.frontier.termination.close();
        }
    }

    /// Reserve one output slot, if the slate has one left.
    fn reserve_slot(&self) -> bool {
        self.output_slots
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |held| {
                (held < self.maximum).then_some(held + 1)
            })
            .is_ok()
    }

    /// Return an output slot whose holder found no region. It wakes nobody: a
    /// producer waiting for a slot sees it within its bounded wait.
    fn release_slot(&self) {
        self.output_slots.fetch_sub(1, Ordering::AcqRel);
    }

    /// One attempt to take a region under a newly reserved output slot: the
    /// newest region of the producer's home deque, or, with that deque empty,
    /// another deque's oldest. Under the gate only `try_lock` is used, and a
    /// home deque found busy ends the attempt without a steal: another producer
    /// holds it, and it may still hold regions, which the next attempt outside
    /// the gate waits for. Every failed attempt settles the lease, so the
    /// back-off never waits holding permits.
    fn admit(&self, index: usize, attempt: Attempt, lease: &WorkLease<'_>) -> Option<Entry> {
        if !self.reserve_slot() {
            lease.settle();
            return None;
        }
        let home = self.home(index);
        let local = match attempt {
            Attempt::Outside => Ok(self.frontier.take_local(home)),
            Attempt::UnderGate => self.frontier.try_take_local(home),
        };
        let entry = match local {
            Ok(Some(entry)) => Some(entry),
            Ok(None) => {
                lease.settle();
                self.frontier.steal(home)
            }
            Err(Busy) => {
                lease.settle();
                None
            }
        };
        if entry.is_none() {
            self.release_slot();
        }
        entry
    }

    /// A region under a reserved output slot, waiting while any remains;
    /// `None` when the slate ended.
    fn acquire(&self, index: usize, lease: &WorkLease<'_>) -> Option<Entry> {
        self.frontier.find_work(
            CONTROL_WAIT,
            self.cancellation,
            |attempt| self.admit(index, attempt, lease),
            |error| self.stop_unless_full(error),
        )
    }

    /// Publish a split's older child and keep its newer one in hand under the
    /// same output slot. A refused publication drops the older child
    /// unpublished, leaving the count unchanged, and fails the newer child
    /// (see [`Self::fail`]).
    fn hand_over(
        &self,
        index: usize,
        (cut, held): (Entry, Entry),
        reserve: impl FnOnce(&mut VecDeque<Entry>, usize) -> Result<(), Incomplete>,
    ) -> Option<Entry> {
        match self
            .frontier
            .publish_split(self.home(index), [cut], reserve)
        {
            Ok(()) => Some(held),
            Err(error) => {
                self.fail(index, held, error);
                None
            }
        }
    }

    /// Stop the slate on `error`, then return `entry`, a region producer
    /// `index` could not resolve, to its home deque when storage permits. The
    /// order is the guarantee: the stop closes the slate before the region
    /// becomes visible, so a producer that takes it acquires the deque after
    /// the close, sees the close in [`Self::ready`] and returns it unstepped.
    /// The recorded stop keeps the run incomplete whether or not the region is
    /// retained.
    fn fail(&self, index: usize, entry: Entry, error: Incomplete) {
        self.stop(error);
        self.frontier.requeue(self.home(index), entry);
    }

    /// Whether `entry`, a region the producer has just taken, may be stepped.
    /// Not once the slate has closed: the close is read after the take, so a
    /// region a stopping producer returned is never stepped again. Nor when
    /// control requests a stop, which is recorded. A refused region comes back
    /// for the producer to return to its deque.
    fn ready(&self, entry: Entry) -> Result<Entry, Entry> {
        if self.frontier.termination.is_closed() {
            return Err(entry);
        }
        if let Err(error) = self.cancellation.poll() {
            self.stop_unless_full(error.into());
            return Err(entry);
        }
        Ok(entry)
    }

    /// After a refutation the producer keeps its output slot when it finds
    /// another region at once: its home deque's newest, or, with that deque
    /// empty, another deque's oldest after settling its lease. Otherwise the
    /// slot is released.
    fn after_refutation(&self, index: usize, lease: &WorkLease<'_>) -> Option<Entry> {
        let home = self.home(index);
        let next = self.frontier.take_local(home).or_else(|| {
            lease.settle();
            self.frontier.steal(home)
        });
        if next.is_none() {
            self.release_slot();
        }
        next
    }

    /// The leaf's region is resolved and its output slot becomes its
    /// candidate's. The emission that fills the slate closes it, a normal end
    /// with no stop, after the outcome lock is released.
    fn emit(&self, candidate: Interpretation) {
        self.frontier.termination.resolve();
        let full = {
            let mut outcome = self.outcome();
            outcome.candidates.push(candidate);
            outcome.candidates.len() == self.maximum
        };
        if full {
            self.frontier.termination.close();
        }
    }

    /// Leaving the slate, producer `index` returns the region it holds, if
    /// any, to its home deque, so the region stays queued for the next slate.
    /// [`Self::produce`] leaves through this one transition on every exit.
    fn leave(&self, index: usize, hand: Option<Entry>) {
        if let Some(entry) = hand {
            self.frontier.requeue(self.home(index), entry);
        }
    }

    /// Producer `index`'s part of the slate. It holds one lease throughout,
    /// checks the slate and polls control after it takes each region and
    /// before it steps it, keeps the newer child of each split in hand, and
    /// leaves through [`Self::leave`] when the slate ends.
    fn produce(&self, index: usize, workspace: &mut Workspace<'filter>) {
        let mut lease = self.allowance.lease(self.cancellation);
        let mut search = SearchStatistics::default();
        let mut hand: Option<Entry> = None;
        loop {
            let taken = match hand.take() {
                Some(entry) => entry,
                None => match self.acquire(index, &lease) {
                    Some(entry) => entry,
                    None => break,
                },
            };
            let mut entry = match self.ready(taken) {
                Ok(entry) => entry,
                Err(entry) => {
                    hand = Some(entry);
                    break;
                }
            };
            let mut counts = RegionCounts {
                regions: 1,
                ..Default::default()
            };
            let result = {
                let mut budget = Budget {
                    quota: lease,
                    limits: self.limits,
                    cancellation: self.cancellation,
                    statistics: search,
                };
                let result = self.step(&mut entry, workspace, &mut budget, &mut counts);
                search = budget.statistics;
                lease = budget.quota;
                result
            };
            let overflow = workspace.counts.add(counts).err();
            match result {
                Ok(Step::Refuted) => {
                    self.frontier.termination.resolve();
                    hand = self.after_refutation(index, &lease);
                }
                Ok(Step::Split(cut, held)) => {
                    match self.hand_over(index, (cut, held), reserve_regions) {
                        Some(held) => hand = Some(held),
                        None => break,
                    }
                }
                Ok(Step::Candidate(candidate)) => self.emit(candidate),
                Err(error) => {
                    // This region remains unresolved.
                    self.fail(index, entry, error);
                    break;
                }
            }
            if let Some(error) = overflow {
                self.stop(error);
                break;
            }
        }
        self.leave(index, hand);
    }

    fn step<'a>(
        &'a self,
        (region, knowledge): &mut Entry,
        workspace: &mut Workspace<'filter>,
        budget: &mut Budget<'a, WorkLease<'a>>,
        counts: &mut RegionCounts,
    ) -> Result<Step, Incomplete> {
        let mut check = self.filter.map(|factory| crate::region_filter::Check {
            filter: factory,
            worker: &mut workspace.filter,
            timings: &mut workspace.timings,
        });
        if regions::narrow(
            (self.parts.subject(self.theory), self.narrower),
            self.restrictions,
            (region, knowledge),
            &mut workspace.scratch,
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
            // (no atoms, no allocation) is discarded with this entry.
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
