//! Regions walked by several workers at once, sharing immutable preparation,
//! a cumulative allowance and a work-stealing frontier of regions still to visit.
//!
//! Each worker owns a work-stealing deque of regions with their knowledge, a
//! budget leased for each region from the enumeration's shared allowance, and
//! its own evaluation workspace. Candidate and reduct traversals share the
//! authenticated original-theory index but never their mutable knowledge. A
//! worker pops a region from its own deque, narrows it from the knowledge it
//! carries, drops it when refuted, splits it otherwise and pushes both children
//! back onto its deque; a worker whose deque is empty steals a region from a
//! peer. At a leaf it decides membership as the scalar proposer does, by the
//! class certificate when one applies and else by the proper-subset query as a
//! region tree; a stable model is sent to the enumeration. The regions
//! partition the candidate space exactly (`Cube.split_partition`,
//! `Cube.split_disjoint` of `Search.lean`), so no leaf is visited by
//! two workers and every leaf by one, whatever the interleaving; the order
//! in which models arrive is the schedule's and is not a property of the
//! result, and no two runs promise the same order.
//!
//! A restriction added while the workers run narrows the regions not yet
//! visited: each worker reads the restrictions before a narrowing, and a
//! region reached before a restriction existed gets fresh knowledge under
//! it. A work, decision or candidate ceiling is shared: the first worker
//! to exhaust it raises the stop, the others stop at their next charge or
//! their next region, and the enumeration is incomplete. The models the
//! workers verified before they stopped are delivered first and the stop
//! after them, so a leaf admitted under the candidate ceiling is never
//! lost to a worker the ceiling refused. The region counts and reading
//! work, the candidates, the countermodel counts and the phase timings are
//! the workers' live counters, current while they run; the certificate and
//! reduct receipts are merged when the workers have finished, so a snapshot
//! taken earlier lacks them. Phase timings, when enabled, are the workers'
//! own narrowing and leaf decisions summed over the workers, so they may
//! exceed the wall time of the walk.
//!
//! No lock guards the frontier: each worker's deque is its own, and a steal is
//! a lock-free read of a peer's deque. An atomic counter of unresolved regions
//! carries termination — a split raises it before pushing the children, a
//! resolved region lowers it once, and a worker that finds every deque empty
//! stops only once that counter has reached zero. An atomic closed flag, set on
//! the first stop or when the enumeration stops listening, halts the others at
//! their next region.

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_deque::{Steal, Stealer, Worker};

use zetesis_cpu::regions::{Narrowing, Region};
use zetesis_ferraris::{Interpretation, Knowledge, Narrower, Producers, Theory};

use super::certified::{self, Certification};
use super::regions::{IndexedTheory, RegionCounts, RegionSearchStatistics};
use super::timing::{self, Phase, PhaseMeasurement};
use crate::ferraris::Decision;
use crate::search::{Budget, SharedBudget, WorkLease};
use crate::{Cancellation, Incomplete, Limits, SearchPhaseTimings, SearchStatistics, Statistics};

/// A waiting worker rechecks cooperative control at least once per timed wait.
const IDLE_WAIT: Duration = Duration::from_millis(1);
/// Models a worker may have sent and the enumeration not yet taken, per worker.
const CHANNEL_SLACK: usize = 16;

/// A region with its per-narrower knowledge, the unit workers steal.
type Entry = (Region, Vec<Knowledge>);

struct Shared {
    producers: Option<Producers>,
    index: Arc<IndexedTheory>,
    certificate: Option<Arc<Certification>>,
    filter: Option<crate::region_filter::Filter>,
    restrictions: RwLock<Vec<Arc<(Theory, Narrower)>>>,
    /// Created-but-unresolved regions across every worker's deque and hand: a
    /// split adds one (before pushing its children), a refuted or decided
    /// region subtracts one. It starts at one for the root and reaches zero
    /// exactly when the whole frontier is resolved; a read of zero is a true
    /// zero, so a worker that finds every deque empty and reads zero is done.
    outstanding: AtomicUsize,
    /// A stop was raised (error or cancellation); workers check it before each
    /// region and while idle, and exit. Normal termination is `outstanding`.
    closed: AtomicBool,
    /// The first stop a worker raised; written only on the cold error path,
    /// read by the enumeration once the workers have finished, after their models.
    stopped: Mutex<Option<Incomplete>>,
    budget: SharedBudget,
    limits: Limits,
    cancellation: Cancellation,
    workers: usize,
    /// Whether the enumeration had phase timing enabled when the workers
    /// started; the workers then time their own phases.
    timed: bool,
    /// Leaves proposed by every worker, against the one candidate ceiling.
    candidates: AtomicU64,
    /// What the workers have done so far, readable while they run, so a
    /// snapshot taken before they finish is current.
    live: Live,
}

/// Counters the workers add to as they go.
#[derive(Default)]
struct Live {
    regions: AtomicU64,
    refuted: AtomicU64,
    leaves: AtomicU64,
    propagations: AtomicU64,
    held: AtomicU64,
    cut: AtomicU64,
    work: AtomicU64,
    countermodel_queries: AtomicU64,
    countermodels: AtomicU64,
    /// The workers' phase timings, one pair of counters per phase.
    timings: [LivePhase; 5],
}

/// One phase's calls and elapsed nanoseconds, summed over the workers.
#[derive(Default)]
struct LivePhase {
    calls: AtomicU64,
    nanos: AtomicU64,
    /// Sticky measurement incompleteness, independent of semantic coverage.
    overflowed: AtomicBool,
}

impl LivePhase {
    /// Keep each counter monotone, saturating only after marking the
    /// measurement incomplete. Live snapshots read independent atomics;
    /// after workers join, the totals are exact or explicitly overflowed.
    fn add(&self, counter: &AtomicU64, amount: u64) {
        if amount == 0 {
            return;
        }
        if counter
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(amount)
            })
            .is_err()
        {
            self.overflowed.store(true, Ordering::Relaxed);
            // Every writer only increases this counter. An overflow makes
            // MAX absorbing, including concurrent successful additions.
            counter.store(u64::MAX, Ordering::Relaxed);
        }
    }
}

impl Live {
    fn add(counter: &AtomicU64, amount: u64) {
        counter.fetch_add(amount, Ordering::Relaxed);
    }
    fn read(counter: &AtomicU64) -> u64 {
        counter.load(Ordering::Relaxed)
    }

    /// Add what a worker measured since its last report.
    fn add_timings(&self, before: &SearchPhaseTimings, after: &SearchPhaseTimings) {
        for (live, (before, after)) in self.timings.iter().zip(phases(before).zip(phases(after))) {
            // A worker can become incomplete without changing its retained
            // numeric prefix, so propagate its flag even for a zero delta.
            if before.overflowed || after.overflowed {
                live.overflowed.store(true, Ordering::Relaxed);
            }
            let calls = after.calls.saturating_sub(before.calls);
            let elapsed = after.elapsed.saturating_sub(before.elapsed);
            let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or_else(|_| {
                live.overflowed.store(true, Ordering::Relaxed);
                u64::MAX
            });
            live.add(&live.calls, calls);
            live.add(&live.nanos, nanos);
        }
    }

    fn timings(&self) -> SearchPhaseTimings {
        let read = |live: &LivePhase| PhaseMeasurement {
            calls: Self::read(&live.calls),
            elapsed: Duration::from_nanos(Self::read(&live.nanos)),
            overflowed: live.overflowed.load(Ordering::Relaxed),
        };
        let [
            candidates,
            original_validation,
            reduct,
            reduct_preparation,
            certified,
        ] = &self.timings;
        SearchPhaseTimings {
            candidates: read(candidates),
            original_validation: read(original_validation),
            reduct: read(reduct),
            reduct_preparation: read(reduct_preparation),
            certified: read(certified),
        }
    }
}

/// The five phases in the order the live counters keep them.
fn phases(timings: &SearchPhaseTimings) -> impl Iterator<Item = &PhaseMeasurement> {
    [
        &timings.candidates,
        &timings.original_validation,
        &timings.reduct,
        &timings.reduct_preparation,
        &timings.certified,
    ]
    .into_iter()
}

impl Shared {
    /// Close for a normal end of work (no error): workers exit when they next
    /// check, without recording a stop.
    fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }

    /// Raise a stop: work closes, and the first stop is the one reported.
    fn stop(&self, error: Incomplete) {
        self.stopped
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert(error);
        self.closed.store(true, Ordering::Release);
    }

    /// The first stop raised, if any.
    fn stopped(&self) -> Option<Incomplete> {
        *self.stopped.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The parallel proposer: verified stable models arrive from the workers.
pub(crate) struct ParallelRegions {
    shared: Arc<Shared>,
    /// The root region, seeded into a worker's deque when the workers start.
    root: Option<Entry>,
    /// Taken before joining on drop, releasing workers blocked on a send.
    receiver: Option<Receiver<Interpretation>>,
    sender: Option<SyncSender<Interpretation>>,
    handles: Vec<JoinHandle<Option<WorkerReport>>>,
    started: bool,
    exhausted: bool,
    statistics: RegionSearchStatistics,
    /// The workers' own membership receipts, merged when they finish.
    merged: Statistics,
}

/// What a worker did, returned when it finishes.
struct WorkerReport {
    regions: RegionCounts,
    statistics: Statistics,
}

impl std::fmt::Debug for ParallelRegions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParallelRegions")
            .field("workers", &self.shared.workers)
            .field("started", &self.started)
            .field("exhausted", &self.exhausted)
            .finish_non_exhaustive()
    }
}

impl ParallelRegions {
    /// Prepare the shared structure; the workers start on the first
    /// proposal, once the enumeration's certificates are configured.
    pub(crate) fn new(
        theory: &Theory,
        workers: NonZeroUsize,
        limits: Limits,
        cancellation: Cancellation,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Incomplete> {
        let super::regions::Opened {
            producers,
            index,
            statistics,
        } = super::regions::open(theory, budget)?;
        let (sender, receiver) = sync_channel(workers.get() * CHANNEL_SLACK);
        let root = (
            Region::all_open(theory.atom_count()),
            vec![index.narrower().knowledge()],
        );
        Ok(Self {
            shared: Arc::new(Shared {
                producers,
                index,
                certificate: None,
                filter: None,
                restrictions: RwLock::new(Vec::new()),
                // The root is the one outstanding region until it is split.
                outstanding: AtomicUsize::new(1),
                closed: AtomicBool::new(false),
                stopped: Mutex::new(None),
                budget: SharedBudget::new(limits.search, budget.statistics),
                limits,
                cancellation,
                workers: workers.get(),
                timed: false,
                candidates: AtomicU64::new(0),
                live: Live::default(),
            }),
            root: Some(root),
            receiver: Some(receiver),
            sender: Some(sender),
            handles: Vec::new(),
            started: false,
            exhausted: false,
            statistics,
            merged: Statistics::default(),
        })
    }

    pub(crate) fn set_filter(
        &mut self,
        filter: crate::region_filter::Filter,
    ) -> Result<(), Incomplete> {
        if self.started {
            return Err(Incomplete::LateRegionFilter);
        }
        Arc::get_mut(&mut self.shared)
            .ok_or(Incomplete::LateRegionFilter)?
            .filter = Some(filter);
        Ok(())
    }

    pub(crate) fn search_statistics(&self) -> SearchStatistics {
        let mut statistics = SearchStatistics::default();
        self.shared.budget.snapshot(&mut statistics);
        statistics
    }

    pub(crate) fn filter(&self) -> Option<&crate::region_filter::Filter> {
        self.shared.filter.as_ref()
    }

    /// Stop admission, release blocked senders, then join before exposing the
    /// final source-accounting receipt. The caller's token is unchanged.
    pub(crate) fn stop(&mut self) -> Result<(), Incomplete> {
        self.shared.close();
        drop(self.receiver.take());
        drop(self.sender.take());
        let joined = self.join();
        self.shared.stopped().map_or(joined, Err)
    }

    pub(crate) fn index(&self) -> &Arc<IndexedTheory> {
        &self.shared.index
    }

    /// The region receipts: the workers' live counters, current while they
    /// run and complete when they have finished, and the set-up work. A
    /// worker's narrowing work reaches the live counter as it goes and is
    /// counted nowhere else.
    pub(crate) fn statistics(&self) -> RegionSearchStatistics {
        let live = &self.shared.live;
        let count =
            |counter: &AtomicU64| usize::try_from(Live::read(counter)).unwrap_or(usize::MAX);
        RegionSearchStatistics {
            counts: RegionCounts {
                regions: count(&live.regions),
                refuted: count(&live.refuted),
                leaves: count(&live.leaves),
                propagations: Live::read(&live.propagations),
                held: Live::read(&live.held),
                cut: Live::read(&live.cut),
                work: self
                    .statistics
                    .counts
                    .work
                    .saturating_add(Live::read(&live.work)),
            },
            producers: self.statistics.producers,
            frontier: None,
        }
    }

    /// The workers' membership receipts: the counters current while they
    /// run, and the certificate and reduct receipts merged when they finish.
    pub(crate) fn merged(&self) -> Statistics {
        let live = &self.shared.live;
        Statistics {
            candidates: Live::read(&self.shared.candidates),
            countermodel_queries: Live::read(&live.countermodel_queries),
            countermodels: Live::read(&live.countermodels),
            phase_timings: self.shared.timed.then(|| live.timings()),
            ..self.merged
        }
    }

    /// Restrict every region not yet visited to the classical models of
    /// `restriction`; the workers read it before their next narrowing.
    /// Indexing it is charged to the shared allowance the workers draw on,
    /// and the enumeration's counters take the charge at once.
    pub(crate) fn restrict(
        &mut self,
        restriction: &Theory,
        budget: &mut Budget<'_>,
    ) -> Result<(), Incomplete> {
        if !self.started {
            self.synchronize_budget(budget.statistics)?;
        }
        let narrower = Narrower::new(restriction);
        self.shared.budget.charge(narrower.work())?;
        self.account(budget);
        self.statistics.counts.work += narrower.work();
        let mut restrictions = self
            .shared
            .restrictions
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        restrictions
            .try_reserve(1)
            .map_err(|_| Incomplete::Allocation)?;
        restrictions.push(Arc::new((restriction.clone(), narrower)));
        Ok(())
    }

    /// The next verified stable model, or `None` once the workers have
    /// covered the root. Starts the workers on the first call, with the
    /// certificate the enumeration holds at that moment, timing their
    /// phases when `timed`. A stop a worker raised is returned once every
    /// model the workers sent has been taken, and on every call after that.
    pub(crate) fn propose(
        &mut self,
        certificate: Option<&Arc<Certification>>,
        timed: bool,
        budget: &mut Budget<'_>,
    ) -> Result<Option<Interpretation>, Incomplete> {
        if self.exhausted {
            return Ok(None);
        }
        if !self.started {
            self.synchronize_budget(budget.statistics)?;
            self.start(certificate, timed)?;
        }
        loop {
            budget.cancellation.poll()?;
            match self
                .receiver
                .as_ref()
                .ok_or(Incomplete::ClosedEnumerator)?
                .recv_timeout(IDLE_WAIT)
            {
                Ok(model) => {
                    self.account(budget);
                    return Ok(Some(model));
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    // Every worker has finished: the channel holds nothing
                    // more, so the stop, if one was raised, follows every
                    // model the workers verified.
                    let joined = self.join();
                    self.account(budget);
                    if let Some(error) = self.shared.stopped() {
                        return Err(error);
                    }
                    joined?;
                    self.exhausted = true;
                    return Ok(None);
                }
            }
        }
    }

    /// Before workers own leases, the coordinator's counters include every
    /// setup charge, including certificate configuration since construction.
    /// Reconcile them before another restriction or the first worker starts.
    fn synchronize_budget(&mut self, statistics: SearchStatistics) -> Result<(), Incomplete> {
        let shared = Arc::get_mut(&mut self.shared).ok_or(Incomplete::InvalidWitness)?;
        shared.budget = SharedBudget::new(shared.limits.search, statistics);
        Ok(())
    }

    fn start(
        &mut self,
        certificate: Option<&Arc<Certification>>,
        timed: bool,
    ) -> Result<(), Incomplete> {
        // The certificate is shared read-only; the Arc is cloned into the
        // shared structure before any worker starts.
        let shared = Arc::get_mut(&mut self.shared).ok_or(Incomplete::InvalidWitness)?;
        shared.certificate = certificate.map(Arc::clone);
        shared.timed = timed;
        let workers = self.shared.workers;
        // One work-stealing deque per worker; every worker holds a stealer for
        // each, and the root seeds the first worker's deque.
        let deques: Vec<Worker<Entry>> = (0..workers).map(|_| Worker::new_lifo()).collect();
        let stealers: Arc<[Stealer<Entry>]> = deques
            .iter()
            .map(Worker::stealer)
            .collect::<Vec<_>>()
            .into();
        deques[0].push(self.root.take().ok_or(Incomplete::ClosedEnumerator)?);
        let sender = self.sender.take().ok_or(Incomplete::ClosedEnumerator)?;
        self.handles
            .try_reserve(workers)
            .map_err(|_| Incomplete::Allocation)?;
        for (index, deque) in deques.into_iter().enumerate() {
            let shared = Arc::clone(&self.shared);
            let sender = sender.clone();
            let stealers = Arc::clone(&stealers);
            let handle = std::thread::Builder::new()
                .name("zetesis-region".into())
                .spawn(move || {
                    contain_worker(&shared, || {
                        worker(&shared, &deque, &stealers, index, &sender)
                    })
                })
                .map_err(|_| Incomplete::Allocation)?;
            self.handles.push(handle);
        }
        drop(sender);
        self.started = true;
        Ok(())
    }

    /// Wait for every worker and merge its membership receipts; its region
    /// receipts are already in the live counters.
    ///
    /// # Errors
    /// A merged count beyond its width is the counter refusal.
    fn join(&mut self) -> Result<(), Incomplete> {
        let mut failure = None;
        for handle in self.handles.drain(..) {
            let result = match handle.join() {
                Ok(Some(report)) => merge_membership(&mut self.merged, &report.statistics),
                Ok(None) => Ok(()),
                Err(_) => Err(Incomplete::WorkerPanicked),
            };
            if let Err(error) = result {
                self.shared.stop(error);
                failure.get_or_insert(error);
            }
        }
        failure.map_or(Ok(()), Err)
    }

    /// The shared allowance's spent work and decisions become the
    /// enumeration's search counters.
    fn account(&self, budget: &mut Budget<'_>) {
        let mut spent = SearchStatistics::default();
        self.shared.budget.snapshot(&mut spent);
        budget.statistics.work = budget.statistics.work.max(spent.work);
        budget.statistics.decisions = budget.statistics.decisions.max(spent.decisions);
    }
}

/// An unwinding worker cannot account for the frontier it held. Close the
/// run immediately, waking idle peers, and preserve an incomplete outcome.
fn contain_worker(shared: &Shared, run: impl FnOnce() -> WorkerReport) -> Option<WorkerReport> {
    if let Ok(report) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Some(report)
    } else {
        shared.stop(Incomplete::WorkerPanicked);
        None
    }
}

impl Drop for ParallelRegions {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Merge a joined worker's certificate and reduct receipts; its candidate,
/// query and countermodel counts reached the live counters as it went, and
/// the coordinator counts the stable models it returns. A sum beyond its
/// counter's width is the counter refusal.
fn merge_membership(into: &mut Statistics, from: &Statistics) -> Result<(), Incomplete> {
    fn add(counter: &mut u64, value: u64) -> Result<(), Incomplete> {
        *counter = counter
            .checked_add(value)
            .ok_or(Incomplete::CounterOverflow)?;
        Ok(())
    }
    add(&mut into.reduct.original_work, from.reduct.original_work)?;
    if let Some(from) = from.certified.as_ref() {
        let into = into.certified.get_or_insert_default();
        add(&mut into.checks, from.checks)?;
        add(&mut into.stable, from.stable)?;
        add(&mut into.refuted, from.refuted)?;
        add(&mut into.failed, from.failed)?;
        add(&mut into.checking_work, from.checking_work)?;
        into.positive_check_peak_bytes = match (
            into.positive_check_peak_bytes,
            from.positive_check_peak_bytes,
        ) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
    }
    into.reduct.regions.add(from.reduct.regions)
}

/// One worker's walk, until the run closes, a stop is raised, or the
/// enumeration stops listening.
fn worker(
    shared: &Shared,
    deque: &Worker<Entry>,
    stealers: &[Stealer<Entry>],
    index: usize,
    sender: &SyncSender<Interpretation>,
) -> WorkerReport {
    let mut report = WorkerReport {
        regions: RegionCounts::default(),
        statistics: Statistics {
            // The certificate records its checks in these receipts.
            certified: shared
                .certificate
                .as_ref()
                .map(|_| super::certified::CertifiedStatistics::default()),
            phase_timings: shared.timed.then(SearchPhaseTimings::default),
            ..Statistics::default()
        },
    };
    let mut filter = None;
    let mut reported = SearchPhaseTimings::default();
    let mut search = SearchStatistics::default();
    let mut membership = crate::prepared_reduct::State::with_index(Arc::clone(&shared.index));
    loop {
        // Honour a stop before taking any region, so a worker with a deep deque
        // does not run its whole subtree on after a stop was raised.
        if shared.closed.load(Ordering::Acquire) {
            break;
        }
        let Some(entry) = deque.pop().or_else(|| find_work(shared, stealers, index)) else {
            break;
        };
        // A region owns its grant. Settle it before waiting for another
        // region or sending a model, so other workers can use unused permits.
        let stepped = {
            let mut budget = Budget {
                quota: shared.budget.lease(&shared.cancellation),
                limits: shared.limits.search,
                cancellation: &shared.cancellation,
                statistics: search,
            };
            let result = step(
                shared,
                entry,
                deque,
                &mut budget,
                &mut membership,
                &mut report,
                &mut filter,
            );
            search = budget.statistics;
            result
        };
        if let Some(measured) = report.statistics.phase_timings {
            shared.live.add_timings(&reported, &measured);
            reported = measured;
        }
        match stepped {
            // A split's children are on this worker's deque; `outstanding` was
            // already incremented before they were pushed.
            Ok(Stepped::Split) => {}
            // A resolved region leaves the frontier: decrement once, here, so
            // the count has exactly one structural decrement point.
            Ok(Stepped::Resolved(model)) => {
                shared.outstanding.fetch_sub(1, Ordering::AcqRel);
                if let Some(model) = model
                    && sender.send(model).is_err()
                {
                    shared.close();
                    break;
                }
            }
            Err(error) => {
                shared.stop(error);
                break;
            }
        }
    }
    report
}

/// Steal a region from another worker, waiting while any remains. `None` when
/// the whole frontier is resolved (`outstanding == 0`) or a stop closed the run.
fn find_work(shared: &Shared, stealers: &[Stealer<Entry>], index: usize) -> Option<Entry> {
    let mut idle_rounds = 0u32;
    loop {
        if shared.closed.load(Ordering::Acquire) {
            return None;
        }
        // Round-robin from the next worker, so stealers do not all target one
        // deque. Aggregate: any success wins; else any retry means try again;
        // only all-empty is a true empty.
        let mut retry = false;
        for offset in 1..stealers.len() {
            match stealers[(index + offset) % stealers.len()].steal() {
                Steal::Success(entry) => return Some(entry),
                Steal::Retry => retry = true,
                Steal::Empty => {}
            }
        }
        if retry {
            std::hint::spin_loop();
            idle_rounds = 0;
            continue;
        }
        // Every deque was empty. A read of zero is a true zero, so the frontier
        // is resolved and this worker is done.
        if shared.outstanding.load(Ordering::Acquire) == 0 {
            return None;
        }
        // Work is held by a busy worker; wait, rechecking cooperative control
        // at least once per timed wait, before trying to steal again.
        idle_rounds += 1;
        if idle_rounds < 16 {
            std::thread::yield_now();
        } else {
            std::thread::park_timeout(IDLE_WAIT);
            if let Err(error) = shared.cancellation.poll() {
                shared.stop(error.into());
                return None;
            }
            idle_rounds = 0;
        }
    }
}

/// What one `step` did to its region: split it into two children (now on the
/// worker's deque), or resolved it — refuted, or a leaf with an optional
/// verified model. The caller decrements `outstanding` once, on `Resolved`.
enum Stepped {
    Split,
    Resolved(Option<Interpretation>),
}

/// Narrow one region and act on it: refuted, split, or a leaf decided by
/// the reduct. A split pushes both children onto the worker's own `deque`.
fn step<'a>(
    shared: &'a Shared,
    (mut region, mut knowledge): Entry,
    deque: &Worker<Entry>,
    budget: &mut Budget<'a, WorkLease<'a>>,
    membership: &mut crate::prepared_reduct::State,
    report: &mut WorkerReport,
    filter: &mut Option<crate::region_filter::Worker<'a>>,
) -> Result<Stepped, Incomplete> {
    // The restrictions current when the region is taken, held for its
    // narrowing without the lock.
    let restrictions: Vec<Arc<(Theory, Narrower)>> = shared
        .restrictions
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    report.regions.regions += 1;
    Live::add(&shared.live.regions, 1);
    let before = report.regions;
    let started = timing::start(report.statistics.phase_timings.as_ref());
    let narrowing: Result<Narrowing, Incomplete> = (|| {
        let narrowing = super::regions::narrow(
            (shared.index.theory(), shared.index.narrower()),
            shared.producers.as_ref(),
            &restrictions,
            &mut region,
            &mut knowledge,
            budget,
            &mut report.regions,
        )?;
        if narrowing != Narrowing::Refuted
            && let Some(factory) = shared.filter.as_ref()
            && factory.check(
                filter,
                shared.index.theory(),
                &region,
                &shared.cancellation,
                &mut report.statistics.phase_timings,
            )? == crate::RegionFeasibility::Refuted
        {
            return Ok(Narrowing::Refuted);
        }
        Ok(narrowing)
    })();
    timing::finish(
        &mut report.statistics.phase_timings,
        Phase::Candidates,
        started,
    );
    let after = report.regions;
    Live::add(
        &shared.live.propagations,
        after.propagations - before.propagations,
    );
    Live::add(&shared.live.held, after.held - before.held);
    Live::add(&shared.live.cut, after.cut - before.cut);
    Live::add(&shared.live.work, after.work - before.work);
    let narrowing = narrowing?;
    if narrowing == Narrowing::Refuted {
        report.regions.refuted += 1;
        Live::add(&shared.live.refuted, 1);
        return Ok(Stepped::Resolved(None));
    }
    let Some(atom) = region.split_atom() else {
        report.regions.leaves += 1;
        Live::add(&shared.live.leaves, 1);
        return leaf(shared, &region, budget, membership, report).map(Stepped::Resolved);
    };
    budget.decide()?;
    // Each child carries its own copy of the knowledge, linear in the theory:
    // what the parent learned holds in both.
    let (cut, held) = region.split(atom);
    let held = (held, knowledge.clone());
    // Two children replace this region (net +1). Count the gain BEFORE
    // publishing either child, so no other worker can observe a transient zero
    // and exit early; only infallible pushes may follow the increment.
    shared.outstanding.fetch_add(1, Ordering::AcqRel);
    deque.push(held);
    deque.push((cut, knowledge));
    Ok(Stepped::Split)
}

/// Decide a leaf as the scalar proposer does.
fn leaf<'a>(
    shared: &Shared,
    region: &Region,
    budget: &mut Budget<'a, WorkLease<'a>>,
    membership: &mut crate::prepared_reduct::State,
    report: &mut WorkerReport,
) -> Result<Option<Interpretation>, Incomplete> {
    let candidate = super::regions::leaf_interpretation(shared.index.theory(), region)?;
    shared
        .candidates
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
            (count < shared.limits.max_candidates).then(|| count + 1)
        })
        .map_err(|_| Incomplete::CandidateLimit)?;
    report.statistics.candidates += 1;
    let queries_before = report.statistics.countermodel_queries;
    let countermodels_before = report.statistics.countermodels;
    let decision: Result<Decision, Incomplete> = if let Some(certificate) = &shared.certificate {
        let mut search = budget.statistics;
        let wanted = certificate
            .checking_work_bound()?
            .min(shared.limits.max_verification_work);
        let reservation = budget.quota.reserve(wanted)?;
        let allowance = reservation.allowance();
        let mut limits = shared.limits;
        limits.max_verification_work = allowance;
        if allowance < wanted {
            // Only a shortage of global work lowers the search ceiling.
            // A complete reservation preserves a stricter verification
            // ceiling's own error attribution.
            limits.search.max_work = search
                .work
                .checked_add(allowance)
                .ok_or(Incomplete::CounterOverflow)?;
        }
        let verdict = certified::classify(
            certificate,
            &candidate,
            limits,
            budget.cancellation,
            &mut report.statistics,
            &mut search,
        );
        reservation.finish(search.work - budget.statistics.work)?;
        budget.statistics = search;
        verdict.map(Decision::from)
    } else {
        membership
            .check(
                shared.index.theory(),
                &candidate,
                shared.limits,
                budget,
                &mut report.statistics,
            )
            .map(Decision::from)
    };
    // Query entry is a receipt even when membership checking stops before
    // producing a verdict. Publish it before propagating that failure.
    Live::add(
        &shared.live.countermodel_queries,
        report.statistics.countermodel_queries - queries_before,
    );
    Live::add(
        &shared.live.countermodels,
        report.statistics.countermodels - countermodels_before,
    );
    match decision? {
        Decision::Stable => Ok(Some(candidate)),
        Decision::Refuted => Ok(None),
        Decision::Invalid => Err(Incomplete::InvalidWitness),
    }
}

#[cfg(test)]
#[path = "../tests/support/region_worker_failure.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/support/parallel_timing.rs"]
mod timing_tests;

#[cfg(test)]
#[path = "../tests/support/parallel_coordination.rs"]
mod coordination_tests;
