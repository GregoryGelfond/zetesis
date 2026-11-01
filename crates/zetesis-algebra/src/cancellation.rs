//! Shared cancellation, deadlines, and typed incomplete outcomes.

use std::fmt;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;
use std::time::Instant;

mod slot;

pub use slot::{CancellationRun, CancellationSlot, CancellationSlotError};

/// The most operations a metered computation charges between two observations
/// of its cancellation and deadline flags. A poll reads up to three shared
/// flags; reading them once per this many operations instead of at every
/// operation takes that read out of per-node loops, while at most
/// `POLL_INTERVAL - 1` further operations can follow a requested stop.
pub const POLL_INTERVAL: u64 = 256;

/// Whether admitting `amount` operations after `charged` contains an
/// observation boundary. Operation numbers are zero-based multiples of
/// [`POLL_INTERVAL`]. A zero amount admits nothing and is never due.
/// Computes no counter sum, so the decision cannot overflow. This is a count
/// boundary; it makes no promise about wall-clock latency or unmetered work.
#[must_use]
pub const fn poll_due(charged: u64, amount: u64) -> bool {
    let offset = charged % POLL_INTERVAL;
    amount != 0 && (offset == 0 || POLL_INTERVAL - offset < amount)
}

/// Why a metered computation could not charge another operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChargeError {
    /// Control requested a stop or the deadline expired. A stop takes
    /// precedence over the work limit.
    Stopped(Stop),
    /// The computation has charged every operation its work limit admits.
    WorkLimit,
}

/// An error a metered operation reports, classified so that a requested stop
/// takes precedence over every resource refusal at the operation's exit
/// ([`Cancellation::metered`]).
pub trait Refusal: Sized {
    /// Whether this error refuses a resource: work, storage, a named ceiling,
    /// a size the host cannot represent, or an allocation. Errors about the
    /// program's shape, identity or invariants are not resource refusals.
    fn is_resource_refusal(&self) -> bool;

    /// This error type's report of a requested stop.
    fn from_stop(stop: Stop) -> Self;
}

/// A caller-owned account granting permits to one metered operation.
/// A refusal grants nothing. A successful grant is in `1..=wanted`; zero is
/// treated as a local work refusal. The cumulative grant retained by one meter
/// must fit `u64`, including already performed work. The account retains its
/// own typed errors.
pub trait Quota {
    /// The account's exact refusal, including any resource or source payload.
    type Error: Refusal;

    /// Reserve at most `wanted` permits before their operations execute.
    ///
    /// # Errors
    /// Returns the account's refusal without granting any permits.
    fn reserve(&mut self, wanted: u64) -> Result<u64, Self::Error>;

    /// Return granted permits whose operations did not execute.
    fn refund(&mut self, unspent: u64);
}

/// A caller refusal or a stop originating in the common work boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure<E> {
    /// The caller's original error, unchanged.
    Refused(E),
    /// Control or a local work, allocation or invariant refusal.
    Stopped(Stop),
}

impl<E> From<Stop> for Failure<E> {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

impl<E: Refusal> Refusal for Failure<E> {
    fn is_resource_refusal(&self) -> bool {
        match self {
            Self::Refused(error) => error.is_resource_refusal(),
            Self::Stopped(stop) => stop.is_resource_refusal(),
        }
    }

    fn from_stop(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

impl Failure<Stop> {
    /// Recover the original stop for an operation with no distinct caller error.
    #[must_use]
    pub const fn into_stop(self) -> Stop {
        match self {
            Self::Refused(stop) | Self::Stopped(stop) => stop,
        }
    }
}

/// The default meter has a local ceiling and no borrowed account.
/// This uninhabited marker adds no account or public construction policy.
#[derive(Debug)]
pub enum LocalQuota {}

impl Quota for LocalQuota {
    type Error = Stop;

    fn reserve(&mut self, _: u64) -> Result<u64, Stop> {
        match *self {}
    }

    fn refund(&mut self, _: u64) {
        match *self {}
    }
}

impl Refusal for Stop {
    fn is_resource_refusal(&self) -> bool {
        match self {
            Self::WorkLimit
            | Self::RoundLimit
            | Self::StorageLimit
            | Self::DerivedAtomLimit
            | Self::CandidateLimit
            | Self::CarrierLimit
            | Self::Allocation => true,
            Self::Cancelled | Self::Deadline | Self::WrongProgram | Self::InvalidProgram => false,
        }
    }

    fn from_stop(stop: Stop) -> Self {
        stop
    }
}

impl ChargeError {
    /// The stop reporting this refusal: the requested stop, or
    /// [`Stop::WorkLimit`] for an exhausted work limit.
    #[must_use]
    pub const fn into_stop(self) -> Stop {
        match self {
            Self::Stopped(stop) => stop,
            Self::WorkLimit => Stop::WorkLimit,
        }
    }
}

/// The error a settled operation reports: a requested stop when `error` refuses
/// a resource and the flags hold one, otherwise `error` itself.
#[cold]
#[inline(never)]
fn settled<E: Refusal>(poll: CancellationPoll<'_>, error: E) -> E {
    if error.is_resource_refusal()
        && let Err(stop) = poll.poll()
    {
        return E::from_stop(stop);
    }
    error
}

/// One operation's performed work and admitted allowance.
///
/// Local charges under [`Cancellation::metered`] retain the existing 256-step
/// polling cadence. [`Self::local`] leaves control to its caller, while
/// [`Self::reserved`] consults a borrowed account only when permits run out.
/// The same count and reservation body serve all three arrangements. Unspent
/// permits are `admitted - charged`, never a second independent account.
///
/// The count retains unread padding on both sides: a paired load cannot combine
/// it with another live value, preserving forwarding of its previous store on
/// aarch64. The meter owns no heap storage. A borrowed account adds stack
/// metadata; it does not change the caller's charged storage or work receipt.
///
/// Refund explicitly at the existing completed-attempt boundary.
/// Drop does not refund: a panicking bounded
/// operation has supplied no actual-work receipt, so its grant remains consumed
/// conservatively by the caller's account.
#[derive(Debug)]
pub struct Meter<'a, Q: Quota + ?Sized = LocalQuota> {
    count: Isolated,
    /// Local ceiling, or the total granted and not refunded.
    limit: u64,
    poll: Option<CancellationPoll<'a>>,
    quota: Option<&'a mut Q>,
}

/// A count whose neighbouring words are its own padding.
#[derive(Debug)]
#[repr(C)]
struct Isolated {
    _before: u64,
    value: u64,
    _after: u64,
}

impl Meter<'static> {
    /// A fixed local allowance with control observed by the enclosing operation.
    /// No poll or allocation occurs at construction or at a local refusal.
    #[must_use]
    pub const fn local(limit: u64) -> Self {
        Self {
            count: Isolated {
                _before: 0,
                value: 0,
                _after: 0,
            },
            limit,
            poll: None,
            quota: None,
        }
    }
}

impl<'a> Meter<'a> {
    const fn new(poll: CancellationPoll<'a>, limit: u64) -> Self {
        Self {
            count: Isolated {
                _before: 0,
                value: 0,
                _after: 0,
            },
            limit,
            poll: Some(poll),
            quota: None,
        }
    }

    /// Charge one operation with the existing local meter error shape.
    /// Poll before operation zero and each 256-step boundary, and again before
    /// refusing the local ceiling. A refused charge performs no operation.
    ///
    /// # Errors
    /// Returns the requested stop, otherwise the exhausted work limit.
    #[inline]
    pub fn charge(&mut self) -> Result<(), ChargeError> {
        self.tick().map_err(|failure| match failure.into_stop() {
            Stop::WorkLimit => ChargeError::WorkLimit,
            stop => ChargeError::Stopped(stop),
        })
    }

    /// The local operation's original work limit.
    #[must_use]
    #[inline]
    pub const fn limit(&self) -> u64 {
        self.limit
    }

    /// Count another arrangement's actual work under [`Self::remaining`].
    /// Its own polling cadence applies within that operation. This meter's
    /// cadence resumes from the completed count.
    ///
    /// # Errors
    /// An overrun counts up to the local limit, then polls control before
    /// returning [`ChargeError::WorkLimit`].
    pub fn absorb(&mut self, nested: u64) -> Result<(), ChargeError> {
        if nested > self.remaining() {
            self.count.value = self.limit;
            self.poll().map_err(ChargeError::Stopped)?;
            return Err(ChargeError::WorkLimit);
        }
        self.record(nested);
        Ok(())
    }
}

impl<'a, Q: Quota + ?Sized> Meter<'a, Q> {
    /// Borrow a caller's account without reserving, polling or allocating.
    /// Per-read refills request [`POLL_INTERVAL`] permits. The account owns
    /// control polling at those refills; the meter adds no per-read poll.
    #[must_use]
    pub fn reserved(quota: &'a mut Q) -> Self {
        Self {
            count: Isolated {
                _before: 0,
                value: 0,
                _after: 0,
            },
            limit: 0,
            poll: None,
            quota: Some(quota),
        }
    }

    /// Operations actually admitted as performed, on every outcome.
    #[must_use]
    #[inline]
    pub const fn charged(&self) -> u64 {
        self.count.value
    }

    /// Currently admitted permits whose operations have not executed.
    #[must_use]
    #[inline]
    pub const fn remaining(&self) -> u64 {
        self.limit - self.count.value
    }

    /// Charge one read before it runs, refilling only when no permits remain.
    ///
    /// # Errors
    /// Returns the exact caller refusal or a local stop before the next read.
    #[inline]
    pub fn tick(&mut self) -> Result<(), Failure<Q::Error>> {
        if let Some(poll) = &self.poll
            && poll_due(self.count.value, 1)
        {
            poll.poll()?;
        }
        if self.count.value == self.limit {
            self.admit()?;
        }
        self.record(1);
        Ok(())
    }

    /// Admit a local optional visit before charging the global account.
    /// An exhausted optional allowance returns false without a global charge;
    /// a refused global charge leaves the optional allowance unchanged.
    ///
    /// # Errors
    /// Returns the exact failure from [`Self::tick`].
    #[inline]
    pub fn tick_within(&mut self, remaining: &mut u64) -> Result<bool, Failure<Q::Error>> {
        if *remaining == 0 {
            return Ok(false);
        }
        self.tick()?;
        *remaining -= 1;
        Ok(true)
    }

    /// Reserve a complete bounded operation before running it, then count only
    /// its reported actual work. The returned value can itself contain a typed
    /// failure; that does not discard its performed prefix. Control follows the
    /// account's refill policy; explicit per-operation polls remain the caller's.
    ///
    /// # Errors
    /// A refused top-up or unrepresentable count runs no part of `operation`.
    /// Earlier partial grants remain refundable.
    ///
    /// # Panics
    /// The caller must report `actual <= maximum`. Debug builds assert this
    /// precondition, as the original private reservation boundary did.
    pub fn bounded<T>(
        &mut self,
        maximum: u64,
        operation: impl FnOnce() -> (T, u64),
    ) -> Result<T, Failure<Q::Error>> {
        self.top_up(maximum)?;
        let (value, actual) = operation();
        debug_assert!(
            actual <= maximum,
            "the operation exceeds its reserved bound"
        );
        self.record(actual);
        Ok(value)
    }

    /// Return only known unused permits, preserving the performed receipt.
    /// Call after normal completion or refusal, not during unwinding when a
    /// bounded operation has not reported its actual work. Repeated calls are
    /// harmless and retain the account's existing zero-refund behavior.
    pub fn refund(&mut self) {
        if let Some(quota) = &mut self.quota {
            let unspent = self.limit - self.count.value;
            self.limit = self.count.value;
            quota.refund(unspent);
        }
    }

    /// Observe this local meter's control view, if it has one.
    /// A reserved meter's account owns its control observations; this method
    /// does not obtain permits or add an account poll.
    ///
    /// # Errors
    /// Returns cancellation first, otherwise an expired deadline.
    #[inline]
    pub fn poll(&self) -> Result<(), Stop> {
        self.poll.as_ref().map_or(Ok(()), CancellationPoll::poll)
    }

    #[inline]
    fn record(&mut self, actual: u64) {
        self.count.value += actual;
    }

    #[cold]
    fn admit(&mut self) -> Result<(), Failure<Q::Error>> {
        let Some(quota) = &mut self.quota else {
            self.poll()?;
            return Err(Stop::WorkLimit.into());
        };
        let granted = quota.reserve(POLL_INTERVAL).map_err(Failure::Refused)?;
        debug_assert!(
            granted <= POLL_INTERVAL,
            "a quota grants at most what is asked"
        );
        if granted == 0 {
            return Err(Stop::WorkLimit.into());
        }
        // The per-read boundary preserves the existing account contract:
        // its cumulative granted count must remain representable. Bounded
        // top-ups below additionally limit each request by representability.
        self.limit += granted;
        Ok(())
    }

    fn top_up(&mut self, minimum: u64) -> Result<(), Failure<Q::Error>> {
        self.count
            .value
            .checked_add(minimum)
            .ok_or(Stop::WorkLimit)?;
        while self.remaining() < minimum {
            let Some(quota) = &mut self.quota else {
                return Err(Stop::WorkLimit.into());
            };
            let wanted = POLL_INTERVAL.min(u64::MAX - self.limit);
            let granted = quota.reserve(wanted).map_err(Failure::Refused)?;
            if granted == 0 {
                return Err(Stop::WorkLimit.into());
            }
            if granted > wanted {
                quota.refund(granted);
                return Err(Stop::InvalidProgram.into());
            }
            self.limit += granted;
        }
        Ok(())
    }
}

/// A metered operation's settled outcome and the work it charged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub struct Metered<T, E> {
    /// The operation's outcome, with a resource refusal replaced by a stop
    /// requested before the operation returned.
    pub result: Result<T, E>,
    /// Operations the meter admitted, on every outcome.
    pub charged: u64,
}

/// Shared cancellation and an optional immutable deadline. Clones observe the
/// same flags. A deadline is observed the way cancellation is: a timer thread
/// sets an expiry flag when the deadline passes, and a poll reads flags only,
/// never the clock. The timer retires at the deadline or when the last clone
/// drops, whichever comes first.
#[derive(Clone, Debug, Default)]
pub struct Cancellation {
    cancelled: Arc<AtomicBool>,
    deadline: Option<Arc<DeadlineOwner>>,
    slot: Option<slot::Membership>,
}

impl Cancellation {
    /// A fresh cancellation handle with the specified absolute deadline. A deadline that
    /// has already passed is expired at once and starts no thread.
    ///
    /// # Errors
    /// Returns the operating system's refusal to start the timer thread.
    pub fn with_deadline(deadline: Instant) -> io::Result<Self> {
        Ok(Self {
            deadline: Some(Arc::new(DeadlineOwner::arm(deadline)?)),
            ..Self::default()
        })
    }

    /// Request cancellation through this handle and every clone of it.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Observe cancellation and the deadline at a bounded work boundary.
    /// Ordinary handles read at most two relaxed flags; a handle from a
    /// [`CancellationSlot`] additionally reads its run's membership. No clock
    /// is read. A retired or superseded run reports cancellation.
    ///
    /// # Errors
    /// Returns cancellation first, otherwise an expired deadline.
    #[inline]
    pub fn poll(&self) -> Result<(), Stop> {
        self.polling().poll()
    }

    /// Run one metered operation under a work `limit` and settle its outcome
    /// where it returns: when the operation refuses a resource
    /// ([`Refusal::is_resource_refusal`]), control is observed and a requested
    /// stop is reported instead. Results and other errors pass through unread.
    /// The operation receives the only meter it can charge, so every charge is
    /// settled at this one exit.
    pub fn metered<'a, T, E: Refusal>(
        &'a self,
        limit: u64,
        operation: impl FnOnce(&mut Meter<'a>) -> Result<T, E>,
    ) -> Metered<T, E> {
        let poll = self.polling();
        let mut meter = Meter::new(poll, limit);
        let outcome = operation(&mut meter);
        Metered {
            // Only an error leaves the inlined path, so a hot operation's
            // success costs one discriminant test.
            result: outcome.map_err(|error| settled(poll, error)),
            charged: meter.charged(),
        }
    }

    /// Borrow the immutable handle shape once for repeated boundary polls.
    /// Preparing this view takes constant work and space, without allocation,
    /// reference-count updates or flag reads. It borrows the underlying flags,
    /// so cancellation, deadline expiry and slot retirement remain live.
    /// The borrow keeps the token's storage alive; it does not keep a slot's
    /// run active or extend its deadline.
    #[must_use]
    #[inline]
    pub fn polling(&self) -> CancellationPoll<'_> {
        CancellationPoll {
            cancelled: &self.cancelled,
            expired: DeadlineOwner::expiry(self.deadline.as_ref()),
            membership: slot::Membership::polling(self.slot.as_ref()),
        }
    }
}

/// A borrowed view of the flags read by [`Cancellation::poll`].
///
/// Tight loops can retain this view to avoid traversing the owning handle at
/// each boundary. It caches references and the immutable run identity, never
/// an observed flag value. Copying it neither owns a token nor prolongs a run.
#[derive(Clone, Copy, Debug)]
pub struct CancellationPoll<'a> {
    cancelled: &'a AtomicBool,
    expired: Option<&'a AtomicBool>,
    membership: Option<slot::MembershipPoll<'a>>,
}

impl CancellationPoll<'_> {
    /// Observe exactly the same flags, in the same order, as the owning token.
    /// Reads only relaxed atomics; no clock, allocation or lock is involved.
    ///
    /// # Errors
    /// Returns cancellation first, otherwise an expired deadline.
    #[inline]
    pub fn poll(&self) -> Result<(), Stop> {
        if self.cancelled.load(Ordering::Relaxed)
            || slot::MembershipPoll::is_cancelled(self.membership)
        {
            Err(Stop::Cancelled)
        } else if self
            .expired
            .is_some_and(|expired| expired.load(Ordering::Relaxed))
        {
            Err(Stop::Deadline)
        } else {
            Ok(())
        }
    }
}

/// The flags a timer sets and the wake-up it waits on.
#[derive(Debug)]
struct Deadline {
    at: Instant,
    expired: AtomicBool,
    /// True once every cancellation handle has dropped; the timer then exits early.
    retired: Mutex<bool>,
    wake: Condvar,
}

/// Held by the cancellation handles, not by the timer, so that dropping the last
/// clone is observable as the owner's drop.
#[derive(Debug)]
struct DeadlineOwner {
    deadline: Arc<Deadline>,
}

impl DeadlineOwner {
    /// Borrow an optional timer flag without observing its current value.
    #[inline]
    fn expiry(deadline: Option<&Arc<Self>>) -> Option<&AtomicBool> {
        let owner = deadline?;
        Some(&owner.deadline.expired)
    }

    fn arm(at: Instant) -> io::Result<Self> {
        let deadline = Arc::new(Deadline {
            at,
            expired: AtomicBool::new(false),
            retired: Mutex::new(false),
            wake: Condvar::new(),
        });
        if Instant::now() >= at {
            deadline.expired.store(true, Ordering::Relaxed);
        } else {
            let timer = Arc::clone(&deadline);
            thread::Builder::new()
                .name("zetesis-deadline".into())
                .spawn(move || timer.wait())?;
        }
        Ok(Self { deadline })
    }
}

impl Drop for DeadlineOwner {
    fn drop(&mut self) {
        *self.deadline.lock() = true;
        self.deadline.wake.notify_all();
    }
}

impl Deadline {
    fn lock(&self) -> std::sync::MutexGuard<'_, bool> {
        self.retired.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sleep until the deadline or retirement. Timed waits bound the sleep to
    /// the remaining time, so a spurious wake-up costs one clock read.
    fn wait(&self) {
        let mut retired = self.lock();
        loop {
            if *retired {
                return;
            }
            let now = Instant::now();
            if now >= self.at {
                self.expired.store(true, Ordering::Relaxed);
                return;
            }
            retired = self
                .wake
                .wait_timeout(retired, self.at - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// An incomplete operation or invalid invocation. None denotes a rejected
/// candidate, exhaustive search, or logical UNSAT.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Shared cancellation was observed.
    Cancelled,
    /// The absolute deadline expired.
    Deadline,
    /// The per-oracle charged work ceiling was reached.
    WorkLimit,
    /// Another complete source round would exceed the round ceiling.
    RoundLimit,
    /// Named live closure storage would exceed its byte ceiling.
    StorageLimit,
    /// Adding another distinct consequence would exceed the atom ceiling.
    DerivedAtomLimit,
    /// Producing another candidate would exceed the candidate ceiling.
    CandidateLimit,
    /// Discovering another carrier atom would exceed its ceiling.
    CarrierLimit,
    /// A fallible vector or carrier allocation was refused.
    Allocation,
    /// Candidate and program have different admitted instance identities.
    WrongProgram,
    /// An admitted shape failed an internal instantiation invariant.
    InvalidProgram,
}

impl fmt::Display for Stop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "operation cancelled",
            Self::Deadline => "operation deadline expired",
            Self::WorkLimit => "oracle work limit reached",
            Self::RoundLimit => "source round limit reached",
            Self::StorageLimit => "closure storage limit reached",
            Self::DerivedAtomLimit => "derived atom limit reached",
            Self::CandidateLimit => "candidate limit reached",
            Self::CarrierLimit => "candidate carrier atom limit reached",
            Self::Allocation => "operation storage could not be reserved",
            Self::WrongProgram => "candidate belongs to a different admitted program",
            Self::InvalidProgram => "admitted program violated an instantiation invariant",
        })
    }
}

impl std::error::Error for Stop {}

#[cfg(test)]
mod tests {
    mod quota;

    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn charge_ranges_observe_exactly_the_contained_boundaries() {
        for charged in 0..1024 {
            for amount in 0..=1024 {
                let boundary = u128::from(charged).div_ceil(u128::from(POLL_INTERVAL))
                    * u128::from(POLL_INTERVAL);
                assert_eq!(
                    poll_due(charged, amount),
                    boundary < u128::from(charged) + u128::from(amount),
                    "charged={charged}, amount={amount}"
                );
            }
        }
    }

    #[test]
    fn boundary_decisions_do_not_overflow_at_the_counter_limit() {
        for charged in [0, 1, u64::MAX - 256, u64::MAX - 1, u64::MAX] {
            for amount in [0, 1, 255, 256, 257, u64::MAX] {
                let boundary = u128::from(charged).div_ceil(u128::from(POLL_INTERVAL))
                    * u128::from(POLL_INTERVAL);
                assert_eq!(
                    poll_due(charged, amount),
                    boundary < u128::from(charged) + u128::from(amount),
                    "charged={charged}, amount={amount}"
                );
            }
        }
    }

    fn wait_for_expiry(cancellation: &Cancellation) {
        let deadline = &cancellation.deadline.as_ref().unwrap().deadline;
        let timeout = Instant::now() + Duration::from_secs(5);
        // Observe the flag directly: a boundary poll must not cause the expiry.
        // The bound detects a stalled timer, not a promised scheduling latency.
        while !deadline.expired.load(Ordering::Relaxed) {
            assert!(Instant::now() < timeout, "deadline timer did not expire");
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Charge once through a meter that has admitted `charged` operations under
    /// `limit`; returns the result and the count after it.
    fn charge_from(
        cancellation: &Cancellation,
        charged: u64,
        limit: u64,
    ) -> (Result<(), ChargeError>, u64) {
        let mut meter = Meter {
            count: Isolated {
                _before: 0,
                value: charged,
                _after: 0,
            },
            limit,
            poll: Some(cancellation.polling()),
            quota: None,
        };
        let result = meter.charge();
        (result, meter.charged())
    }

    #[test]
    fn charges_observe_a_stop_only_at_metered_boundaries() {
        let cancellation = Cancellation::default();
        cancellation.cancel();
        for charged in 0..=3 * POLL_INTERVAL {
            let expected = if charged.is_multiple_of(POLL_INTERVAL) {
                (Err(ChargeError::Stopped(Stop::Cancelled)), charged)
            } else {
                (Ok(()), charged + 1)
            };
            assert_eq!(
                charge_from(&cancellation, charged, u64::MAX),
                expected,
                "{charged}"
            );
        }
    }

    #[test]
    fn a_requested_stop_precedes_the_work_limit_between_boundaries() {
        let cancellation = Cancellation::default();
        cancellation.cancel();
        // Seven is not a boundary, so only the precedence observation sees the stop.
        assert_eq!(
            charge_from(&cancellation, 7, 7),
            (Err(ChargeError::Stopped(Stop::Cancelled)), 7)
        );
    }

    #[test]
    fn the_work_limit_refuses_without_counting() {
        let cancellation = Cancellation::default();
        assert_eq!(
            charge_from(&cancellation, 7, 7),
            (Err(ChargeError::WorkLimit), 7)
        );
        assert_eq!(
            charge_from(&cancellation, 0, 0),
            (Err(ChargeError::WorkLimit), 0)
        );
    }

    #[test]
    fn an_admitted_charge_counts_its_operation() {
        let cancellation = Cancellation::default();
        assert_eq!(charge_from(&cancellation, 6, 7), (Ok(()), 7));
    }

    #[test]
    fn nested_work_counts_within_the_remaining_allowance() {
        let cancellation = Cancellation::default();
        let metered = cancellation.metered(10, |meter| {
            for _ in 0..3 {
                meter.charge().unwrap();
            }
            assert_eq!(meter.remaining(), 7);
            meter.absorb(7).map_err(ChargeError::into_stop)
        });
        assert_eq!(metered.result, Ok(()));
        assert_eq!(metered.charged, 10);
    }

    #[test]
    fn an_overrun_absorb_counts_up_to_the_limit() {
        // The refusal is returned as a value, so the exit leaves it alone.
        let cancellation = Cancellation::default();
        let metered = cancellation.metered(10, |meter| {
            for _ in 0..3 {
                meter.charge().unwrap();
            }
            Ok::<_, Stop>((meter.absorb(8), meter.remaining()))
        });
        assert_eq!(metered.result, Ok((Err(ChargeError::WorkLimit), 0)));
        assert_eq!(metered.charged, 10);
    }

    #[test]
    fn an_overrun_absorb_reports_a_requested_stop() {
        // The refusal is returned as a value, so the exit leaves it alone.
        let cancellation = Cancellation::default();
        let metered = cancellation.metered(10, |meter| {
            cancellation.cancel();
            Ok::<_, Stop>(meter.absorb(11))
        });
        assert_eq!(
            metered.result,
            Ok(Err(ChargeError::Stopped(Stop::Cancelled)))
        );
        assert_eq!(metered.charged, 10);
    }

    /// The settled outcome of an operation that returns `outcome`, with a stop
    /// requested inside it when `stop` holds.
    fn metered_outcome(stop: bool, outcome: Result<u8, Stop>) -> Result<u8, Stop> {
        let cancellation = Cancellation::default();
        cancellation
            .metered(u64::MAX, |_| {
                if stop {
                    cancellation.cancel();
                }
                outcome
            })
            .result
    }

    #[test]
    fn a_requested_stop_settles_a_resource_refusal() {
        assert_eq!(
            metered_outcome(false, Err(Stop::StorageLimit)),
            Err(Stop::StorageLimit)
        );
        assert_eq!(
            metered_outcome(true, Err(Stop::StorageLimit)),
            Err(Stop::Cancelled)
        );
    }

    #[test]
    fn settling_leaves_results_and_shape_errors_alone() {
        assert_eq!(metered_outcome(true, Ok(3)), Ok(3));
        for shape in [Stop::WrongProgram, Stop::InvalidProgram] {
            assert_eq!(metered_outcome(true, Err(shape)), Err(shape));
        }
    }

    #[test]
    fn a_metered_operation_reports_its_work_on_every_outcome() {
        let cancellation = Cancellation::default();
        let metered = cancellation.metered(5, |meter| {
            while meter.charge().is_ok() {}
            Err::<(), _>(Stop::WorkLimit)
        });
        assert_eq!(metered.result, Err(Stop::WorkLimit));
        assert_eq!(metered.charged, 5);
    }

    #[test]
    fn the_count_has_padding_on_both_sides() {
        assert_eq!(std::mem::offset_of!(Isolated, value), size_of::<u64>());
        assert_eq!(size_of::<Isolated>(), 3 * size_of::<u64>());
    }

    #[test]
    fn stops_classify_resource_refusals() {
        for stop in [
            Stop::WorkLimit,
            Stop::RoundLimit,
            Stop::StorageLimit,
            Stop::DerivedAtomLimit,
            Stop::CandidateLimit,
            Stop::CarrierLimit,
            Stop::Allocation,
        ] {
            assert!(stop.is_resource_refusal(), "{stop:?}");
        }
        for stop in [
            Stop::Cancelled,
            Stop::Deadline,
            Stop::WrongProgram,
            Stop::InvalidProgram,
        ] {
            assert!(!stop.is_resource_refusal(), "{stop:?}");
        }
    }

    #[test]
    fn a_deadline_expires_without_polling() {
        let cancellation =
            Cancellation::with_deadline(Instant::now() + Duration::from_millis(30)).unwrap();
        let polling = cancellation.polling();
        wait_for_expiry(&cancellation);
        assert_eq!(polling.poll(), Err(Stop::Deadline));
        assert_eq!(cancellation.poll(), Err(Stop::Deadline));
    }

    #[test]
    fn clones_share_the_deadline() {
        let cancellation =
            Cancellation::with_deadline(Instant::now() + Duration::from_millis(30)).unwrap();
        let clone = cancellation.clone();
        assert!(Arc::ptr_eq(
            cancellation.deadline.as_ref().unwrap(),
            clone.deadline.as_ref().unwrap(),
        ));
        wait_for_expiry(&cancellation);
        assert_eq!(clone.poll(), Err(Stop::Deadline));
        assert_eq!(cancellation.poll(), Err(Stop::Deadline));
    }

    #[test]
    fn dropping_the_last_clone_retires_the_timer() {
        // A second waiter on the same deadline stands in for the timer thread,
        // whose handle is not retained: retirement wakes every waiter.
        let owner = DeadlineOwner::arm(Instant::now() + Duration::from_hours(1)).unwrap();
        let deadline = Arc::clone(&owner.deadline);
        let (retired, observer) = mpsc::channel();
        thread::spawn(move || {
            deadline.wait();
            retired.send(()).unwrap();
        });
        assert!(observer.recv_timeout(Duration::from_millis(100)).is_err());
        drop(owner);
        assert!(observer.recv_timeout(Duration::from_secs(5)).is_ok());
    }

    #[test]
    fn retirement_does_not_expire_the_deadline() {
        let owner = DeadlineOwner::arm(Instant::now() + Duration::from_hours(1)).unwrap();
        let deadline = Arc::clone(&owner.deadline);
        drop(owner);
        deadline.wait();
        assert!(!deadline.expired.load(Ordering::Relaxed));
    }
}
