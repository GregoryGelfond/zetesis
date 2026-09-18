//! Regions walked by several workers at once, sharing nothing but a pool
//! of regions still to visit.
//!
//! Each worker owns a stack of regions with their knowledge, a budget
//! leased from the enumeration's shared allowance, an index of the theory
//! for the reduct query and its own evaluation workspace. It pops a region,
//! narrows it from the knowledge it carries, drops it when refuted, splits
//! it otherwise and keeps both children, offering one to the pool when the
//! pool runs short, and at a leaf decides membership as the scalar
//! proposer does, by the class certificate when one applies and else by
//! the proper-subset query as a region tree; a stable model is sent to the
//! enumeration. The regions partition the candidate space exactly
//! (`Search.split_partition`, `split_disjoint`), so no leaf is visited by
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
//! lost to a worker the ceiling refused. Statistics are merged when the
//! workers have finished; a snapshot taken earlier reports the
//! coordinator's view. Phase timings, when enabled, are the workers' own
//! narrowing and leaf decisions summed over the workers, so they may
//! exceed the wall time of the walk.

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Condvar, Mutex, PoisonError, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

use zetesis_cpu::regions::{Narrowing, Region};
use zetesis_ferraris::{Interpretation, Knowledge, Narrower, Producers, Theory};

use super::certified::{self, Certification};
use super::regions::{RegionSearchStatistics, SearchMethod};
use super::timing::{self, Phase, PhaseMeasurement};
use crate::search::{Budget, SharedBudget, WorkLease};
use crate::{Check, Control, Incomplete, Limits, SearchPhaseTimings, SearchStatistics, Statistics};

/// A waiting worker rechecks cooperative control at least once per timed wait.
const POOL_WAIT: Duration = Duration::from_millis(1);
/// Models a worker may have sent and the enumeration not yet taken, per worker.
const CHANNEL_SLACK: usize = 16;

/// The regions still to visit, and how many workers wait for one.
struct Pool {
    pending: Vec<(Region, Vec<Knowledge>)>,
    idle: usize,
    /// Every worker was idle with nothing pending, or a stop was raised.
    closed: bool,
    /// The first stop a worker raised; read by the enumeration once the
    /// workers have finished, after their models.
    stopped: Option<Incomplete>,
}

struct Shared {
    theory: Theory,
    producers: Option<Producers>,
    narrower: Narrower,
    certificate: Option<Arc<Certification>>,
    restrictions: RwLock<Vec<Arc<(Theory, Narrower)>>>,
    pool: Mutex<Pool>,
    pool_changed: Condvar,
    budget: SharedBudget,
    limits: Limits,
    control: Control,
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
    forced: AtomicU64,
    cut: AtomicU64,
    work: AtomicU64,
    countermodel_queries: AtomicU64,
    countermodels: AtomicU64,
    stable_models: AtomicU64,
    /// The workers' phase timings, one pair of counters per phase.
    timings: [LivePhase; 5],
}

/// One phase's calls and elapsed nanoseconds, summed over the workers.
#[derive(Default)]
struct LivePhase {
    calls: AtomicU64,
    nanos: AtomicU64,
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
            Self::add(&live.calls, after.calls.saturating_sub(before.calls));
            let elapsed = after.elapsed.saturating_sub(before.elapsed);
            Self::add(
                &live.nanos,
                u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX),
            );
        }
    }

    fn timings(&self) -> SearchPhaseTimings {
        let read = |live: &LivePhase| PhaseMeasurement {
            calls: Self::read(&live.calls),
            elapsed: Duration::from_nanos(Self::read(&live.nanos)),
            overflowed: false,
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
    fn lock(&self) -> std::sync::MutexGuard<'_, Pool> {
        self.pool.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn close(&self) {
        self.lock().closed = true;
        self.pool_changed.notify_all();
    }

    /// Raise a stop: the pool closes, and the first stop is the one reported.
    fn stop(&self, error: Incomplete) {
        let mut pool = self.lock();
        pool.closed = true;
        pool.stopped.get_or_insert(error);
        self.pool_changed.notify_all();
    }
}

/// The parallel proposer: verified stable models arrive from the workers.
pub(crate) struct ParallelRegions {
    shared: Arc<Shared>,
    receiver: Receiver<Interpretation>,
    sender: Option<SyncSender<Interpretation>>,
    handles: Vec<JoinHandle<WorkerReport>>,
    started: bool,
    exhausted: bool,
    statistics: RegionSearchStatistics,
    /// The workers' own membership receipts, merged when they finish.
    merged: Statistics,
}

/// What a worker did, returned when it finishes.
struct WorkerReport {
    regions: RegionSearchStatistics,
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
        control: Control,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Incomplete> {
        let extraction =
            zetesis_ferraris::producers(theory, super::regions::limits(budget), budget.control)
                .map_err(super::regions::stopped)?;
        budget.charge(extraction.work)?;
        let narrower = Narrower::new(theory);
        budget.charge(narrower.work())?;
        let statistics = RegionSearchStatistics {
            work: extraction.work + narrower.work(),
            producers: extraction.producers.is_some(),
            ..Default::default()
        };
        let (sender, receiver) = sync_channel(workers.get() * CHANNEL_SLACK);
        let mut pending = Vec::new();
        pending.try_reserve(1).map_err(|_| Incomplete::Allocation)?;
        pending.push((
            Region::undecided(theory.atom_count()),
            vec![narrower.knowledge()],
        ));
        Ok(Self {
            shared: Arc::new(Shared {
                theory: theory.clone(),
                producers: extraction.producers,
                narrower,
                certificate: None,
                restrictions: RwLock::new(Vec::new()),
                pool: Mutex::new(Pool {
                    pending,
                    idle: 0,
                    closed: false,
                    stopped: None,
                }),
                pool_changed: Condvar::new(),
                budget: SharedBudget::new(limits.search, budget.statistics),
                limits,
                control,
                workers: workers.get(),
                timed: false,
                candidates: AtomicU64::new(0),
                live: Live::default(),
            }),
            receiver,
            sender: Some(sender),
            handles: Vec::new(),
            started: false,
            exhausted: false,
            statistics,
            merged: Statistics::default(),
        })
    }

    /// The region receipts, current while the workers run.
    pub(crate) fn statistics(&self) -> RegionSearchStatistics {
        let live = &self.shared.live;
        let count =
            |counter: &AtomicU64| usize::try_from(Live::read(counter)).unwrap_or(usize::MAX);
        RegionSearchStatistics {
            regions: count(&live.regions),
            refuted: count(&live.refuted),
            leaves: count(&live.leaves),
            propagations: Live::read(&live.propagations),
            forced: Live::read(&live.forced),
            cut: Live::read(&live.cut),
            work: self.statistics.work.saturating_add(Live::read(&live.work)),
            producers: self.statistics.producers,
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
            stable_models: Live::read(&live.stable_models),
            phase_timings: self.shared.timed.then(|| live.timings()),
            ..self.merged
        }
    }

    /// Restrict every region not yet visited to the classical models of
    /// `restriction`; the workers read it before their next narrowing.
    pub(crate) fn restrict(&mut self, restriction: &Theory) -> Result<(), Incomplete> {
        let narrower = Narrower::new(restriction);
        self.statistics.work += narrower.work();
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
            self.start(certificate, timed)?;
        }
        loop {
            budget.control.poll()?;
            match self.receiver.recv_timeout(POOL_WAIT) {
                Ok(model) => {
                    self.account(budget);
                    return Ok(Some(model));
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    // Every worker has finished: the channel holds nothing
                    // more, so the stop, if one was raised, follows every
                    // model the workers verified.
                    self.join();
                    self.account(budget);
                    if let Some(error) = self.shared.lock().stopped {
                        return Err(error);
                    }
                    self.exhausted = true;
                    return Ok(None);
                }
            }
        }
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
        let sender = self.sender.take().ok_or(Incomplete::ClosedEnumerator)?;
        self.handles
            .try_reserve(self.shared.workers)
            .map_err(|_| Incomplete::Allocation)?;
        for _ in 0..self.shared.workers {
            let shared = Arc::clone(&self.shared);
            let sender = sender.clone();
            let handle = std::thread::Builder::new()
                .name("zetesis-region".into())
                .spawn(move || worker(&shared, &sender))
                .map_err(|_| Incomplete::Allocation)?;
            self.handles.push(handle);
        }
        drop(sender);
        self.started = true;
        Ok(())
    }

    /// Wait for every worker and merge what it did.
    fn join(&mut self) {
        for handle in self.handles.drain(..) {
            if let Ok(report) = handle.join() {
                merge_regions(&mut self.statistics, &report.regions);
                merge_membership(&mut self.merged, &report.statistics);
            }
        }
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

impl Drop for ParallelRegions {
    fn drop(&mut self) {
        self.shared.close();
        // Dropping the receiver makes every pending send fail, so a worker
        // blocked on a full channel exits at once.
        self.join();
    }
}

fn merge_regions(into: &mut RegionSearchStatistics, from: &RegionSearchStatistics) {
    into.regions = into.regions.saturating_add(from.regions);
    into.refuted = into.refuted.saturating_add(from.refuted);
    into.leaves = into.leaves.saturating_add(from.leaves);
    into.propagations = into.propagations.saturating_add(from.propagations);
    into.forced = into.forced.saturating_add(from.forced);
    into.cut = into.cut.saturating_add(from.cut);
    into.work = into.work.saturating_add(from.work);
}

fn merge_membership(into: &mut Statistics, from: &Statistics) {
    into.candidates = into.candidates.saturating_add(from.candidates);
    into.countermodel_queries = into
        .countermodel_queries
        .saturating_add(from.countermodel_queries);
    into.countermodels = into.countermodels.saturating_add(from.countermodels);
    into.stable_models = into.stable_models.saturating_add(from.stable_models);
    into.reduct.original_work = into
        .reduct
        .original_work
        .saturating_add(from.reduct.original_work);
    if let (Some(into), Some(from)) = (into.certified.as_mut(), from.certified.as_ref()) {
        into.checks = into.checks.saturating_add(from.checks);
        into.stable = into.stable.saturating_add(from.stable);
        into.refuted = into.refuted.saturating_add(from.refuted);
        into.failed = into.failed.saturating_add(from.failed);
        into.checking_work = into.checking_work.saturating_add(from.checking_work);
        into.positive_check_peak_bytes = match (
            into.positive_check_peak_bytes,
            from.positive_check_peak_bytes,
        ) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
    }
    let regions = &mut into.reduct.regions;
    regions.regions = regions.regions.saturating_add(from.reduct.regions.regions);
    regions.refuted = regions.refuted.saturating_add(from.reduct.regions.refuted);
    regions.leaves = regions.leaves.saturating_add(from.reduct.regions.leaves);
    regions.propagations = regions
        .propagations
        .saturating_add(from.reduct.regions.propagations);
    regions.work = regions.work.saturating_add(from.reduct.regions.work);
}

/// One worker's walk, until the pool closes, a stop is raised, or the
/// enumeration stops listening.
fn worker(shared: &Shared, sender: &SyncSender<Interpretation>) -> WorkerReport {
    let mut report = WorkerReport {
        regions: RegionSearchStatistics::default(),
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
    let mut reported = SearchPhaseTimings::default();
    let lease = shared.budget.lease(&shared.control);
    let mut budget = Budget {
        quota: lease,
        limits: shared.limits.search,
        control: &shared.control,
        statistics: SearchStatistics::default(),
    };
    let mut membership = crate::prepared_reduct::State::new(SearchMethod::Regions);
    let mut local: Vec<(Region, Vec<Knowledge>)> = Vec::new();
    while let Some((region, knowledge)) = take(shared, &mut local) {
        let stepped = step(
            shared,
            region,
            knowledge,
            &mut local,
            &mut budget,
            &mut membership,
            &mut report,
        );
        if let Some(measured) = report.statistics.phase_timings {
            shared.live.add_timings(&reported, &measured);
            reported = measured;
        }
        match stepped {
            Ok(Some(model)) => {
                if sender.send(model).is_err() {
                    break;
                }
            }
            Ok(None) => {}
            Err(error) => {
                shared.stop(error);
                break;
            }
        }
    }
    report
}

/// A region from the worker's own stack, else from the pool, waiting while
/// another worker may still offer one; `None` once the pool is closed.
fn take(
    shared: &Shared,
    local: &mut Vec<(Region, Vec<Knowledge>)>,
) -> Option<(Region, Vec<Knowledge>)> {
    if let Some(entry) = local.pop() {
        return Some(entry);
    }
    let mut pool = shared.lock();
    loop {
        if pool.closed {
            return None;
        }
        if let Some(entry) = pool.pending.pop() {
            return Some(entry);
        }
        pool.idle += 1;
        if pool.idle == shared.workers {
            pool.closed = true;
            shared.pool_changed.notify_all();
            return None;
        }
        let (guard, _) = shared
            .pool_changed
            .wait_timeout(pool, POOL_WAIT)
            .unwrap_or_else(PoisonError::into_inner);
        pool = guard;
        pool.idle -= 1;
        if shared.control.poll().is_err() {
            pool.closed = true;
            shared.pool_changed.notify_all();
            return None;
        }
    }
}

/// Narrow one region and act on it: refuted, split, or a leaf decided by
/// the reduct. Returns a verified stable model when the leaf is one.
fn step<'a>(
    shared: &Shared,
    mut region: Region,
    mut knowledge: Vec<Knowledge>,
    local: &mut Vec<(Region, Vec<Knowledge>)>,
    budget: &mut Budget<'a, WorkLease<'a>>,
    membership: &mut crate::prepared_reduct::State,
    report: &mut WorkerReport,
) -> Result<Option<Interpretation>, Incomplete> {
    let restrictions: Vec<Arc<(Theory, Narrower)>> = shared
        .restrictions
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let restricted: Vec<&(Theory, Narrower)> = restrictions.iter().map(|r| &**r).collect();
    report.regions.regions += 1;
    Live::add(&shared.live.regions, 1);
    let before = report.regions;
    let started = timing::start(report.statistics.phase_timings.as_ref());
    let narrowing = super::regions::narrow(
        (&shared.theory, &shared.narrower),
        shared.producers.as_ref(),
        &restricted,
        &mut region,
        &mut knowledge,
        budget,
        &mut report.regions,
    );
    timing::finish(
        &mut report.statistics.phase_timings,
        Phase::Candidates,
        started,
    );
    let narrowing = narrowing?;
    let after = report.regions;
    Live::add(
        &shared.live.propagations,
        after.propagations - before.propagations,
    );
    Live::add(&shared.live.forced, after.forced - before.forced);
    Live::add(&shared.live.cut, after.cut - before.cut);
    Live::add(&shared.live.work, after.work - before.work);
    if narrowing == Narrowing::Refuted {
        report.regions.refuted += 1;
        Live::add(&shared.live.refuted, 1);
        return Ok(None);
    }
    let Some(atom) = region.split_atom() else {
        report.regions.leaves += 1;
        Live::add(&shared.live.leaves, 1);
        return leaf(shared, &region, budget, membership, report);
    };
    budget.decide()?;
    let (cut, held) = region.split(atom);
    // Keep the cut branch for this worker and offer the held one to the
    // pool when the pool is short, else keep both.
    let mut pool = shared.lock();
    if pool.pending.len() < shared.workers {
        pool.pending
            .try_reserve(1)
            .map_err(|_| Incomplete::Allocation)?;
        pool.pending.push((held, knowledge.clone()));
        shared.pool_changed.notify_one();
        drop(pool);
    } else {
        drop(pool);
        local.try_reserve(1).map_err(|_| Incomplete::Allocation)?;
        local.push((held, knowledge.clone()));
    }
    local.try_reserve(1).map_err(|_| Incomplete::Allocation)?;
    local.push((cut, knowledge));
    Ok(None)
}

/// Decide a leaf as the scalar proposer does.
fn leaf<'a>(
    shared: &Shared,
    region: &Region,
    budget: &mut Budget<'a, WorkLease<'a>>,
    membership: &mut crate::prepared_reduct::State,
    report: &mut WorkerReport,
) -> Result<Option<Interpretation>, Incomplete> {
    let mut selected = crate::search::storage(shared.theory.atom_count())?;
    selected.extend(region.held());
    let candidate = Interpretation::new(&shared.theory, selected).map_err(|error| match error {
        zetesis_ferraris::AdmissionError::Allocation => Incomplete::Allocation,
        _ => Incomplete::InvalidWitness,
    })?;
    shared
        .candidates
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
            (count < shared.limits.max_candidates).then(|| count + 1)
        })
        .map_err(|_| Incomplete::CandidateLimit)?;
    report.statistics.candidates += 1;
    let queries_before = report.statistics.countermodel_queries;
    let countermodels_before = report.statistics.countermodels;
    let verdict = if let Some(certificate) = &shared.certificate {
        let mut search = budget.statistics;
        let verdict = certified::classify(
            certificate,
            &candidate,
            shared.limits,
            budget.control,
            &mut report.statistics,
            &mut search,
        );
        budget.statistics = search;
        match verdict? {
            certified::Verdict::Stable => Check::Stable,
            certified::Verdict::NotModel => Check::NotModel,
            certified::Verdict::Unsupported { atom } => Check::Unsupported { atom },
        }
    } else {
        membership.check(
            &shared.theory,
            &candidate,
            shared.limits,
            budget,
            &mut report.statistics,
        )?
    };
    Live::add(
        &shared.live.countermodel_queries,
        report.statistics.countermodel_queries - queries_before,
    );
    Live::add(
        &shared.live.countermodels,
        report.statistics.countermodels - countermodels_before,
    );
    match verdict {
        Check::Stable => {
            report.statistics.stable_models += 1;
            Live::add(&shared.live.stable_models, 1);
            Ok(Some(candidate))
        }
        Check::NonMinimal(_) | Check::Unsupported { .. } => Ok(None),
        Check::NotModel | Check::Inconclusive(_) => Err(Incomplete::InvalidWitness),
    }
}
