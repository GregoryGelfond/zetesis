//! The frontier protocols both parallel region routes run.
//!
//! A frontier is one deque of regions per worker and the termination record
//! that carries the count of unresolved regions and the gate idle workers
//! wait at. The CPU walker and the GPU-route producers share every protocol
//! here: taking a worker's own newest region, stealing a peer's oldest,
//! publishing a split's children, and the idle back-off that waits for work
//! or the end of the walk.
//!
//! Each deque has its own mutex: local removal locks only that deque, a split
//! publication locks that deque and then, after releasing it, the idle gate to
//! wake one idle worker, and a thief skips a busy peer. Region preparation and
//! evaluation hold no deque lock. Entry capacity grows fallibly and before any
//! child is published. An atomic counter of unresolved regions carries
//! termination: a split raises it before pushing its children, a resolved
//! region lowers it once, and an unsuccessful thief stops only once that
//! counter has reached zero. An atomic closed flag, set on the first stop or
//! when the walk's consumer stops listening, halts the others at their next
//! region. Idle workers wait at a gate that the last resolution and every
//! close open, and that a publication opens for one waiter, so none sleeps
//! out its timed wait after the walk has ended or while a region it could
//! take is published. The timed wait bounds how late an idle worker sees a
//! cancellation, and how late it finds a region left in a deque that was busy
//! when it last looked.

use std::collections::VecDeque;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError, TryLockError};
use std::time::Duration;

use zetesis_cpu::regions::Region;

use super::conditions::CandidateKnowledge;
use crate::{Cancellation, Incomplete};

/// A region with its per-narrower knowledge: the unit a worker takes,
/// publishes and steals.
pub(super) type Entry = (Region, CandidateKnowledge);

/// Idle steal rounds a worker spins (yielding) before it waits for the walk
/// to end or the wait to time out: the busy-wait/wait backoff crossover.
const SPIN_ROUNDS_BEFORE_WAIT: u32 = 16;

/// An entry's owned allocations, beyond the entry itself, which its deque's
/// entry capacity holds: the region's and knowledge's owned vectors, including
/// unused knowledge slots.
pub(super) fn entry_payload((region, knowledge): &Entry) -> u128 {
    region.retained_bytes() - size_of::<Region>() as u128 + knowledge.allocated_bytes()
}

/// Entry-capacity growth preserves the prior deque and returns a typed
/// refusal.
pub(super) fn reserve_regions(
    queue: &mut VecDeque<Entry>,
    additional: usize,
) -> Result<(), Incomplete> {
    queue
        .try_reserve(additional)
        .map_err(|_| Incomplete::Allocation)
}

/// What ends the walk — every region resolved, or a close — and the gate idle
/// workers wait at for either.
///
/// The count and the flag are atomics because the region loop reads them for
/// every region; the gate guards the number of idle waiters. Every transition
/// that ends the walk changes its atomic first and then takes the gate to wake
/// the waiters, and an idle worker takes the gate and re-checks both conditions,
/// and the peers' deques, before it waits. Whichever reaches the gate first, no
/// idle worker waits past the end of the walk: a worker that takes the gate after
/// the ending transition sees it, and one that took the gate before is already
/// waiting when the wake comes. A publication pushes its children, releases its
/// deque and then takes the gate to wake one waiter, so by the same argument no
/// waiter misses a region published after its re-check. Zero is terminal, since
/// a region in hand counts until it is resolved. Every wait stays bounded by its
/// timeout, so correctness never rests on a wake: the timeout is how an idle
/// worker sees a cancellation, or a region left in a deque its re-check found
/// busy.
pub(super) struct Termination {
    /// Created-but-unresolved regions across every worker's deque and hand: a
    /// split adds one (before pushing its children), a refuted or decided
    /// region subtracts one. It starts at one for the root and reaches zero
    /// exactly when the whole frontier is resolved; a read of zero is a true
    /// zero, so a worker that finds every deque empty and reads zero is done.
    outstanding: AtomicUsize,
    /// A stop was raised (error or cancellation) or the walk's consumer
    /// stopped listening; workers check it before each region and while idle,
    /// and exit. Normal termination is `outstanding` reaching zero.
    closed: AtomicBool,
    /// The number of idle workers waiting on `idle`. Held to re-check the
    /// walk's end and the peers' deques before an idle wait, to wake one waiter
    /// after a publication, and to wake every waiter after the walk ends.
    pub(super) gate: Mutex<usize>,
    /// Idle workers wait here for a published region or the end of the walk.
    idle: Condvar,
}

impl Termination {
    /// The root is the one outstanding region until it is split.
    pub(super) fn new() -> Self {
        Self {
            outstanding: AtomicUsize::new(1),
            closed: AtomicBool::new(false),
            gate: Mutex::new(0),
            idle: Condvar::new(),
        }
    }

    /// A split replaces its parent by two children: count the net gain of one
    /// before either child becomes visible.
    pub(super) fn grow(&self) {
        self.outstanding.fetch_add(1, Ordering::AcqRel);
    }

    /// A refuted or decided region leaves the frontier. The last one ends the
    /// walk and wakes every idle worker.
    pub(super) fn resolve(&self) {
        let previous = self.outstanding.fetch_sub(1, Ordering::AcqRel);
        debug_assert_ne!(previous, 0, "a region was resolved twice");
        if previous == 1 {
            self.wake_idle();
        }
    }

    /// Close the walk: workers take no further region, and idle workers wake.
    pub(super) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.wake_idle();
    }

    /// The created-but-unresolved regions, as the workers read them.
    pub(super) fn unresolved(&self) -> usize {
        self.outstanding.load(Ordering::Acquire)
    }

    pub(super) fn is_resolved(&self) -> bool {
        self.unresolved() == 0
    }

    pub(super) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub(super) fn has_ended(&self) -> bool {
        self.is_closed() || self.is_resolved()
    }

    /// Wake one idle worker, if any waits, after a publication.
    pub(super) fn wake_one(&self) {
        let waiting = self.gate.lock().unwrap_or_else(PoisonError::into_inner);
        if *waiting != 0 {
            self.idle.notify_one();
        }
    }

    /// Wake every idle worker, after the transition that ended the walk.
    fn wake_idle(&self) {
        let _gate = self.gate.lock().unwrap_or_else(PoisonError::into_inner);
        self.idle.notify_all();
    }
}

/// Where an attempt to take a region runs. Outside the gate a worker may wait
/// for its own deque's lock; under the gate only `try_lock` is used, so a
/// waiter never waits for a deque lock while it holds the gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Attempt {
    Outside,
    UnderGate,
}

/// One deque of regions per worker and the walk's termination record.
///
/// An owner pops its newest region and thieves take the oldest. At most one
/// deque lock is held at a time, and none while the gate is waited for.
pub(super) struct RegionFrontier {
    pub(super) queues: Vec<Mutex<VecDeque<Entry>>>,
    /// Whether the walk has ended, and where idle workers wait for its end.
    pub(super) termination: Termination,
}

impl RegionFrontier {
    /// Deque 0 holds `root`; the others start empty. Storage for the deques
    /// and the root's entry is reserved fallibly.
    pub(super) fn new(workers: usize, root: Entry) -> Result<Self, Incomplete> {
        let mut queues = crate::search::storage(workers)?;
        let mut first = VecDeque::new();
        reserve_regions(&mut first, 1)?;
        first.push_back(root);
        queues.push(Mutex::new(first));
        for _ in 1..workers {
            queues.push(Mutex::new(VecDeque::new()));
        }
        Ok(Self {
            queues,
            termination: Termination::new(),
        })
    }

    pub(super) fn queue(&self, index: usize) -> MutexGuard<'_, VecDeque<Entry>> {
        self.queues[index]
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Worker `index`'s newest region.
    pub(super) fn take_local(&self, index: usize) -> Option<Entry> {
        self.queue(index).pop_back()
    }

    /// Take the oldest region of a peer's deque, trying peers in round-robin
    /// order and skipping a busy one, so one owner's growth cannot block
    /// stealing from the others. No two queue locks overlap, and none is
    /// waited for.
    pub(super) fn steal(&self, index: usize) -> Option<Entry> {
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

    /// Wait at the gate for a published region or the end of the walk, for at
    /// most `timeout`. Under the gate, the walk's end and `try_take` are
    /// re-checked first; a region found then is returned at once. `None` does
    /// not mean the walk ended: the caller re-checks.
    pub(super) fn wait_for_work(
        &self,
        timeout: Duration,
        mut try_take: impl FnMut(Attempt) -> Option<Entry>,
    ) -> Option<Entry> {
        let mut waiting = self
            .termination
            .gate
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if self.termination.has_ended() {
            return None;
        }
        if let Some(entry) = try_take(Attempt::UnderGate) {
            return Some(entry);
        }
        *waiting += 1;
        let (mut waiting, _) = self
            .termination
            .idle
            .wait_timeout(waiting, timeout)
            .unwrap_or_else(PoisonError::into_inner);
        *waiting -= 1;
        None
    }

    /// Publish a split's children on worker `index`'s deque: reserve their
    /// entry capacity, count the split's net gain of one, push them, release
    /// the deque, then wake one idle worker. The injected reservation is the
    /// sole fallible storage effect; success must reserve `N` entries, as
    /// [`reserve_regions`] does. A refused reservation publishes nothing and
    /// leaves the count unchanged.
    pub(super) fn publish_split<const N: usize>(
        &self,
        index: usize,
        children: [Entry; N],
        reserve: impl FnOnce(&mut VecDeque<Entry>, usize) -> Result<(), Incomplete>,
    ) -> Result<(), Incomplete> {
        let mut queue = self.queue(index);
        reserve(&mut queue, N)?;
        // The parent is still outstanding. Count its net gain before making
        // any child visible; the reserved pushes cannot allocate.
        self.termination.grow();
        for child in children {
            queue.push_back(child);
        }
        // Release the deque before taking the gate: a waiter holds the gate
        // while it tries the deques, and never waits for a queue lock.
        drop(queue);
        self.termination.wake_one();
        Ok(())
    }

    /// Look for a region, waiting while any remains. `None` when the whole
    /// frontier is resolved or the walk closed; a cancellation observed while
    /// idle is reported through `on_stop` first. `try_take` makes one attempt
    /// and is told where it runs. Every idle round makes one attempt outside
    /// the gate; of every sixteen rounds, the first fifteen then yield and the
    /// last waits at the gate, where it attempts once more, and then polls
    /// cancellation. `idle_wait` bounds that wait; a publication or the end of
    /// the walk ends it at once.
    pub(super) fn find_work(
        &self,
        idle_wait: Duration,
        cancellation: &Cancellation,
        mut try_take: impl FnMut(Attempt) -> Option<Entry>,
        on_stop: impl FnOnce(Incomplete),
    ) -> Option<Entry> {
        let mut idle_rounds = 0u32;
        loop {
            if self.termination.is_closed() {
                return None;
            }
            if let Some(entry) = try_take(Attempt::Outside) {
                return Some(entry);
            }
            // No attempt succeeded. Even with busy queues, zero means every
            // created region was resolved; a split counts its children before
            // publishing.
            if self.termination.is_resolved() {
                return None;
            }
            // Busy queues and empty queues share the backoff, so lock
            // contention cannot postpone cooperative control checks
            // indefinitely.
            idle_rounds += 1;
            if idle_rounds < SPIN_ROUNDS_BEFORE_WAIT {
                std::thread::yield_now();
            } else {
                if let Some(entry) = self.wait_for_work(idle_wait, &mut try_take) {
                    return Some(entry);
                }
                if let Err(error) = cancellation.poll() {
                    on_stop(error.into());
                    return None;
                }
                idle_rounds = 0;
            }
        }
    }

    /// Release every deque's payloads and capacity, once no party can still
    /// own or publish work.
    pub(super) fn release(&self) {
        for index in 0..self.queues.len() {
            drop(std::mem::take(&mut *self.queue(index)));
        }
    }
}
