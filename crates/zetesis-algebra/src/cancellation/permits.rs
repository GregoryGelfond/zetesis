//! Incremental permit transitions over cumulative or leased allowance.
//!
//! The two views borrow their actual owners. Shared transitions consume the
//! available prefix and detach its settlement; source admission, synchronization
//! and polling remain at their original boundaries.

use std::cell::Cell;

/// One detached grant's used and unused parts, for its source to publish.
///
/// This is a transfer receipt, not another permit account. The source must
/// publish it exactly once. Construction preserves `unused <= granted`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PermitSettlement {
    granted: u64,
    unused: u64,
}

impl PermitSettlement {
    /// The complete grant removed from its local owner.
    #[must_use]
    pub const fn granted(&self) -> u64 {
        self.granted
    }

    /// The part consumed before settlement.
    #[must_use]
    pub const fn used(&self) -> u64 {
        self.granted - self.unused
    }

    /// The part available for return to the source.
    #[must_use]
    pub const fn unused(&self) -> u64 {
        self.unused
    }
}

/// A query-local incremental grant, replenished by its source adapter.
///
/// The two Cells are the sole grant owner; this type is deliberately not Sync.
/// It owns no source reference, poll, lock or allocation. The source wrapper
/// decides when to settle and whether its Drop settles. This owner itself has
/// no automatic Drop settlement: unwinding does not invent a work receipt.
pub struct PermitLease {
    granted: Cell<u64>,
    remaining: Cell<u64>,
}

impl PermitLease {
    /// An empty lease, without a source observation or reservation.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            granted: Cell::new(0),
            remaining: Cell::new(0),
        }
    }

    /// Consume one permit, refilling only when the grant is exhausted.
    /// A successful refill must return with a positive, unconsumed replacement
    /// grant installed in this same lease through [`Self::renew`].
    ///
    /// # Errors
    /// Returns the refill's exact error before consuming the next permit.
    #[inline]
    pub fn tick<E>(&self, refill: impl FnOnce() -> Result<(), E>) -> Result<(), E> {
        tick(&mut Leased(self), |_| refill())
    }

    /// Consume a prefix grant by grant. Zero does not invoke `refill`.
    /// Each successful refill must return with a positive, unconsumed
    /// replacement grant installed in this same lease through [`Self::renew`].
    ///
    /// # Errors
    /// A refused refill preserves permits consumed from earlier grants.
    #[inline]
    pub fn take<E>(
        &self,
        mut amount: u64,
        mut refill: impl FnMut() -> Result<(), E>,
    ) -> Result<(), E> {
        let mut state = Leased(self);
        while amount > 0 {
            if state.available() == 0 {
                refill()?;
            }
            amount -= take_available(&mut state, amount);
        }
        Ok(())
    }

    /// Consume up to `wanted` permits from one grant. An exhausted lease
    /// invokes `refill` even when `wanted` is zero. A successful refill must
    /// return with a positive, unconsumed replacement grant installed in this
    /// same lease through [`Self::renew`]. An existing grant needs no refill.
    ///
    /// # Errors
    /// Returns the refill's exact error without consuming from a new grant.
    #[inline]
    pub fn take_up_to<E>(
        &self,
        wanted: u64,
        refill: impl FnOnce() -> Result<(), E>,
    ) -> Result<u64, E> {
        let mut state = Leased(self);
        if state.available() == 0 {
            refill()?;
        }
        Ok(take_available(&mut state, wanted))
    }

    /// Return permits borrowed from the current grant and not spent.
    ///
    /// # Panics
    /// The returned amount must fit the current grant's consumed part. Debug
    /// builds assert that precondition, as at the source lease boundary.
    #[inline]
    pub fn give_back(&self, unused: u64) {
        let remaining = self.remaining.get() + unused;
        debug_assert!(
            remaining <= self.granted.get(),
            "permits return to their own grant"
        );
        self.remaining.set(remaining);
    }

    /// Detach the grant before publishing its settlement. An empty lease does
    /// not invoke `publish`. If publication unwinds, the grant remains detached;
    /// the source adapter retains responsibility for its ledger transition.
    #[inline]
    pub fn settle(&self, publish: impl FnOnce(PermitSettlement)) {
        settle(&mut Leased(self), publish);
    }

    /// Renew an exhausted lease at its source's existing admission boundary.
    ///
    /// Detach the old grant before calling `next`, supplying a zero receipt
    /// when none exists. The source publishes that receipt before attempting a
    /// new grant. `Some(n)` installs both Cells before return; `None` leaves the
    /// lease empty and reports pending. No polling, locking or waiting is added.
    /// During `next`, the callback must not invoke any operation on this same
    /// lease, directly or indirectly; it operates only on the source ledger.
    ///
    /// # Errors
    /// Returns the source's exact error with the old grant detached and no
    /// replacement installed. No rollback or additional observation occurs.
    ///
    /// # Panics
    /// The current grant must be exhausted, and a successful replacement must
    /// be positive. Debug builds assert these source-adapter preconditions.
    /// An unwinding `next` leaves the old grant detached, without restoration.
    #[inline]
    pub fn renew<E>(
        &self,
        next: impl FnOnce(PermitSettlement) -> Result<Option<u64>, E>,
    ) -> Result<bool, E> {
        debug_assert_eq!(self.remaining.get(), 0);
        let prior = Leased(self).detach().unwrap_or(PermitSettlement {
            granted: 0,
            unused: 0,
        });
        match next(prior)? {
            Some(granted) => {
                debug_assert!(granted > 0, "a successful renewal grants permits");
                self.granted.set(granted);
                self.remaining.set(granted);
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

impl Default for PermitLease {
    fn default() -> Self {
        Self::new()
    }
}

/// Only the representation writes differ; the transitions below are shared.
pub(super) trait State {
    fn available(&self) -> u64;
    fn consume(&mut self, amount: u64);
    fn detach(&mut self) -> Option<PermitSettlement>;
}

/// A borrowed view of Meter's actual charged count and admitted total.
pub(super) struct Counted<'a> {
    pub(super) charged: &'a mut u64,
    pub(super) admitted: &'a mut u64,
}

impl State for Counted<'_> {
    #[inline]
    fn available(&self) -> u64 {
        *self.admitted - *self.charged
    }

    #[inline]
    fn consume(&mut self, amount: u64) {
        *self.charged += amount;
    }

    #[inline]
    fn detach(&mut self) -> Option<PermitSettlement> {
        let prior = PermitSettlement {
            granted: *self.admitted,
            unused: self.available(),
        };
        *self.admitted = *self.charged;
        Some(prior)
    }
}

/// Shared references permit the source refill to renew these same Cells.
struct Leased<'a>(&'a PermitLease);

impl State for Leased<'_> {
    #[inline]
    fn available(&self) -> u64 {
        self.0.remaining.get()
    }

    #[inline]
    fn consume(&mut self, amount: u64) {
        self.0.remaining.set(self.0.remaining.get() - amount);
    }

    #[inline]
    fn detach(&mut self) -> Option<PermitSettlement> {
        let granted = self.0.granted.replace(0);
        if granted == 0 {
            return None;
        }
        Some(PermitSettlement {
            granted,
            unused: self.0.remaining.replace(0),
        })
    }
}

#[inline]
pub(super) fn tick<S: State, E>(
    state: &mut S,
    refill: impl FnOnce(&mut S) -> Result<(), E>,
) -> Result<(), E> {
    if state.available() == 0 {
        return refill_and_tick(state, refill);
    }
    state.consume(1);
    Ok(())
}

// Finish consumption on the refill path so its caller can return the final
// outcome directly, without inspecting an intermediate refill result.
#[cold]
fn refill_and_tick<S: State, E>(
    state: &mut S,
    refill: impl FnOnce(&mut S) -> Result<(), E>,
) -> Result<(), E> {
    refill(state)?;
    state.consume(1);
    Ok(())
}

#[inline]
pub(super) fn take_available(state: &mut impl State, wanted: u64) -> u64 {
    let taken = state.available().min(wanted);
    state.consume(taken);
    taken
}

#[inline]
pub(super) fn settle(state: &mut impl State, publish: impl FnOnce(PermitSettlement)) {
    if let Some(prior) = state.detach() {
        publish(prior);
    }
}

#[cfg(test)]
mod tests;
