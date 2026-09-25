//! A moved immutable row/index base, followed by at most one append layer.

use std::sync::Arc;

use super::control::Work;
use super::{
    Budget, Failure, Fault, FrozenVocabulary, Index, Owner, RowBase, RowSegment, Snapshot,
    SnapshotData, Store, Vocabulary, budget,
};
use crate::catalog::ReadError;

#[derive(Debug)]
pub(super) struct IndexedRows {
    pub(super) payload: Arc<RowBase>,
    pub(super) index: Index,
    source_owner: Arc<Owner>,
}

#[derive(Clone, Debug)]
pub(crate) struct Closed {
    pub(super) vocabulary: FrozenVocabulary,
    pub(super) rows: Arc<IndexedRows>,
    bytes: u128,
    peak: u128,
}

#[derive(Debug)]
pub(crate) struct CloseError<E> {
    pub(crate) failure: Failure<E>,
    pub(crate) peak: u128,
}

impl Closed {
    pub(crate) const fn retained_bytes(&self) -> u128 {
        self.bytes
    }
    pub(crate) const fn publication_peak(&self) -> u128 {
        self.peak
    }
    pub(crate) fn vocabulary(&self) -> &FrozenVocabulary {
        &self.vocabulary
    }

    /// One original append scope fixes the immutable segment lineage. Extents
    /// authenticate an older prefix; payload equality alone never authorizes a
    /// sharing deduction. This invariant excludes descendant and sibling scopes.
    pub(crate) fn prior_metadata(&self, prior: &Snapshot) -> Result<u128, ReadError> {
        let vocabulary = &prior.data.vocabulary;
        if !Arc::ptr_eq(&prior.atom_owner, &self.rows.source_owner)
            || !Arc::ptr_eq(&vocabulary.owner, &self.vocabulary.data.owner)
        {
            return Err(ReadError::ForeignCatalog);
        }
        let current = self.vocabulary.data.counts;
        let old = vocabulary.counts;
        if prior.data.atoms > self.rows.payload.atoms
            || old.texts > current.texts
            || old.terms > current.terms
            || old.predicates > current.predicates
        {
            return Err(ReadError::OutsidePrefix);
        }
        Ok(size_of::<SnapshotData>() as u128
            + budget::capacity(&prior.data.segments)
            + if Arc::ptr_eq(vocabulary, &self.vocabulary.data) {
                0
            } else {
                vocabulary.metadata_bytes()
            })
    }
}

impl Store {
    pub(crate) fn with_closed(base: &Closed, max_bytes: usize) -> Result<Self, Fault> {
        let mut budget = Budget::new(max_bytes, size_of::<Self>());
        budget.add(base.retained_bytes())?;
        Ok(Self {
            atom_owner: Arc::new(Owner),
            vocabulary: Vocabulary::Frozen(base.vocabulary.clone()),
            base: Some(Arc::clone(&base.rows)),
            sealed: Vec::new(),
            tail: RowSegment::new(base.rows.payload.atoms),
            atoms: Index::default(),
            budget,
        })
    }

    pub(crate) fn shared_closed_bytes(&self) -> u128 {
        self.base.as_ref().map_or(0, |base| {
            self.shared_vocabulary_bytes()
                + size_of::<IndexedRows>() as u128
                + base.payload.bytes
                + base.index.buffer_bytes()
        })
    }

    /// Consume the sole original writer. Its current snapshot/discovery metadata
    /// remains live in `extra` until publication finishes. Refusal returns the
    /// actual attempt receipt; callback panic promises only old-prefix safety.
    pub(crate) fn close_with<E>(
        mut self,
        extra: u128,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Closed, CloseError<E>> {
        let mut peak = self.budget.used + extra;
        let plan = self.prepare_close(extra, &mut peak, &mut Work::new(&mut before));
        let plan = plan.map_err(|failure| CloseError { failure, peak })?;
        // All fallible reserves and caller callbacks finished. Fixed Arc
        // envelopes use the same infallible boundary as ordinary publication.
        peak = peak.max(plan.admission.used);
        let vocabulary = self
            .vocabulary
            .finish_freeze(peak)
            .map_err(|error| CloseError {
                failure: Failure::Storage(error),
                peak,
            })?;
        if !self.tail.is_empty() {
            self.sealed.push(Arc::new(self.tail));
        }
        let payload = Arc::new(RowBase {
            segments: self.sealed,
            atoms: plan.atoms,
            bytes: plan.row_bytes,
        });
        let rows = Arc::new(IndexedRows {
            payload,
            index: self.atoms,
            source_owner: self.atom_owner,
        });
        let bytes = vocabulary.retained_bytes()
            + plan.row_bytes
            + size_of::<IndexedRows>() as u128
            + rows.index.buffer_bytes();
        Ok(Closed {
            vocabulary,
            rows,
            bytes,
            peak,
        })
    }

    fn prepare_close<E>(
        &mut self,
        extra: u128,
        peak: &mut u128,
        work: &mut Work<'_, E>,
    ) -> Result<Plan, Failure<E>> {
        work.step()?;
        self.budget.check_extra(extra)?;
        if self.base.is_some() {
            return Err(Fault::CatalogHasBase.into());
        }
        let append = !self.tail.is_empty();
        let envelopes = self.vocabulary.freeze_envelopes()
            + size_of::<Closed>() as u128
            + size_of::<IndexedRows>() as u128
            + size_of::<RowBase>() as u128
            + if append {
                size_of::<RowSegment>() as u128
            } else {
                0
            };
        let mut admission = self.budget.clone();
        admission.used += extra;
        admission.peak = admission.used;
        admission.check_extra(envelopes)?;
        admission.used += envelopes;
        admission.peak = admission.used;
        if append {
            let result = work.reserve(&mut self.sealed, 1, &mut admission);
            *peak = (*peak).max(admission.peak - envelopes);
            result?;
        }
        let result = self.vocabulary.prepare_freeze(&mut admission, work);
        *peak = (*peak).max(admission.peak - envelopes);
        result?;
        let mut row_bytes = size_of::<RowBase>() as u128 + budget::capacity(&self.sealed);
        for segment in &self.sealed {
            row_bytes += segment.bytes_with(|| work.step())?;
        }
        if append {
            row_bytes += self.tail.bytes_with(|| work.step())?;
        }
        work.steps(8)?;
        Ok(Plan {
            admission,
            row_bytes,
            atoms: self.tail.count(),
        })
    }
}

struct Plan {
    admission: Budget,
    row_bytes: u128,
    atoms: usize,
}

#[cfg(test)]
mod tests;
