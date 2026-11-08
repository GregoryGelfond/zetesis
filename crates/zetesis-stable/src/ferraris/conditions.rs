//! Candidate conditions and knowledge with distinct permanent and bound lifetimes.
//!
//! Replacing a bound assumes that every classical model of the unchanged
//! original theory and the new bound satisfies the preceding bound. Regions
//! already omitted therefore contain no newly eligible original models.
//! Knowledge learned about one bound's DAG never describes another DAG: queued
//! regions carry only its generation and private knowledge, while active readers
//! retain the exact immutable index through an `Arc` snapshot.

use std::borrow::Borrow;
use std::mem::size_of;
use std::sync::Arc;

use zetesis_algebra::{AnswerSetsSought, Countermodels};
use zetesis_ferraris::{
    CardinalityKnowledge, CardinalitySystem, Knowledge, Narrower, ParityKnowledge, ParitySystem,
    Theory,
};

use crate::Incomplete;

mod copy;

#[derive(Clone, Debug)]
pub(super) struct Bound {
    pub(super) generation: u64,
    pub(super) index: Arc<(Theory, Narrower)>,
}

impl Bound {
    pub(super) fn prepare(
        theory: &Theory,
        generation: u64,
        native_byte_limit: u64,
        cancellation: &crate::Cancellation,
    ) -> Result<Self, Incomplete> {
        let narrower = super::original_index::prepare(theory, native_byte_limit, cancellation)?;
        Ok(Self {
            generation,
            index: Arc::new((theory.clone(), narrower)),
        })
    }

    pub(super) fn work(&self) -> u64 {
        self.index.1.work()
    }

    /// Prepare the bound `restriction` makes at `generation` and charge its
    /// preparation to `budget` and to the search's `work` counter, installing
    /// nothing: a caller installs the bound only once this returns it, so a
    /// stop after a charge leaves that charge in place and the previous bound
    /// in force.
    pub(super) fn tightened(
        restriction: &Theory,
        generation: u64,
        native_byte_limit: u64,
        budget: &mut crate::budget::Budget<'_>,
        work: &mut u64,
    ) -> Result<Self, Incomplete> {
        budget.cancellation.poll()?;
        let bound = Self::prepare(
            restriction,
            generation,
            native_byte_limit,
            budget.cancellation,
        )?;
        budget.charge(bound.work())?;
        *work = work
            .checked_add(bound.work())
            .ok_or(Incomplete::CounterOverflow)?;
        budget.cancellation.poll()?;
        Ok(bound)
    }
}

#[derive(Clone, Debug)]
pub(super) struct Conditions<R> {
    pub(super) permanent: Vec<R>,
    pub(super) bound: Option<Bound>,
}

impl<R> Default for Conditions<R> {
    fn default() -> Self {
        Self {
            permanent: Vec::new(),
            bound: None,
        }
    }
}

impl<R: Borrow<(Theory, Narrower)>> Conditions<R> {
    pub(super) fn iter(&self) -> impl Iterator<Item = &(Theory, Narrower)> {
        self.permanent
            .iter()
            .map(Borrow::borrow)
            .chain(self.bound.iter().map(|bound| bound.index.as_ref()))
    }

    pub(super) fn is_empty(&self) -> bool {
        self.permanent.is_empty() && self.bound.is_none()
    }
}

/// No immutable bound owner is retained in a queued region. In particular, a
/// large inactive frontier cannot prolong the lifetime of a superseded DAG.
///
/// A region is queued with the knowledge it already has; the root, with none.
/// Every slot, the original theory's included, is created by
/// [`Self::permanent`] or [`Self::bound`] at the region's first narrowing
/// under its theory, so there is one creation path.
#[derive(Debug, Default)]
pub(super) struct CandidateKnowledge {
    /// Original theory first, then permanent restrictions, then the optional
    /// bound. Keeping one vector avoids another allocation on each region copy.
    entries: Vec<Knowledge<AnswerSetsSought>>,
    bound_generation: Option<u64>,
    /// Original arithmetic only; neither restrictions nor objective generations
    /// create or replace these system-specific descendant snapshots.
    parity: Option<ParityKnowledge>,
    cardinality: Option<CardinalityKnowledge>,
}

impl Clone for CandidateKnowledge {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            bound_generation: self.bound_generation,
            parity: self.parity.clone(),
            cardinality: self.cardinality.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.clone_entries_from(source);
        self.parity.clone_from(&source.parity);
        self.cardinality.clone_from(&source.cardinality);
    }
}

impl CandidateKnowledge {
    /// Copy the preceding non-arithmetic state once, with its existing exact
    /// outer-capacity policy. Charged arithmetic copying calls this operation
    /// directly, so it never first clones the new snapshots for free.
    fn clone_entries_from(&mut self, source: &Self) {
        let entries = &source.entries;
        // Published children retain no more vector capacity than a fresh
        // clone. Compatible entries reuse their arrays, overwriting every
        // conclusion and counter from the current parent.
        if self.entries.capacity() == entries.len() {
            self.entries.clone_from(entries);
        } else {
            self.entries = Vec::from(entries.as_slice());
        }
        self.bound_generation = source.bound_generation;
    }

    /// First inspection creates only this system's empty inline state. The
    /// runtime initializes its payload under the same search Meter.
    pub(super) fn parity(&mut self, system: &ParitySystem) -> &mut ParityKnowledge {
        self.parity.get_or_insert_with(|| system.knowledge())
    }

    /// Counting state has the same original-system lifetime as parity state.
    pub(super) fn cardinality(&mut self, system: &CardinalitySystem) -> &mut CardinalityKnowledge {
        self.cardinality.get_or_insert_with(|| system.knowledge())
    }

    /// The knowledge under the `index`th permanent theory, the original
    /// theory being the zeroth, created from `narrower` when this region has
    /// none under it yet. The first slot is reserved exactly, so a root that
    /// knows only the original theory holds one slot; later slots grow the
    /// vector as amortized growth does.
    pub(super) fn permanent(
        &mut self,
        index: usize,
        narrower: &Narrower,
    ) -> Result<&mut Knowledge<AnswerSetsSought>, Incomplete> {
        let permanent = self.entries.len() - usize::from(self.bound_generation.is_some());
        if index == permanent {
            let reserved = if self.entries.is_empty() {
                self.entries.try_reserve_exact(1)
            } else {
                self.entries.try_reserve(1)
            };
            reserved.map_err(|_| Incomplete::Allocation)?;
            // At most the bound header moves. Its knowledge remains tied to
            // its own generation, independently of later permanent additions.
            self.entries.insert(index, narrower.knowledge());
        }
        Ok(&mut self.entries[index])
    }

    pub(super) fn bound(
        &mut self,
        bound: &Bound,
    ) -> Result<&mut Knowledge<AnswerSetsSought>, Incomplete> {
        match self.bound_generation {
            Some(generation) if generation == bound.generation => {}
            Some(_) => {
                *self
                    .entries
                    .last_mut()
                    .expect("bound generation has a knowledge slot") = bound.index.1.knowledge();
            }
            None => {
                self.entries
                    .try_reserve(1)
                    .map_err(|_| Incomplete::Allocation)?;
                self.entries.push(bound.index.1.knowledge());
            }
        }
        self.bound_generation = Some(bound.generation);
        Ok(self
            .entries
            .last_mut()
            .expect("bound generation has a knowledge slot"))
    }

    /// Original and currently preferred knowledge after joint propagation has
    /// closed every subject in `conditions`. Both references borrow the same
    /// stored entries used by propagation; no index or counters are copied.
    pub(super) fn preference_subjects<R>(
        &self,
        conditions: &Conditions<R>,
    ) -> (&Knowledge<AnswerSetsSought>, &Knowledge<AnswerSetsSought>) {
        let current = conditions.permanent.len() + usize::from(conditions.bound.is_some());
        (&self.entries[0], &self.entries[current])
    }

    /// Owned allocations only; the enclosing frontier counts this value's
    /// inline vector and generation headers.
    pub(super) fn allocated_bytes(&self) -> u128 {
        self.entries.capacity() as u128 * size_of::<Knowledge<AnswerSetsSought>>() as u128
            + self
                .entries
                .iter()
                .map(|known| {
                    known.retained_bytes() - size_of::<Knowledge<AnswerSetsSought>>() as u128
                })
                .sum::<u128>()
            + self.parity.as_ref().map_or(0, |known| {
                known.retained_bytes() - size_of::<ParityKnowledge>() as u128
            })
            + self.cardinality.as_ref().map_or(0, |known| {
                known.retained_bytes() - size_of::<CardinalityKnowledge>() as u128
            })
    }
}

/// A region's knowledge in a countermodel walk: the frozen reduct's, carried
/// from the root, and the candidate's inner systems' slots, created at first
/// inspection as the candidate walk's are. A walk without a preparation never
/// creates a slot and carries exactly the frozen knowledge.
#[derive(Debug)]
pub(crate) struct CountermodelKnowledge {
    pub(crate) frozen: Knowledge<Countermodels>,
    pub(crate) parity: Option<ParityKnowledge>,
    pub(crate) cardinality: Option<CardinalityKnowledge>,
}

impl Clone for CountermodelKnowledge {
    fn clone(&self) -> Self {
        Self {
            frozen: self.frozen.clone(),
            parity: self.parity.clone(),
            cardinality: self.cardinality.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.frozen.clone_from(&source.frozen);
        self.parity.clone_from(&source.parity);
        self.cardinality.clone_from(&source.cardinality);
    }
}

impl CountermodelKnowledge {
    /// The root's knowledge: the frozen reduct's, with no inner slot yet.
    pub(crate) fn fresh(narrower: &Narrower) -> Self {
        Self {
            frozen: narrower.frozen_knowledge(),
            parity: None,
            cardinality: None,
        }
    }

    pub(crate) const fn frozen(&mut self) -> &mut Knowledge<Countermodels> {
        &mut self.frozen
    }

    /// First inspection creates only this system's empty inline state.
    pub(crate) fn parity(&mut self, system: &ParitySystem) -> &mut ParityKnowledge {
        self.parity.get_or_insert_with(|| system.knowledge())
    }

    pub(crate) fn cardinality(&mut self, system: &CardinalitySystem) -> &mut CardinalityKnowledge {
        self.cardinality.get_or_insert_with(|| system.knowledge())
    }
}

#[cfg(test)]
mod tests;
