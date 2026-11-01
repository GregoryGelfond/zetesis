//! Shared cancellation, deadlines, and typed incomplete outcomes.

use std::fmt;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;
use std::time::Instant;

mod bytes;
mod permits;
mod slot;

pub use bytes::{MeterBytes, MeterBytesError};
use permits::{Counted, State as _};
pub use permits::{PermitLease, PermitSettlement};
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

/// A caller-owned accepted-work receipt could not admit a complete request.
/// Neither failure changes the receipt or observes cancellation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeterWorkError {
    /// The representable complete demand exceeds the inclusive work ceiling.
    Limit {
        /// Prior accepted work plus the complete requested increment.
        observed: u128,
        /// The caller's current, unchanged ceiling.
        limit: u128,
    },
    /// Prior accepted work plus the requested increment does not fit u128.
    /// The operation neither wraps nor substitutes a saturated demand.
    Overflow,
}

/// A whole request cannot be admitted into a machine-sized named count.
/// The caller retains its resource identity, sole counter and commit boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeterCountError {
    /// The representable complete demand exceeds the inclusive count ceiling.
    Limit {
        /// The existing count plus the complete requested increment.
        observed: usize,
        /// The caller's current, unchanged ceiling.
        limit: usize,
    },
    /// The existing count plus the increment does not fit usize.
    /// This precedes the ceiling check, including at a maximum ceiling.
    Overflow,
}

/// A whole request cannot be admitted into a wide named count.
/// The caller retains the resource identity, counter and commit boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeterWideCountError {
    /// The representable complete demand exceeds its inclusive count ceiling.
    Limit {
        /// The existing count plus the complete requested increment.
        observed: u128,
        /// The caller's current, unchanged ceiling.
        limit: u128,
    },
    /// The existing count plus the increment does not fit u128.
    /// This precedes the ceiling check, including at a maximum ceiling.
    Overflow,
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

/// An already-admitted whole-kernel allowance and its settlement authority.
/// Obtaining the guard remains the originating account's operation. Dropping it
/// without committing preserves that account's policy for unreceipted work.
/// This is ownership of a reservation, not a second work counter.
pub trait Reservation {
    /// The fixed allowance for this attempt; zero is representable.
    fn allowance(&self) -> u64;

    /// Settle a returned receipt, consuming the guard without polling or running
    /// the kernel. The caller must establish `used <= self.allowance()` first;
    /// [`Meter::under_reservation`] supplies that check. The guard's existing
    /// settlement policy determines how unused permits return to its account.
    fn commit(self, used: u64);
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
    /// Admit one whole increment into a caller-owned accepted-work receipt.
    ///
    /// The receipt remains the sole stored work counter. This operation checks
    /// the current inclusive ceiling even when `count` is zero. A receipt
    /// imported under a lower new ceiling is permitted as input and is refused
    /// without modification at this charge boundary, not at construction.
    ///
    /// No Meter is constructed. There is no polling, allocation, callback,
    /// reservation, refund or retained state. The caller owns control polling,
    /// source identity and construction of its typed failure.
    ///
    /// # Errors
    /// Returns `Overflow` if the complete demand cannot fit u128; otherwise
    /// returns `Limit` with the exact demand and limit if admission fails.
    /// Both failures leave `accepted` unchanged. Success records the whole
    /// demand, which fits u64 because it does not exceed the supplied ceiling.
    ///
    /// # Panics
    /// The final conversion asserts the preceding u64-ceiling check. No input
    /// can violate that invariant while this check is preserved.
    pub fn charge_receipt(
        accepted: &mut u64,
        count: u128,
        limit: u64,
    ) -> Result<(), MeterWorkError> {
        *accepted = Self::check_receipt(*accepted, count, limit)?;
        Ok(())
    }

    /// Check a whole work request without changing the caller's receipt.
    ///
    /// Success returns the proposed complete receipt, not work already performed
    /// or a reservation. The caller retains its sole recorded count and decides
    /// when to publish work. A later partial commit is justified only while the
    /// relevant recorded prefix and ceiling are unchanged and the actual work
    /// does not exceed this checked request.
    ///
    /// This pure check has no polling, allocation, callback, refund or retained
    /// state. Zero still checks the current ceiling, including when the recorded
    /// prefix was imported under a larger earlier ceiling.
    ///
    /// # Errors
    /// Returns `Overflow` if the complete demand cannot fit u128; otherwise
    /// returns `Limit` with the exact demand and ceiling when it does not fit.
    ///
    /// # Panics
    /// The final conversion asserts the preceding u64-ceiling check. No input
    /// can violate that invariant while this check is preserved.
    pub fn check_receipt(recorded: u64, count: u128, limit: u64) -> Result<u64, MeterWorkError> {
        let observed = Self::check_wide_receipt(u128::from(recorded), count, u128::from(limit))?;
        Ok(u64::try_from(observed).expect("work checked against u64 ceiling"))
    }

    /// Check a whole request against an existing wide accepted-work receipt.
    ///
    /// Like [`Self::check_receipt`], this pure operation returns only a proposed
    /// total. It neither changes the caller's u128 counter nor establishes that
    /// work was performed or reserved. Zero checks the supplied ceiling; an
    /// imported prefix above a new ceiling is valid input and is refused.
    /// There is no polling, allocation, callback, quota, refund or retained state.
    ///
    /// # Errors
    /// Returns `Overflow` if the complete demand does not fit u128, even when
    /// the ceiling is `u128::MAX`. Otherwise an excess returns `Limit` with the
    /// exact demand and ceiling. Neither operation saturates or commits a count.
    pub fn check_wide_receipt(
        recorded: u128,
        count: u128,
        limit: u128,
    ) -> Result<u128, MeterWorkError> {
        check_sum_as(
            recorded,
            count,
            limit,
            u128::checked_add,
            MeterWorkError::Overflow,
            |observed, limit| MeterWorkError::Limit { observed, limit },
        )
    }

    /// Check a whole increment of a caller-owned, machine-sized named count.
    ///
    /// Returns the proposed complete count without changing any owner or
    /// establishing that the requested event occurred. The caller retains the
    /// resource name, source attribution and publication/commit boundary.
    /// No Meter is constructed. There is no polling, allocation, callback,
    /// reservation or refund. A zero increment still checks the current ceiling,
    /// including when a prefix was imported under an earlier larger ceiling.
    ///
    /// # Errors
    /// Returns [`MeterCountError::Overflow`] when the sum does not fit usize,
    /// before considering the ceiling. Otherwise an excess returns the exact
    /// proposed count and ceiling in [`MeterCountError::Limit`].
    pub fn check_count(
        recorded: usize,
        amount: usize,
        limit: usize,
    ) -> Result<usize, MeterCountError> {
        check_sum_as(
            recorded,
            amount,
            limit,
            usize::checked_add,
            MeterCountError::Overflow,
            |observed, limit| MeterCountError::Limit { observed, limit },
        )
    }

    /// Check a complete increment of a caller-owned wide named count.
    ///
    /// This is the u128 counterpart of [`Self::check_count`]. Counts may name
    /// objects, occurrences or cumulative byte charges; this does not observe
    /// actual storage or a peak. The caller retains those meanings and the
    /// original publication point. The result is only a proposed total.
    /// No counter is changed, and no event is established as performed.
    /// There is no polling, allocation, callback, reservation or refund.
    /// Zero still checks the supplied ceiling, including an imported prefix
    /// admitted under an earlier larger ceiling.
    ///
    /// # Errors
    /// Returns [`MeterWideCountError::Overflow`] when the sum does not fit u128,
    /// before checking the ceiling. Otherwise an excess returns the exact
    /// proposed count and ceiling in [`MeterWideCountError::Limit`]. Neither
    /// operation wraps, saturates or commits a count.
    pub fn check_wide_count(
        recorded: u128,
        amount: u128,
        limit: u128,
    ) -> Result<u128, MeterWideCountError> {
        check_sum_as(
            recorded,
            amount,
            limit,
            u128::checked_add,
            MeterWideCountError::Overflow,
            |observed, limit| MeterWideCountError::Limit { observed, limit },
        )
    }

    /// Execute once under an admitted whole-kernel reservation and commit its
    /// valid work receipt. The callback receives the fixed allowance, including
    /// zero; the caller retains its admission and polling policy. This operation
    /// adds constant work without allocating, polling or creating another meter.
    ///
    /// `T` may carry a native failed attempt with performed work. Such a receipt
    /// is committed just like success, before returning the value and used work.
    /// An outer `Err(E)` supplies no receipt. On that error, an invalid receipt,
    /// or unwinding, the guard is dropped without commit under its account's
    /// existing abandonment policy.
    ///
    /// # Errors
    /// Returns the callback's original error as [`Failure::Refused`], without
    /// observing control again. Returns [`Stop::InvalidProgram`] when reported
    /// work exceeds the admitted allowance; that receipt is not committed.
    pub fn under_reservation<R, T, E>(
        reservation: R,
        run: impl FnOnce(u64) -> Result<(T, u64), E>,
    ) -> Result<(T, u64), Failure<E>>
    where
        R: Reservation,
        E: Refusal,
    {
        let granted = reservation.allowance();
        let (value, used) = run(granted).map_err(Failure::Refused)?;
        if used > granted {
            return Err(Failure::Stopped(Stop::InvalidProgram));
        }
        reservation.commit(used);
        Ok((value, used))
    }

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
        let taken = permits::take_available(
            &mut Counted {
                charged: &mut self.count.value,
                admitted: &mut self.limit,
            },
            nested,
        );
        if taken < nested {
            self.poll().map_err(ChargeError::Stopped)?;
            return Err(ChargeError::WorkLimit);
        }
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
        let Self {
            count,
            limit,
            poll,
            quota,
        } = self;
        let mut state = Counted {
            charged: &mut count.value,
            admitted: limit,
        };
        permits::tick(&mut state, |state| Self::admit(state, quota, poll.as_ref()))
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
            permits::settle(
                &mut Counted {
                    charged: &mut self.count.value,
                    admitted: &mut self.limit,
                },
                |prior| quota.refund(prior.unused()),
            );
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
        Counted {
            charged: &mut self.count.value,
            admitted: &mut self.limit,
        }
        .consume(actual);
    }

    #[cold]
    fn admit(
        state: &mut Counted<'_>,
        quota: &mut Option<&mut Q>,
        poll: Option<&CancellationPoll<'_>>,
    ) -> Result<(), Failure<Q::Error>> {
        let Some(quota) = quota else {
            poll.map_or(Ok(()), CancellationPoll::poll)?;
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
        *state.admitted += granted;
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

// Only primitive checked addition and native error construction vary between
// dimensions. Both use their actual width; this private operation owns no state.
fn check_sum_as<T: Copy + Ord, E>(
    recorded: T,
    amount: T,
    limit: T,
    add: impl FnOnce(T, T) -> Option<T>,
    overflow: E,
    excess: impl FnOnce(T, T) -> E,
) -> Result<T, E> {
    let observed = add(recorded, amount).ok_or(overflow)?;
    if observed > limit {
        Err(excess(observed, limit))
    } else {
        Ok(observed)
    }
}

#[cfg(test)]
mod tests;
