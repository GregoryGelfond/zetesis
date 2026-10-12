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

/// Whether `charged` operations complete a run of [`POLL_INTERVAL`], so that
/// the next operation starts a new run and observes the flags first.
const fn at_poll_boundary(charged: u64) -> bool {
    charged.is_multiple_of(POLL_INTERVAL)
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

/// The metered charge shared by [`Cancellation::charge`] and
/// [`CancellationPoll::charge`]: observe control at a boundary of `charged`,
/// observe it again before refusing the limit, and count an admitted charge.
#[inline]
fn metered_charge(
    charged: &mut u64,
    limit: u64,
    poll: impl Fn() -> Result<(), Stop>,
) -> Result<(), ChargeError> {
    if at_poll_boundary(*charged) {
        poll().map_err(ChargeError::Stopped)?;
    }
    if *charged >= limit {
        // A requested stop takes precedence over the work limit.
        poll().map_err(ChargeError::Stopped)?;
        return Err(ChargeError::WorkLimit);
    }
    *charged += 1;
    Ok(())
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

    /// Charge one operation of a metered computation, as
    /// [`CancellationPoll::charge`] does for the borrowed view.
    ///
    /// # Errors
    /// As [`CancellationPoll::charge`].
    #[inline]
    pub fn charge(&self, charged: &mut u64, limit: u64) -> Result<(), ChargeError> {
        metered_charge(charged, limit, || self.poll())
    }

    /// Report a resource refusal of a metered computation, as
    /// [`CancellationPoll::refusal`] does for the borrowed view.
    #[must_use]
    #[inline]
    pub fn refusal<E>(&self, refusal: E, stopped: impl FnOnce(Stop) -> E) -> E {
        match self.poll() {
            Ok(()) => refusal,
            Err(stop) => stopped(stop),
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

    /// Charge one operation of a metered computation that has admitted
    /// `charged` operations under its work `limit`.
    ///
    /// The flags are observed first when `charged` completes a run of
    /// [`POLL_INTERVAL`], so a computation that charges each of its
    /// operations here observes them before its first operation and at least
    /// once every [`POLL_INTERVAL`] operations. At the limit they are observed
    /// again before refusing, so a requested stop takes precedence over the
    /// work limit. An admitted charge counts the operation; a refused charge
    /// leaves `charged` unchanged.
    ///
    /// A charge writes `charged` and the next one reads it again with `limit`.
    /// Keep the two apart in memory: if they are adjacent, a single paired
    /// load of both cannot be forwarded from the pending store on aarch64,
    /// which stalls every charge in a per-node loop.
    ///
    /// # Errors
    /// [`ChargeError::Stopped`] when an observation finds a requested stop or
    /// an expired deadline; otherwise [`ChargeError::WorkLimit`] when
    /// `charged` has reached `limit`.
    #[inline]
    pub fn charge(&self, charged: &mut u64, limit: u64) -> Result<(), ChargeError> {
        metered_charge(charged, limit, || self.poll())
    }

    /// Report a resource refusal of a metered computation: the requested stop
    /// when the flags hold one, otherwise `refusal`. Observing the flags first
    /// keeps a requested stop's precedence over every resource refusal,
    /// although the computation observes them only at metered boundaries.
    #[must_use]
    #[inline]
    pub fn refusal<E>(&self, refusal: E, stopped: impl FnOnce(Stop) -> E) -> E {
        match self.poll() {
            Ok(()) => refusal,
            Err(stop) => stopped(stop),
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

impl Stop {
    /// Canonical construction is an execution refusal, never model absence.
    /// Identity-space limits remain distinct from invalid admitted shapes.
    pub(crate) fn catalog(error: &zetesis_core::catalog::Error) -> Self {
        use zetesis_core::{ValueError, catalog::Error};
        match error {
            Error::Allocation
            | Error::Storage { .. }
            | Error::Overflow
            | Error::Value(ValueError::Allocation) => Self::Allocation,
            Error::IdExhausted | Error::Value(ValueError::Limit { .. }) => Self::CarrierLimit,
            Error::Shape
            | Error::FrozenVocabulary
            | Error::UnindexedVocabulary
            | Error::VocabularyHasAtoms
            | Error::CatalogHasBase
            | Error::Value(ValueError::Shape) => Self::InvalidProgram,
        }
    }

    pub(crate) fn model(error: &zetesis_core::ModelError) -> Self {
        match error {
            zetesis_core::ModelError::Allocation => Self::Allocation,
            zetesis_core::ModelError::Bytes { .. } => Self::StorageLimit,
            zetesis_core::ModelError::Catalog(error) => Self::catalog(error),
            zetesis_core::ModelError::Position { .. } | zetesis_core::ModelError::Order { .. } => {
                Self::InvalidProgram
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

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

    /// Charge once from `charged` through the owned token and through its
    /// borrowed view, which must agree; returns the result and the count.
    fn charge_both(
        cancellation: &Cancellation,
        charged: u64,
        limit: u64,
    ) -> (Result<(), ChargeError>, u64) {
        let (mut owned, mut borrowed) = (charged, charged);
        let result = cancellation.charge(&mut owned, limit);
        assert_eq!(cancellation.polling().charge(&mut borrowed, limit), result);
        assert_eq!(owned, borrowed);
        (result, owned)
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
                charge_both(&cancellation, charged, u64::MAX),
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
            charge_both(&cancellation, 7, 7),
            (Err(ChargeError::Stopped(Stop::Cancelled)), 7)
        );
    }

    #[test]
    fn the_work_limit_refuses_without_counting() {
        let cancellation = Cancellation::default();
        assert_eq!(
            charge_both(&cancellation, 7, 7),
            (Err(ChargeError::WorkLimit), 7)
        );
        assert_eq!(
            charge_both(&cancellation, 0, 0),
            (Err(ChargeError::WorkLimit), 0)
        );
    }

    #[test]
    fn an_admitted_charge_counts_its_operation() {
        let cancellation = Cancellation::default();
        assert_eq!(charge_both(&cancellation, 6, 7), (Ok(()), 7));
    }

    #[test]
    fn a_requested_stop_precedes_a_resource_refusal() {
        let cancellation = Cancellation::default();
        let refuse = |cancellation: &Cancellation| {
            let owned = cancellation.refusal(Stop::StorageLimit, |stop| stop);
            let borrowed = cancellation
                .polling()
                .refusal(Stop::StorageLimit, |stop| stop);
            assert_eq!(owned, borrowed);
            owned
        };
        assert_eq!(refuse(&cancellation), Stop::StorageLimit);
        cancellation.cancel();
        assert_eq!(refuse(&cancellation), Stop::Cancelled);
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
