//! Shared work-stealing frontier and termination protocols.
//!
//! A frontier is one deque of regions per worker and the termination record
//! that carries the count of unresolved regions and the gate idle workers
//! wait at. The CPU walker and the GPU-route producers share every protocol
//! here: taking a worker's own newest region, stealing a peer's oldest,
//! publishing a split's children, and the idle back-off that waits for work
//! or the end of the walk.
//!
//! Each deque has its own mutex: local removal locks only that deque, a split
//! publication locks that deque and then, after releasing it and only while a
//! worker is idle, the idle gate to wake one, and a thief skips a busy peer.
//! Region preparation and evaluation hold no deque lock. Entry capacity grows
//! fallibly and before any child is published. An atomic counter of unresolved
//! regions carries termination: a split raises it before pushing its children,
//! a resolved region lowers it once, and an unsuccessful thief stops only once
//! that counter has reached zero. An atomic closed flag, set on the first stop
//! or when the walk's consumer stops listening, halts the others at their next
//! region. Idle workers wait at a gate that the last resolution and every
//! close open, and that a publication opens for one waiter, so none sleeps
//! out its timed wait after the walk has ended or while a region it could
//! take is published. A publication reads the number of idle workers without
//! the gate and takes the gate only when one is counted, so busy workers
//! never contend for it. The timed wait bounds how late an idle worker sees a
//! cancellation, and how late it finds a region left in a deque that was busy
//! when it last looked.

use std::collections::VecDeque;
use std::marker::PhantomData;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering, fence};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError, TryLockError};
use std::time::Duration;

use crate::{Cancellation, Stop};

#[cfg(test)]
mod tests;

/// Allocation metadata of one frontier entry, beyond its inline representation.
///
/// The frontier accounts for `size_of::<Self>()` through deque capacity. Return
/// only allocations owned by this entry, including unused capacity and excluding
/// shared owners, the inline entry/header bytes, temporary storage and allocator
/// metadata. The value must remain stable while the entry is queued. Actual
/// owned allocations fit the host address range; a larger reported value is
/// clamped to `usize::MAX`, preserving the frontier's host-counter conversion.
/// This capability supplies no narrowing or membership decision.
pub trait Payload {
    /// Bytes of allocations owned by the entry, excluding the inline entry.
    fn allocated_bytes(&self) -> u128;
}

/// Idle steal rounds a worker spins (yielding) before it waits for the walk
/// to end or the wait to time out: the busy-wait/wait backoff crossover.
const SPIN_ROUNDS_BEFORE_WAIT: u32 = 16;

/// Grow entry capacity before publication, preserving the prior deque on refusal.
///
/// # Errors
/// Returns `R::from(Stop::Allocation)` when the requested capacity cannot be
/// reserved. No entry is published by this operation.
pub fn reserve_entries<E, R: From<Stop>>(
    queue: &mut VecDeque<E>,
    additional: usize,
) -> Result<(), R> {
    queue
        .try_reserve(additional)
        .map_err(|_| R::from(Stop::Allocation))
}

/// What ends the walk — every region resolved, or a close — and the gate idle
/// workers wait at for either.
///
/// The count and the flag are atomics because the region loop reads them for
/// every region. Every transition that ends the walk changes its atomic first
/// and then takes the gate to wake the waiters, and an idle worker takes the
/// gate and re-checks both conditions, and the peers' deques, before it waits.
/// Whichever reaches the gate first, no idle worker waits past the end of the
/// walk: a worker that takes the gate after the ending transition sees it, and
/// one that took the gate before is already waiting when the wake comes.
///
/// The number of idle waiters is an atomic changed only while the gate is
/// held, and a publication reads it without the gate. A waiter registers
/// itself, issues a sequentially consistent fence, and only then re-checks
/// the deques; a publication pushes its children, releases its deque, issues
/// a sequentially consistent fence, and only then reads the count, taking the
/// gate to wake one waiter when it is not zero. The two fences are totally
/// ordered. If the publication's comes first, the waiter's re-check observes
/// the released deque and so the pushed child; if the waiter's comes first,
/// the publication reads its registration and takes the gate, which the waiter
/// holds until it is waiting, so the wake reaches it. Either the waiter sees
/// the child or the publication sees the waiter. Zero is terminal, since a
/// region in hand counts until it is resolved. Every wait stays bounded by its
/// timeout, so correctness never rests on a wake: the timeout is how an idle
/// worker sees a cancellation, or a region left in a deque its re-check found
/// busy.
///
/// The first case rests on a premise about the platform's mutex: releasing it
/// and `try_lock` are atomic operations on its lock word, as they are in std's
/// own futex mutex (Linux, Windows) and in the pthread mutex std uses on
/// macOS, so the fences order them as they order any atomic access. A re-check
/// after the publication's fence therefore finds the deque released, or locked
/// again by a later holder, which costs the waiter a bounded wait. Were the
/// premise to fail, a waiter could also find the publication still holding the
/// deque, and would find the child after its bounded wait.
pub struct Termination {
    /// Created-but-unresolved regions across every worker's deque and region
    /// in hand: a split adds one (before pushing its children), a refuted or
    /// decided region subtracts one. It starts at one for the root and reaches
    /// zero exactly when the whole frontier is resolved; a read of zero is a
    /// true zero, so a worker that finds every deque empty and reads zero is
    /// done.
    outstanding: AtomicUsize,
    /// A stop was raised (error or cancellation) or the walk's consumer
    /// stopped listening; workers check it before each region and while idle,
    /// and exit. Normal termination is `outstanding` reaching zero.
    closed: AtomicBool,
    /// Held to register and re-check before an idle wait, to wake one waiter
    /// after a publication that counted one, and to wake every waiter after
    /// the walk ends.
    pub gate: Mutex<()>,
    /// The number of idle workers waiting on `idle`: changed only while
    /// `gate` is held, read by a publication without it.
    pub idle_waiters: AtomicUsize,
    /// Idle workers wait here for a published region or the end of the walk.
    idle: Condvar,
}

impl Termination {
    /// The root is the one outstanding region until it is split.
    fn new() -> Self {
        Self::resuming(1)
    }

    /// A walk that continues `unresolved` regions left by an earlier joined
    /// one, which no worker holds: the count is carried, the walk is open and
    /// nobody waits.
    #[must_use]
    pub fn resuming(unresolved: usize) -> Self {
        Self {
            outstanding: AtomicUsize::new(unresolved),
            closed: AtomicBool::new(false),
            gate: Mutex::new(()),
            idle_waiters: AtomicUsize::new(0),
            idle: Condvar::new(),
        }
    }

    /// A split replaces its parent by two children: count the net gain of one
    /// before either child becomes visible.
    pub fn grow(&self) {
        self.outstanding.fetch_add(1, Ordering::AcqRel);
    }

    /// A refuted or decided region leaves the frontier. The last one ends the
    /// walk and wakes every idle worker.
    pub fn resolve(&self) {
        let previous = self.outstanding.fetch_sub(1, Ordering::AcqRel);
        debug_assert_ne!(previous, 0, "a region was resolved twice");
        if previous == 1 {
            self.wake_idle();
        }
    }

    /// Close the walk: workers take no further region, and idle workers wake.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.wake_idle();
    }

    /// The created-but-unresolved regions, as the workers read them.
    #[must_use]
    pub fn unresolved(&self) -> usize {
        self.outstanding.load(Ordering::Acquire)
    }

    /// Whether every created entry has been resolved.
    #[must_use]
    pub fn is_resolved(&self) -> bool {
        self.unresolved() == 0
    }

    /// Whether a caller closed the walk.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    /// Whether the walk is closed or has no unresolved entry.
    #[must_use]
    pub fn has_ended(&self) -> bool {
        self.is_closed() || self.is_resolved()
    }

    /// Wake one idle worker, if any is counted, after a publication released
    /// its deque. The fence orders the publication before the count's read;
    /// see the type's documentation for why no waiter is missed.
    pub fn wake_one(&self) {
        fence(Ordering::SeqCst);
        if self.idle_waiters.load(Ordering::Relaxed) != 0 {
            let _gate = self.gate.lock().unwrap_or_else(PoisonError::into_inner);
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
pub enum Attempt {
    /// Outside the idle gate; local removal may wait for its deque.
    Outside,
    /// While holding the idle gate; only nonblocking deque attempts are permitted.
    UnderGate,
}

/// A worker's own deque found locked by another worker: its contents are
/// unknown, so it may still hold regions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Busy;

/// One counter of queued storage and the maximum it has held.
struct Counter {
    current: AtomicUsize,
    peak: AtomicUsize,
}

impl Counter {
    fn starting_at(value: usize) -> Self {
        Self {
            current: AtomicUsize::new(value),
            peak: AtomicUsize::new(value),
        }
    }

    /// Raise the counter and fold its new value into the peak.
    fn add(&self, amount: usize) {
        if amount != 0 {
            let previous = self.current.fetch_add(amount, Ordering::Relaxed);
            self.peak
                .fetch_max(previous.saturating_add(amount), Ordering::Relaxed);
        }
    }

    fn subtract(&self, amount: usize) {
        self.current.fetch_sub(amount, Ordering::Relaxed);
    }

    fn set(&self, value: usize) {
        self.current.store(value, Ordering::Relaxed);
    }

    fn current(&self) -> usize {
        self.current.load(Ordering::Relaxed)
    }

    fn peak(&self) -> usize {
        self.peak.load(Ordering::Relaxed)
    }
}

/// Exact receipts of a frontier's queued regions and their storage.
///
/// Each counter changes by a relaxed read-modify-write under the deque lock of
/// the change that causes it, and an increase folds its result into the
/// counter's peak. Updates to one counter are totally ordered by its
/// modification order, and each is applied while its deque's change is held,
/// so the counter's history is a linearization of the deques' changes and its
/// peak the exact maximum of that history. Counters of different kinds are
/// separate histories, so their peaks need not occur together. They are read
/// once no party can change the frontier.
struct Accounting {
    regions: Counter,
    capacity: Counter,
    /// The deque vector's header and its deques' headers, every deque's entry
    /// capacity times the entry size, and every queued entry's owned payload.
    retained_bytes: Counter,
    /// The headers' share of `retained_bytes`, constant for a frontier.
    headers: usize,
}

/// An entry's owned payload as a count of host bytes. The payload measures
/// actual allocations, so it fits the host's address range.
fn payload_bytes<E: Payload>(entry: &E) -> usize {
    usize::try_from(entry.allocated_bytes()).unwrap_or(usize::MAX)
}

/// Owned storage of queued, inactive entries in the frontier's deques, at frontier mutations.
///
/// Bytes include the deque vector's and each deque's header, the deques'
/// allocated entry capacity, used or not, and each queued entry's owned allocations
/// reported by [`Payload`]. They exclude the regions producers
/// hold, temporary split copies, shared owners, output batches,
/// measurement bookkeeping, thread stacks and allocator/driver storage. This
/// observation is neither total search memory nor a byte admission limit. Each
/// count changes under the lock of the deque whose change causes it, so its
/// peak is the maximum of a linearization of the deques' changes; peaks need
/// not occur at the same mutation, and an empty frontier can retain entry
/// capacity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    /// Currently queued inactive entries; the historical receipt field is `regions`.
    pub regions: usize,
    /// Current entry capacity of the deques: the regions their storage can
    /// queue without growing, used or not.
    pub capacity: usize,
    /// Current logical retained bytes in the stated ownership scope.
    pub retained_bytes: u128,
    /// Maximum number of simultaneously queued regions.
    pub peak_regions: usize,
    /// Maximum allocated entry capacity.
    pub peak_capacity: usize,
    /// Maximum logical retained bytes at a frontier mutation.
    pub peak_retained_bytes: u128,
}

/// One deque of entries per worker and the walk's termination record.
///
/// An owner pops its newest region and thieves take the oldest. At most one
/// deque lock is held at a time, and none while the gate is waited for. A
/// measured frontier keeps exact receipts of its queued regions; its deques
/// are then changed only through these operations.
pub struct Frontier<E, R> {
    /// Worker deques. Measured mutations must use the frontier's operations;
    /// direct mutation would bypass its ownership receipts.
    pub queues: Vec<Mutex<VecDeque<E>>>,
    /// Whether the walk has ended, and where idle workers wait for its end.
    pub termination: Termination,
    accounting: Option<Accounting>,
    refusal: PhantomData<fn() -> R>,
}

impl<E: Payload, R: From<Stop>> Frontier<E, R> {
    /// Deque 0 holds `root`; the others start empty. Storage for the deques
    /// and the root's entry is reserved fallibly. The frontier keeps no
    /// receipts. As in the existing protocol, callers supply at least one worker.
    ///
    /// # Errors
    /// Returns the caller's allocation refusal if either reservation fails.
    pub fn new(workers: usize, root: E) -> Result<Self, R> {
        let mut queues = Vec::new();
        queues
            .try_reserve_exact(workers)
            .map_err(|_| R::from(Stop::Allocation))?;
        let mut first = VecDeque::new();
        reserve_entries::<E, R>(&mut first, 1)?;
        first.push_back(root);
        queues.push(Mutex::new(first));
        for _ in 1..workers {
            queues.push(Mutex::new(VecDeque::new()));
        }
        Ok(Self {
            queues,
            termination: Termination::new(),
            accounting: None,
            refusal: PhantomData,
        })
    }

    /// [`Self::new`], keeping exact receipts of the queued regions and their
    /// storage from the root onward. Payload measurement precedes reservation,
    /// and the counter scope excludes this record's measurement bookkeeping.
    ///
    /// # Errors
    /// Returns the same typed allocation refusal as [`Self::new`].
    pub fn measured(workers: usize, root: E) -> Result<Self, R> {
        let payload = payload_bytes(&root);
        let mut frontier = Self::new(workers, root)?;
        let headers = size_of::<Vec<Mutex<VecDeque<E>>>>()
            + frontier.queues.capacity() * size_of::<Mutex<VecDeque<E>>>();
        let capacity = frontier.queue(0).capacity();
        frontier.accounting = Some(Accounting {
            regions: Counter::starting_at(1),
            capacity: Counter::starting_at(capacity),
            retained_bytes: Counter::starting_at(headers + capacity * size_of::<E>() + payload),
            headers,
        });
        Ok(frontier)
    }

    /// The frontier's receipts, when it keeps them.
    #[must_use]
    pub fn statistics(&self) -> Option<Statistics> {
        let accounting = self.accounting.as_ref()?;
        Some(Statistics {
            regions: accounting.regions.current(),
            capacity: accounting.capacity.current(),
            retained_bytes: accounting.retained_bytes.current() as u128,
            peak_regions: accounting.regions.peak(),
            peak_capacity: accounting.capacity.peak(),
            peak_retained_bytes: accounting.retained_bytes.peak() as u128,
        })
    }

    /// Record a removal from a deque whose lock the caller holds.
    fn removed(&self, entry: &E) {
        if let Some(accounting) = &self.accounting {
            accounting.regions.subtract(1);
            accounting.retained_bytes.subtract(payload_bytes(entry));
        }
    }

    /// Record an insertion into a deque whose lock the caller holds.
    fn inserted(&self, entry: &E) {
        if let Some(accounting) = &self.accounting {
            accounting.regions.add(1);
            accounting.retained_bytes.add(payload_bytes(entry));
        }
    }

    /// Record a capacity change of a deque whose lock the caller holds.
    fn resized(&self, before: usize, after: usize) {
        if let Some(accounting) = &self.accounting
            && after > before
        {
            let grown = after - before;
            accounting.capacity.add(grown);
            accounting.retained_bytes.add(grown * size_of::<E>());
        }
    }

    /// Lock a worker deque, recovering its contents if an earlier holder panicked.
    /// Measured mutations must use the frontier operations, not this raw guard.
    ///
    /// # Panics
    /// Panics if `index` is outside the worker deques.
    pub fn queue(&self, index: usize) -> MutexGuard<'_, VecDeque<E>> {
        self.queues[index]
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Worker `index`'s newest entry.
    ///
    /// # Panics
    /// Panics if `index` is outside the worker deques.
    pub fn take_local(&self, index: usize) -> Option<E> {
        let mut queue = self.queue(index);
        let entry = queue.pop_back()?;
        self.removed(&entry);
        Some(entry)
    }

    /// Worker `index`'s newest region without waiting for its deque's lock:
    /// the gate's form of [`Self::take_local`]. `Ok(None)` means the deque was
    /// empty, and [`Busy`] that another worker held it, which says nothing of
    /// its contents.
    ///
    /// # Errors
    /// Returns [`Busy`] when another holder has the deque lock.
    ///
    /// # Panics
    /// Panics if `index` is outside the worker deques.
    pub fn try_take_local(&self, index: usize) -> Result<Option<E>, Busy> {
        let mut queue = match self.queues[index].try_lock() {
            Ok(queue) => queue,
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
            Err(TryLockError::WouldBlock) => return Err(Busy),
        };
        let entry = queue.pop_back();
        if let Some(entry) = &entry {
            self.removed(entry);
        }
        Ok(entry)
    }

    /// Return a region to worker `index`'s deque without changing the count:
    /// a region taken but neither resolved nor split goes back. The injected
    /// reservation is the sole fallible storage effect, as in
    /// [`Self::publish_split`]. When it refuses entry capacity the region is
    /// dropped and the refusal returned, for the caller to record as a stop:
    /// the count still includes the region, so the run cannot claim
    /// exhaustion, and the stop says why.
    ///
    /// # Errors
    /// Propagates the injected reservation refusal without publishing the entry.
    ///
    /// # Panics
    /// Panics if `index` is outside the worker deques.
    pub fn requeue(
        &self,
        index: usize,
        entry: E,
        reserve: impl FnOnce(&mut VecDeque<E>, usize) -> Result<(), R>,
    ) -> Result<(), R> {
        let mut queue = self.queue(index);
        let before = queue.capacity();
        reserve(&mut queue, 1)?;
        self.resized(before, queue.capacity());
        self.inserted(&entry);
        queue.push_back(entry);
        Ok(())
    }

    /// Take the oldest region of a peer's deque, trying peers in round-robin
    /// order and skipping a busy one, so one owner's growth cannot block
    /// stealing from the others. No two queue locks overlap, and none is
    /// waited for.
    pub fn steal(&self, index: usize) -> Option<E> {
        let workers = self.queues.len();
        for offset in 1..workers {
            let mut queue = match self.queues[(index + offset) % workers].try_lock() {
                Ok(queue) => queue,
                Err(TryLockError::Poisoned(error)) => error.into_inner(),
                Err(TryLockError::WouldBlock) => continue,
            };
            if let Some(entry) = queue.pop_front() {
                self.removed(&entry);
                return Some(entry);
            }
        }
        None
    }

    /// Wait at the gate for a published region or the end of the walk, for at
    /// most `timeout`. Under the gate the waiter registers, fences, and then
    /// re-checks the walk's end and `try_take`; a region found then is returned
    /// at once. `None` does not mean the walk ended: the caller re-checks.
    /// What a region is, a frontier entry or another deque's item taken beside
    /// it, is `try_take`'s.
    pub fn wait_for_work<T>(
        &self,
        timeout: Duration,
        mut try_take: impl FnMut(Attempt) -> Option<T>,
    ) -> Option<T> {
        let termination = &self.termination;
        let gate = termination
            .gate
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        termination.idle_waiters.fetch_add(1, Ordering::Relaxed);
        // Pairs with the publication's fence: see `Termination`.
        fence(Ordering::SeqCst);
        let ended = termination.has_ended();
        let found = if ended {
            None
        } else {
            try_take(Attempt::UnderGate)
        };
        if ended || found.is_some() {
            termination.idle_waiters.fetch_sub(1, Ordering::Relaxed);
            return found;
        }
        let _gate = termination
            .idle
            .wait_timeout(gate, timeout)
            .unwrap_or_else(PoisonError::into_inner);
        termination.idle_waiters.fetch_sub(1, Ordering::Relaxed);
        None
    }

    /// Publish a split's children on worker `index`'s deque: reserve their
    /// entry capacity, count the split's net gain of one, push them, release
    /// the deque, then wake one idle worker. The injected reservation is the
    /// sole fallible storage effect; success must reserve `N` entries, as
    /// [`reserve_entries`] does. A refused reservation publishes nothing and
    /// leaves the count unchanged. A publication of no child would count a
    /// region that no deque or hand holds, so the count could never reach
    /// zero; it is refused when the call is compiled. The split is binary even
    /// when only one child is queued: the caller retains the other in hand.
    /// Therefore its unresolved count always grows by one, never by `N-1`.
    ///
    /// # Errors
    /// Propagates the injected reservation refusal before changing the count.
    ///
    /// # Panics
    /// Panics if `index` is outside the worker deques.
    pub fn publish_split<const N: usize>(
        &self,
        index: usize,
        children: [E; N],
        reserve: impl FnOnce(&mut VecDeque<E>, usize) -> Result<(), R>,
    ) -> Result<(), R> {
        const { assert!(N > 0, "a split publishes at least one child") };
        let mut queue = self.queue(index);
        let before = queue.capacity();
        reserve(&mut queue, N)?;
        self.resized(before, queue.capacity());
        // The parent is still outstanding. Count its net gain before making
        // any child visible; the reserved pushes cannot allocate.
        self.termination.grow();
        for child in children {
            self.inserted(&child);
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
    pub fn find_work<T>(
        &self,
        idle_wait: Duration,
        cancellation: &Cancellation,
        mut try_take: impl FnMut(Attempt) -> Option<T>,
        on_stop: impl FnOnce(R),
    ) -> Option<T> {
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
    /// own or publish work. A measured frontier's current receipts become its
    /// headers alone; its peaks remain.
    pub fn release(&self) {
        for index in 0..self.queues.len() {
            drop(std::mem::take(&mut *self.queue(index)));
        }
        if let Some(accounting) = &self.accounting {
            accounting.regions.set(0);
            accounting.capacity.set(0);
            accounting.retained_bytes.set(accounting.headers);
        }
    }
}
