//! The learned rows one solve's candidate streams share.
//!
//! A [`LearnedStore`] holds, by consequence class, the rows its streams' tiers
//! delivered. Its public surface is creating an empty store and handing it to
//! streams ([`crate::StableModels::set_learned_store`]): no public operation
//! adds, seeds, edits or reads a row, so a caller chooses which streams share
//! a store, never what is learned. A walk seeds a tier from the store before
//! the tier's first narrowing and delivers it at the walk's boundaries: when
//! the walk ends, when a parallel walk's coordinator hands over to its
//! workers, at the GPU route's slate joins, and each time the walk has visited
//! the owner's `publish_regions` regions since the tier last delivered. None
//! of these is inside a narrowing. One lock serializes the store; the tier
//! pays for every read.

use std::sync::{Mutex, MutexGuard, PoisonError};

use zetesis_ferraris::LearningStore;

/// The learned rows the candidate streams of one solve share, by consequence
/// class. Create an empty store and hand it to every stream that should
/// share; rows enter it only from those streams' own narrowing.
#[derive(Debug, Default)]
pub struct LearnedStore {
    rows: Mutex<LearningStore>,
}

impl LearnedStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Exclusive access for one seeding or delivery. Retention publishes a
    /// row only after every fallible step succeeds, so a panic while the lock
    /// was held leaves only complete rows.
    pub(crate) fn lock(&self) -> MutexGuard<'_, LearningStore> {
        self.rows.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A walk's sharing state beside its scratch's tier: whether the tier has
/// drawn the store's rows, and the regions the walk visited since the tier
/// last delivered.
#[derive(Debug, Default)]
pub(crate) struct Tier {
    pub(super) seeded: bool,
    pub(super) visited: u64,
}

impl Tier {
    /// Draw the store's rows again before the tier's next narrowing, as a
    /// producer does after each slate's join.
    pub(crate) fn reseed(&mut self) {
        self.seeded = false;
    }
}

#[cfg(test)]
mod tests;
