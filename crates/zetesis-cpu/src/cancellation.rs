//! Shared cancellation, deadlines, and typed incomplete outcomes.

use std::fmt;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;
use std::time::Instant;

/// Shared cancellation and an optional immutable deadline. Clones observe the
/// same flags. A deadline is observed the way cancellation is: a timer thread
/// sets an expiry flag when the deadline passes, and a poll reads flags only,
/// never the clock. The timer retires at the deadline or when the last clone
/// drops, whichever comes first.
#[derive(Clone, Debug, Default)]
pub struct Cancellation {
    cancelled: Arc<AtomicBool>,
    deadline: Option<Arc<DeadlineOwner>>,
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
    /// At most two relaxed loads; no clock is read.
    ///
    /// # Errors
    /// Returns cancellation first, otherwise an expired deadline.
    #[inline]
    pub fn poll(&self) -> Result<(), Stop> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err(Stop::Cancelled)
        } else if self
            .deadline
            .as_ref()
            .is_some_and(|owner| owner.deadline.expired.load(Ordering::Relaxed))
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
