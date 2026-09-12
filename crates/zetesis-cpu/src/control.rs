//! Shared cancellation, deadlines, and typed incomplete outcomes.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Shared cancellation and an optional immutable deadline. Clones observe the
/// same cancellation flag; no background thread or timer is created.
#[derive(Clone, Debug, Default)]
pub struct Control {
    cancelled: Arc<AtomicBool>,
    deadline: Option<Instant>,
}

impl Control {
    /// A fresh control with the specified absolute deadline.
    #[must_use]
    pub fn with_deadline(deadline: Instant) -> Self {
        Self {
            deadline: Some(deadline),
            ..Self::default()
        }
    }

    /// Cancel this control and every clone of it.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Observe cancellation and the deadline at a bounded work boundary.
    ///
    /// # Errors
    /// Returns cancellation first, otherwise an expired deadline.
    #[inline]
    pub fn poll(&self) -> Result<(), Stop> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err(Stop::Cancelled)
        } else if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            Err(Stop::Deadline)
        } else {
            Ok(())
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
