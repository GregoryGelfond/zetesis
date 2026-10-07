//! A reusable cancellation handle with one generation per run.

use std::fmt;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use super::{Cancellation, DeadlineOwner};

// The upper bits identify a generation; the low bits are its explicit status.
// Generation zero starts inactive. No generation is reused, including after a
// failed deadline setup, so a delayed compare-exchange cannot encounter ABA.
const STATUS_MASK: u64 = 3;
const GENERATION_STEP: u64 = STATUS_MASK + 1;
const ACTIVE: u64 = 1;
const CANCELLED: u64 = 2;

/// A reusable handle that cancels only its current run.
///
/// Clones address the same slot. [`open`](Self::open) gives each run a fresh
/// generation and returns its lifetime guard; cloned [`Cancellation`] tokens
/// observe that run but do not keep its window active. Opening a new run
/// supersedes the previous one, whose tokens then observe cancellation even
/// if its guard was leaked. Dropping a guard retires only its own generation.
///
/// A pull observed while idle does nothing. A pull racing with a replacement
/// either cancels the generation it observed or does nothing; it never retries
/// against the replacement. This is one atomic word, not a pointer to mutable
/// run state: no lock, relay thread or allocation participates in a pull.
/// Slot clones share one allocation; opening also allocates the ordinary
/// cancellation flag, with a deadline's existing timer resources if requested.
#[derive(Clone, Debug, Default)]
pub struct CancellationSlot {
    state: Arc<AtomicU64>,
}

impl CancellationSlot {
    /// Open a fresh run, optionally bounded by an absolute deadline.
    ///
    /// The slot becomes active before its flag is allocated or the deadline
    /// timer is armed, so a pull during setup is retained. A failed timer setup
    /// retires this run. Superseded
    /// tokens report cancellation; no older guard can retire the new run.
    /// The guard must stay alive until completion or abandonment of the run.
    ///
    /// Opening uses constant space and retries its atomic transition only
    /// when another operation changed the slot; concurrent openings can
    /// therefore increase its work. Deadline setup has the same costs as
    /// [`Cancellation::with_deadline`].
    ///
    /// # Errors
    /// Returns [`CancellationSlotError::GenerationExhausted`] after all
    /// `2^62 - 1` generations have been used, without altering the current
    /// run, or [`CancellationSlotError::Deadline`] if its timer cannot start.
    pub fn open(
        &self,
        deadline: Option<Instant>,
    ) -> Result<CancellationRun, CancellationSlotError> {
        self.open_with(deadline, DeadlineOwner::arm)
    }

    fn open_with(
        &self,
        deadline: Option<Instant>,
        arm: impl FnOnce(Instant) -> io::Result<DeadlineOwner>,
    ) -> Result<CancellationRun, CancellationSlotError> {
        // Retain pulls before allocating the ordinary flag. The guard then
        // owns retirement, including a failure while arming the deadline.
        let active = self.open_generation()?;
        let cancellation = Cancellation {
            slot: Some(Membership {
                state: Arc::clone(&self.state),
                active,
            }),
            ..Cancellation::default()
        };
        let mut run = CancellationRun { cancellation };
        if let Some(at) = deadline {
            run.cancellation.deadline =
                Some(Arc::new(arm(at).map_err(CancellationSlotError::Deadline)?));
        }
        Ok(run)
    }

    fn open_generation(&self) -> Result<u64, CancellationSlotError> {
        let mut observed = self.state.load(Ordering::Relaxed);
        loop {
            let generation = (observed & !STATUS_MASK)
                .checked_add(GENERATION_STEP)
                .ok_or(CancellationSlotError::GenerationExhausted)?;
            let active = generation | ACTIVE;
            match self.state.compare_exchange(
                observed,
                active,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(active),
                Err(current) => observed = current,
            }
        }
    }

    /// Request cancellation of the active run, if any. Repeated pulls are
    /// idempotent; a pull while idle has no effect on a later run.
    ///
    /// Constant work: one relaxed load and at most one strong compare-exchange,
    /// with no retry, allocation, lock or wait. If the observed run changes
    /// before the exchange, the pull does nothing to its replacement.
    pub fn cancel(&self) {
        self.cancel_observed(self.state.load(Ordering::Relaxed));
    }

    fn cancel_observed(&self, observed: u64) {
        if observed & STATUS_MASK == ACTIVE {
            let _ = self.state.compare_exchange(
                observed,
                (observed & !STATUS_MASK) | CANCELLED,
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
        }
    }

    /// Retire the currently observed run, for example when its backend drops.
    /// Its tokens then report cancellation and idle pulls affect no later run.
    /// A concurrent replacement is not retired. Constant work: one load and
    /// at most two strong compare-exchanges, with no allocation, lock or wait.
    pub fn clear(&self) {
        retire(&self.state, self.state.load(Ordering::Relaxed));
    }
}

/// The owner of one slot's active window.
///
/// Dropping this guard retires only its run, whose tokens subsequently report
/// cancellation. The guard is not cloned: token clones do not own the window.
/// Superseding this guard by opening another run stops its tokens as well.
/// Retirement makes at most two atomic attempts and cannot retire a newer
/// generation. Dropping the last deadline owner also wakes its timer through
/// the existing deadline retirement mechanism.
#[derive(Debug)]
#[must_use = "dropping the guard retires the run's cancellation window"]
pub struct CancellationRun {
    cancellation: Cancellation,
}

impl CancellationRun {
    /// The ordinary cancellation token for this run. Clone it for workers or
    /// owned sessions while retaining the guard until the whole run closes.
    /// Borrowing is constant work; cloning shares its flags and membership.
    #[must_use]
    pub fn cancellation(&self) -> &Cancellation {
        &self.cancellation
    }
}

impl Drop for CancellationRun {
    fn drop(&mut self) {
        let membership = self
            .cancellation
            .slot
            .as_ref()
            .expect("a slot run retains its membership");
        retire(&membership.state, membership.active);
    }
}

/// Why a run's cancellation window could not be opened.
#[derive(Debug)]
pub enum CancellationSlotError {
    /// Every representable generation was used; identities are never reused.
    GenerationExhausted,
    /// The operating system refused to start the deadline timer.
    Deadline(io::Error),
}

impl fmt::Display for CancellationSlotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GenerationExhausted => f.write_str("cancellation slot generations exhausted"),
            Self::Deadline(error) => write!(f, "cancellation deadline timer: {error}"),
        }
    }
}

impl std::error::Error for CancellationSlotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::GenerationExhausted => None,
            Self::Deadline(error) => Some(error),
        }
    }
}

/// Immutable run identity alongside the shared slot, held by ordinary tokens.
/// These atomic reads signal stopping only; they publish no other mutable data,
/// so relaxed ordering suffices throughout the slot's state machine.
#[derive(Clone, Debug)]
pub(super) struct Membership {
    state: Arc<AtomicU64>,
    active: u64,
}

impl Membership {
    #[inline]
    pub(super) fn polling(membership: Option<&Self>) -> Option<MembershipPoll<'_>> {
        let membership = membership?;
        Some(MembershipPoll {
            state: &membership.state,
            active: membership.active,
        })
    }
}

/// A live slot-state reference with the token's immutable expected generation.
#[derive(Clone, Copy, Debug)]
pub(super) struct MembershipPoll<'a> {
    state: &'a AtomicU64,
    active: u64,
}

impl MembershipPoll<'_> {
    #[inline]
    pub(super) fn is_cancelled(membership: Option<Self>) -> bool {
        let Some(membership) = membership else {
            return false;
        };
        membership.state.load(Ordering::Relaxed) != membership.active
    }
}

/// Retire this generation, whether a pull already cancelled it or races the
/// first exchange. Within one generation only active -> cancelled can race
/// retirement, so a second attempt suffices. Neither attempt names a later
/// generation; monotonic identities rule out ABA.
fn retire(state: &AtomicU64, observed: u64) {
    let generation = observed & !STATUS_MASK;
    if state
        .compare_exchange(
            generation | ACTIVE,
            generation,
            Ordering::Relaxed,
            Ordering::Relaxed,
        )
        .is_err_and(|current| current == (generation | CANCELLED))
    {
        let _ = state.compare_exchange(
            generation | CANCELLED,
            generation,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }
}

#[cfg(test)]
mod tests;
