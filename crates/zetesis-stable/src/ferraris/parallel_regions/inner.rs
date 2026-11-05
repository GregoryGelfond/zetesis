//! Proper-subset queries divided among the parallel walk's workers.
//!
//! A worker decides a leaf's membership inline, as the scalar walk does. Before
//! each narrowing its query asks whether to publish: on a worker, once the
//! query has run for [`INNER_DELAY`] since its first narrowing while another
//! worker waits idle at the frontier's gate (as [`QueryDivision`] says); on the
//! coordinator, once the walk's engagement deadline has passed. A published
//! query stops with the region it was about to narrow still pending and hands
//! every pending region to the publisher's inner deque, each with its
//! frozen-narrowing knowledge and the query's cell. Those regions and the ones
//! the query visited partition its root, so serving every published region
//! covers the query's tree. The publisher returns to its loop; any worker may
//! serve the regions, and the leaf stays outstanding in the walk's count until
//! the cell is decided. A refused allocation keeps the query inline.
//!
//! The cell owns the query's subject, a copy of the candidate's node truth and
//! the candidate, and counts the query's live regions, queued or in hand. The
//! count is set to the published regions' number before any is visible; a split
//! adds one before either child is visible; a refuted region, the candidate's
//! own leaf, a leaf validated as a countermodel, and a region dropped once the
//! query is decided each subtract one. A server writes a witness or a stop
//! before its subtraction, which releases it, and the subtraction that reaches
//! zero is an acquire-release read-modify-write that follows every earlier one,
//! so the server that reads zero sees every earlier witness and stop. A
//! compare-and-swap on the cell's resolution decides its one resolver: the
//! server that validates a countermodel resolves it by absorption and delivers
//! nothing; the server whose subtraction reaches zero with no witness and no
//! stop resolves it stable and sends the candidate; either then resolves the
//! leaf in the walk's count. A server's error, a stop or the validator's
//! refusal, is the query's as it is inline: it sets the cell's stop, raises the
//! walk's, and is never counted as a resolved region, so no cell resolves
//! stable on an undecided leaf. That an emptied walk of the query's tree without
//! a leaf is stability, and that any leaf emitted by any walk refutes it,
//! whoever held each region, is `InnerQuery.lean`.
//!
//! A server narrows on its own reduct workspace's worklists, admitted against
//! the reduct's byte ceiling as an inline query's are, and charges its own
//! lease on the walk's one allowance. A worker takes its own inner deque's
//! newest region before its own candidate deque's, and with both empty steals
//! a peer's oldest published region before a peer's oldest candidate region.
//! A worker publishes only with its inner deque empty, takes a candidate region
//! only then, and steals only with both deques empty, so an inner deque holds
//! one query's regions at a time; depth-first order keeps one older sibling per
//! ancestor, so it holds at most `|M| + 1` for candidate `M`, and with the
//! candidate deque's `atom_count + 1` a worker holds at most
//! `2(atom_count + 1)` entries. A decided query's regions may remain queued
//! when the walk ends; they carry no obligation and are released with the
//! deques.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, TryLockError};
use std::time::{Duration, Instant};

use zetesis_algebra::{region::Entry, walk::Walk};
use zetesis_ferraris::Interpretation;

use super::{
    ENGAGEMENT_COST_PER_WORKER, Live, QueryDivision, Shared, Taken, WorkerReport, Workspaces,
    publish_timings,
};
use crate::budget::{Budget, WorkLease};
use crate::ferraris::IndexedTheory;
use crate::ferraris::conditions::CountermodelKnowledge;
use crate::ferraris::region_frontier::Termination;
use crate::ferraris::regions::{Publication, ReductQuery, Served};
use crate::timing::{self, Phase};
use crate::{Incomplete, SearchPhaseTimings};

#[cfg(test)]
mod tests;

/// How long a worker's query runs, from its first narrowing, before it may
/// publish to an idle worker: the measured cost of engaging one worker.
/// Publishing copies the candidate's node truth and atoms and moves at most
/// `|M| + 1` pending regions, less than a thread's start, so a query that has
/// run this long has spent more than its publication costs.
pub(super) const INNER_DELAY: Duration = ENGAGEMENT_COST_PER_WORKER;

/// One candidate's published proper-subset query: what any worker needs to
/// narrow its regions, and the count that decides it.
pub(super) struct QueryCell {
    /// The candidate's node truth, the frozen reduct's mask, owned because the
    /// publisher's evaluation workspace serves its next leaf.
    truth: Vec<bool>,
    /// The candidate the query decides.
    candidate: Interpretation,
    /// The query's live regions, queued or in hand.
    outstanding: AtomicUsize,
    /// A validated countermodel, set once.
    witness: OnceLock<Interpretation>,
    /// The first stop raised while serving the query, set once.
    stop: OnceLock<Incomplete>,
    /// Whether the query is decided, set once by compare-and-swap.
    resolved: AtomicBool,
}

impl QueryCell {
    /// A cell counting `pending` published regions of `candidate`'s query
    /// under node truth `truth`. Its copies are allocated fallibly.
    fn new(truth: &[bool], candidate: &Interpretation, pending: usize) -> Result<Self, Incomplete> {
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(truth.len())
            .map_err(|_| Incomplete::Allocation)?;
        owned.extend_from_slice(truth);
        let candidate = candidate.try_clone().map_err(|_| Incomplete::Allocation)?;
        Ok(Self {
            truth: owned,
            candidate,
            outstanding: AtomicUsize::new(pending),
            witness: OnceLock::new(),
            stop: OnceLock::new(),
            resolved: AtomicBool::new(false),
        })
    }

    fn is_resolved(&self) -> bool {
        self.resolved.load(Ordering::Acquire)
    }

    /// A split replaces its region by two: count the net gain of one.
    fn grow(&self) {
        self.outstanding.fetch_add(1, Ordering::AcqRel);
    }

    /// Withdraw a gain whose children were never published. The parent, in
    /// hand, still counts, so this never reaches zero.
    fn shrink(&self) {
        let previous = self.outstanding.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 1, "a withdrawn gain left no parent counted");
    }

    /// Retire one region, releasing whatever this server wrote before it.
    /// Returns whether this retirement decided the query stable: it was the
    /// last, it saw no witness and no stop, and it won the resolution.
    fn retire(&self) -> bool {
        let previous = self.outstanding.fetch_sub(1, Ordering::AcqRel);
        debug_assert_ne!(previous, 0, "a published region was retired twice");
        previous == 1 && self.witness.get().is_none() && self.stop.get().is_none() && self.resolve()
    }

    /// Record a validated countermodel. Returns whether this call decided
    /// the query: by absorption, whoever's witness the cell keeps.
    fn absorb(&self, witness: Interpretation) -> bool {
        let _ = self.witness.set(witness);
        self.resolve()
    }

    /// Record the first stop raised while serving the query.
    fn halt(&self, error: Incomplete) {
        let _ = self.stop.set(error);
    }

    /// Win the query's one resolution.
    fn resolve(&self) -> bool {
        self.resolved
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}

/// A region of a published query, with its knowledge and the query's cell.
pub(super) struct InnerEntry {
    entry: Entry<CountermodelKnowledge>,
    cell: Arc<QueryCell>,
}

impl InnerEntry {
    fn new(entry: Entry<CountermodelKnowledge>, cell: &Arc<QueryCell>) -> Self {
        Self {
            entry,
            cell: Arc::clone(cell),
        }
    }
}

/// One deque of published regions per worker, beside the frontier's, with
/// the regions each worker served. Each deque has its own mutex, held only to
/// push or pop; no two are held at once, and a thief skips a busy one.
pub(super) struct InnerDeques {
    queues: Vec<Mutex<VecDeque<InnerEntry>>>,
    /// The published regions each worker narrowed.
    served: Vec<AtomicU64>,
}

impl InnerDeques {
    /// Empty deques for `workers`, their headers reserved fallibly.
    pub(super) fn new(workers: usize) -> Result<Self, Incomplete> {
        let mut queues = crate::budget::storage(workers)?;
        let mut served = crate::budget::storage(workers)?;
        for _ in 0..workers {
            queues.push(Mutex::new(VecDeque::new()));
            served.push(AtomicU64::new(0));
        }
        Ok(Self { queues, served })
    }

    fn queue(&self, index: usize) -> MutexGuard<'_, VecDeque<InnerEntry>> {
        self.queues[index]
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Worker `index`'s newest published region.
    pub(super) fn take_local(&self, index: usize) -> Option<InnerEntry> {
        self.queue(index).pop_back()
    }

    /// The oldest published region of a peer's deque, trying peers in
    /// round-robin order and skipping a busy one.
    pub(super) fn steal(&self, index: usize) -> Option<InnerEntry> {
        let workers = self.queues.len();
        for offset in 1..workers {
            let mut queue = match self.queues[(index + offset) % workers].try_lock() {
                Ok(queue) => queue,
                Err(TryLockError::Poisoned(error)) => error.into_inner(),
                Err(TryLockError::WouldBlock) => continue,
            };
            if let Some(entry) = queue.pop_front() {
                return Some(entry);
            }
        }
        None
    }

    /// Publish `traversal`'s pending regions under `cell`, which already
    /// counts them, on worker `index`'s deque, the oldest first so that the
    /// owner's newest-first pops continue its depth-first order; then release
    /// the deque and wake one idle worker. The injected reservation is the sole
    /// fallible storage effect: a refusal publishes nothing and leaves the
    /// traversal as it was.
    fn publish(
        &self,
        index: usize,
        cell: &Arc<QueryCell>,
        traversal: &mut Walk<CountermodelKnowledge>,
        termination: &Termination,
        reserve: impl FnOnce(&mut VecDeque<InnerEntry>, usize) -> Result<(), Incomplete>,
    ) -> Result<(), Incomplete> {
        let mut queue = self.queue(index);
        reserve(&mut queue, traversal.pending())?;
        for pending in traversal.surrender() {
            queue.push_back(InnerEntry::new(pending, cell));
        }
        drop(queue);
        termination.wake_one();
        Ok(())
    }

    /// Admit both children while retaining the narrowed parent and its query.
    /// The cut knowledge is copied before locking. The reversible count gain
    /// precedes reservation; refusal withdraws it and leaves the exact parent
    /// in the occupied caller slot. Success consumes that slot only after
    /// admission. Errors remain the original typed refusal.
    /// After admission, queue held then cut so the inline query's cut-first
    /// order is unchanged, release the deque, and wake an idle worker.
    fn publish_split(
        &self,
        index: usize,
        pending: &mut Option<InnerEntry>,
        atom: usize,
        cut_knowledge: CountermodelKnowledge,
        termination: &Termination,
        reserve: impl FnOnce(&mut VecDeque<InnerEntry>, usize) -> Result<(), Incomplete>,
    ) -> Result<(), Incomplete> {
        let parent = pending.as_ref().expect("publication has a pending parent");
        let atoms = parent.entry.region.len();
        let mut queue = self.queue(index);
        // The parent is in hand and still counted.
        parent.cell.grow();
        if let Err(error) = reserve(&mut queue, 2) {
            parent.cell.shrink();
            return Err(error);
        }
        let InnerEntry { entry, cell } = pending.take().expect("admission retains its parent");
        let (cut, held) = entry.split_with(atom, |knowledge| (cut_knowledge, knowledge));
        queue.push_back(InnerEntry::new(held, &cell));
        queue.push_back(InnerEntry::new(cut, &cell));
        debug_assert!(
            queue.len() <= atoms + 1,
            "an inner deque outgrew its depth-first bound"
        );
        drop(queue);
        termination.wake_one();
        Ok(())
    }

    fn count_served(&self, index: usize) {
        self.served[index].fetch_add(1, Ordering::Relaxed);
    }

    /// The published regions served in total, the workers that served any,
    /// and the most one worker served.
    pub(super) fn served(&self) -> (u64, usize, u64) {
        self.served
            .iter()
            .map(|served| served.load(Ordering::Relaxed))
            .fold((0, 0, 0), |(total, servers, peak), served| {
                (
                    total.saturating_add(served),
                    servers + usize::from(served != 0),
                    peak.max(served),
                )
            })
    }

    /// Release every deque's payloads and capacity, once no party can still
    /// own or publish a published region.
    pub(super) fn release(&self) {
        for index in 0..self.queues.len() {
            drop(std::mem::take(&mut *self.queue(index)));
        }
    }
}

/// Entry-capacity growth preserves the prior deque and returns a typed
/// refusal.
fn reserve_entries(queue: &mut VecDeque<InnerEntry>, additional: usize) -> Result<(), Incomplete> {
    queue
        .try_reserve(additional)
        .map_err(|_| Incomplete::Allocation)
}

/// When a party's inline query publishes.
pub(super) enum Timing<'p> {
    /// Never: the coordinator of a walk that never engages or divides.
    Inline,
    /// A worker, as the walk's division says.
    Worker,
    /// The coordinator, once its engagement deadline has passed, as `passed`
    /// reads it.
    Coordinator {
        deadline: Instant,
        passed: &'p mut dyn FnMut(Instant) -> bool,
    },
}

/// A leaf query's publication onto worker `index`'s inner deque.
pub(super) struct WalkPublication<'s, 'p> {
    shared: &'s Shared,
    index: usize,
    timing: Timing<'p>,
    /// When the query first asked, before its first narrowing.
    started: Option<Instant>,
}

impl<'s, 'p> WalkPublication<'s, 'p> {
    pub(super) fn new(shared: &'s Shared, index: usize, timing: Timing<'p>) -> Self {
        Self {
            shared,
            index,
            timing,
            started: None,
        }
    }
}

impl Publication for WalkPublication<'_, '_> {
    type Receipt = ();

    fn due(&mut self) -> bool {
        match &mut self.timing {
            Timing::Inline => false,
            Timing::Coordinator { deadline, passed } => passed(*deadline),
            Timing::Worker => match self.shared.division {
                QueryDivision::Never => false,
                QueryDivision::Immediate => true,
                QueryDivision::WhenIdle => {
                    let started = *self.started.get_or_insert_with(Instant::now);
                    // The idle count is read without the gate: a stale read
                    // only moves the publication by a narrowing.
                    self.shared
                        .frontier
                        .termination
                        .idle_waiters
                        .load(Ordering::Relaxed)
                        != 0
                        && started.elapsed() >= INNER_DELAY
                }
            },
        }
    }

    fn publish(
        &mut self,
        truth: &[bool],
        candidate: &Interpretation,
        traversal: &mut Walk<CountermodelKnowledge>,
    ) -> Option<()> {
        let pending = traversal.pending();
        // A cell counting no region would never be decided.
        if pending == 0 {
            return None;
        }
        let cell = Arc::new(QueryCell::new(truth, candidate, pending).ok()?);
        self.shared
            .inner
            .publish(
                self.index,
                &cell,
                traversal,
                &self.shared.frontier.termination,
                reserve_entries,
            )
            .ok()?;
        Live::add(&self.shared.live.published, 1);
        Some(())
    }
}

/// Serve one published region, taken by worker `index` from its own deque or
/// a peer's: drop it if its query is decided, else narrow it as the inline
/// query would, on this worker's reduct workspace and lease, then retire it,
/// publish its children, or record its countermodel. Returns the candidate
/// when this region was its query's last and no countermodel was found: the
/// walk's count then no longer holds its leaf, and the caller sends it.
///
/// # Errors
/// The query's error, a stopped narrowing or decision, an invalid leaf, a
/// countermodel's validation refused or stopped, or a refused child
/// publication, once it is recorded as the query's stop and raised as the
/// walk's. The region is then undecided and is never counted as resolved. A
/// refused copy of a stable candidate is returned unrecorded.
pub(super) fn serve<'a>(
    shared: &'a Shared,
    original: &IndexedTheory,
    (entry, taken, index): (InnerEntry, Taken, usize),
    workspaces: &mut Workspaces<'a>,
    budget: &mut Budget<'a, WorkLease<'a>>,
    (report, reported): (&mut WorkerReport, &mut SearchPhaseTimings),
) -> Result<Option<Interpretation>, Incomplete> {
    let InnerEntry { mut entry, cell } = entry;
    if cell.is_resolved() {
        // A decided query's region carries no obligation.
        drop(entry);
        let decided = cell.retire();
        debug_assert!(!decided, "a decided query was decided again");
        return Ok(None);
    }
    shared.inner.count_served(index);
    if taken == Taken::Stolen {
        Live::add(&shared.live.stolen, 1);
    }
    let countermodels = report.statistics.countermodels;
    let started = timing::start(report.statistics.phase_timings.as_ref());
    let served = workspaces.membership.workspace.lend_narrowing(
        shared.limits.max_reduct_bytes,
        &mut report.statistics,
        |scratch, statistics| {
            ReductQuery::new(original).serve(
                &cell.truth,
                &cell.candidate,
                &mut entry,
                shared.limits,
                budget,
                (statistics, scratch),
            )
        },
    );
    timing::finish(&mut report.statistics.phase_timings, Phase::Reduct, started);
    publish_timings(shared, (report, reported));
    Live::add(
        &shared.live.countermodels,
        report.statistics.countermodels - countermodels,
    );
    match served {
        Ok(Served::Empty) => decide(shared, &cell),
        Ok(Served::Countermodel(witness)) => {
            // Absorption: the candidate is refuted and nothing is delivered.
            if cell.absorb(witness) {
                shared.frontier.termination.resolve();
            }
            decide(shared, &cell)
        }
        Ok(Served::Split(atom)) => {
            let cut_knowledge = entry.state.clone();
            let mut pending = Some(InnerEntry { entry, cell });
            match shared.inner.publish_split(
                index,
                &mut pending,
                atom,
                cut_knowledge,
                &shared.frontier.termination,
                reserve_entries,
            ) {
                Ok(()) => Ok(None),
                Err(error) => {
                    abandon(
                        shared,
                        pending.take().expect("refusal retains its parent"),
                        error,
                    );
                    Err(error)
                }
            }
        }
        Err(error) => {
            abandon(shared, InnerEntry { entry, cell }, error);
            Err(error)
        }
    }
}

/// Commit the query and walk stops while still owning the unresolved parent,
/// then retire its count and release it as terminal abandonment. Retirement
/// after halt cannot certify stability; this is not a resolved search region.
fn abandon(shared: &Shared, parent: InnerEntry, error: Incomplete) {
    parent.cell.halt(error);
    shared.stop(error);
    let decided = parent.cell.retire();
    debug_assert!(!decided, "a stopped query was decided stable");
    drop(parent);
}

/// Retire a resolved region of `cell`'s query. The last, with no witness and
/// no stop, decides the query stable: its leaf leaves the walk's count and the
/// candidate is returned to send.
fn decide(shared: &Shared, cell: &QueryCell) -> Result<Option<Interpretation>, Incomplete> {
    if !cell.retire() {
        return Ok(None);
    }
    let model = cell
        .candidate
        .try_clone()
        .map_err(|_| Incomplete::Allocation)?;
    shared.frontier.termination.resolve();
    Ok(Some(model))
}
