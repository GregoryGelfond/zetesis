//! Candidate conditions and knowledge with distinct permanent and bound lifetimes.
//!
//! Replacing a bound assumes that its classical models are a subset of the
//! preceding bound's models. Regions already omitted therefore stay omitted.
//! Knowledge learned about one bound's DAG never describes another DAG: queued
//! regions carry only its generation and private knowledge, while active readers
//! retain the exact immutable index through an `Arc` snapshot.

use std::borrow::Borrow;
use std::mem::size_of;
use std::sync::Arc;

use zetesis_ferraris::{Knowledge, Narrower, Theory};

use crate::Incomplete;

#[derive(Clone, Debug)]
pub(super) struct Bound {
    pub(super) generation: u64,
    pub(super) index: Arc<(Theory, Narrower)>,
}

impl Bound {
    pub(super) fn prepare(theory: &Theory, generation: u64) -> Result<Self, Incomplete> {
        let narrower = Narrower::try_new(theory).map_err(super::regions::stopped)?;
        Ok(Self {
            generation,
            index: Arc::new((theory.clone(), narrower)),
        })
    }

    pub(super) fn work(&self) -> u64 {
        self.index.1.work()
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
#[derive(Clone, Debug, Default)]
pub(super) struct CandidateKnowledge {
    /// Original theory first, then permanent restrictions, then the optional
    /// bound. Keeping one vector avoids another allocation on each region copy.
    entries: Vec<Knowledge>,
    bound_generation: Option<u64>,
}

impl CandidateKnowledge {
    pub(super) fn new(original: Knowledge) -> Self {
        Self {
            entries: vec![original],
            bound_generation: None,
        }
    }

    pub(super) fn permanent(
        &mut self,
        index: usize,
        narrower: &Narrower,
    ) -> Result<&mut Knowledge, Incomplete> {
        let permanent = self.entries.len() - usize::from(self.bound_generation.is_some());
        if index == permanent {
            self.entries
                .try_reserve(1)
                .map_err(|_| Incomplete::Allocation)?;
            // At most the bound header moves. Its knowledge remains tied to
            // its own generation, independently of later permanent additions.
            self.entries.insert(index, narrower.knowledge());
        }
        Ok(&mut self.entries[index])
    }

    pub(super) fn bound(&mut self, bound: &Bound) -> Result<&mut Knowledge, Incomplete> {
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

    /// Owned allocations only; the enclosing frontier counts this value's
    /// inline vector and generation headers.
    pub(super) fn allocated_bytes(&self) -> u128 {
        self.entries.capacity() as u128 * size_of::<Knowledge>() as u128
            + self
                .entries
                .iter()
                .map(|known| known.retained_bytes() - size_of::<Knowledge>() as u128)
                .sum::<u128>()
    }
}

#[cfg(test)]
mod tests;
