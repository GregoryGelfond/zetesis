//! Candidate entry state and typed refusal for the shared algebra frontier.
//!
//! All deque, publication, idle and receipt behavior belongs to the one generic
//! frontier. This module supplies only candidate-owned allocation metadata and
//! the existing semantic conversion from `Stop` to `Incomplete`.

use std::collections::VecDeque;
use std::mem::size_of;

use zetesis_algebra::region::Region;
use zetesis_algebra::walk::frontier::{Frontier, Payload, reserve_entries};

pub(super) use zetesis_algebra::walk::frontier::{Attempt, Busy, Termination};

use super::conditions::CandidateKnowledge;
use crate::Incomplete;

#[cfg(test)]
mod tests;

/// A candidate region and the knowledge retained for that region.
#[derive(Clone, Debug)]
pub(super) struct Entry(pub(super) Region, pub(super) CandidateKnowledge);

impl Payload for Entry {
    fn allocated_bytes(&self) -> u128 {
        self.0.retained_bytes() - size_of::<Region>() as u128 + self.1.allocated_bytes()
    }
}

/// The sole frontier protocol specialized to candidate state and solver errors.
pub(super) type RegionFrontier = Frontier<Entry, Incomplete>;

/// Preserve the caller's typed allocation refusal at the shared reservation.
pub(super) fn reserve_regions(
    queue: &mut VecDeque<Entry>,
    additional: usize,
) -> Result<(), Incomplete> {
    reserve_entries(queue, additional)
}
