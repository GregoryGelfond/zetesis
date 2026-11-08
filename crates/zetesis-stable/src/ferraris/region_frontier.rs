//! Candidate entry state and typed refusal for the shared algebra frontier.
//!
//! All deque, publication, idle and receipt behavior belongs to the one generic
//! frontier. This module supplies only candidate-owned allocation metadata and
//! the existing semantic conversion from `Stop` to `Incomplete`.

use std::collections::VecDeque;

use zetesis_algebra::walk::frontier::{Frontier, Payload, reserve_entries};

pub(super) use zetesis_algebra::walk::frontier::{Attempt, Busy, Termination};

use super::conditions::CandidateKnowledge;
use crate::Incomplete;

#[cfg(test)]
mod tests;

/// A candidate interval and its lineage, using the shared entry policy.
pub(super) type Entry = zetesis_algebra::region::Entry<CandidateKnowledge>;

impl Payload for CandidateKnowledge {
    fn allocated_bytes(&self) -> u128 {
        Self::allocated_bytes(self)
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
