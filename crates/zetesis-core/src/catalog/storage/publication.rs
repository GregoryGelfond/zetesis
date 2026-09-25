//! Reserve all prefix directories before moving either mutable component. A
//! frozen vocabulary is retained by Arc, never copied into execution snapshots.

use std::{mem::size_of, sync::Arc};

use super::vocabulary::{Vocabulary, VocabularyData};
use super::{Fault, RowSegment, Snapshot, SnapshotData, Store, VocabularySegment, budget};

impl Store {
    pub(crate) fn publication_steps(&self) -> Result<usize, Fault> {
        let rows = directory_steps(&self.sealed, !self.tail.is_empty())?;
        let vocabulary = match &self.vocabulary {
            Vocabulary::Growing(growing) => {
                directory_steps(&growing.sealed, !growing.tail.is_empty())?
            }
            Vocabulary::Frozen(_) => 0,
        };
        rows.checked_add(vocabulary)
            .and_then(|steps| steps.checked_add(16))
            .ok_or(Fault::Overflow)
    }

    pub(crate) fn snapshot(&mut self, extra: u128) -> Result<Snapshot, Fault> {
        self.budget.check()?;
        record(&mut self.budget, extra)?;
        let rows_added = !self.tail.is_empty();
        let rows_count = self
            .sealed
            .len()
            .checked_add(usize::from(rows_added))
            .ok_or(Fault::Overflow)?;
        let vocabulary_added = self.vocabulary.has_unpublished();
        let pending = self.publication_capacity(extra, rows_added, vocabulary_added)?;
        let mut publication = self.budget.clone();
        publication.check_extra(pending)?;
        publication.used += pending;
        if rows_added {
            reserve_writer(
                &mut self.sealed,
                &mut self.budget,
                &mut publication,
                pending,
                extra,
            )?;
        }
        if let Vocabulary::Growing(growing) = &mut self.vocabulary
            && vocabulary_added
        {
            reserve_writer(
                &mut growing.sealed,
                &mut self.budget,
                &mut publication,
                pending,
                extra,
            )?;
        }
        let mut rows = Vec::new();
        let mut vocabulary = Vec::new();
        let result = budget::reserve(&mut rows, rows_count, &mut publication);
        record(&mut self.budget, extra + budget::capacity(&rows))?;
        result?;
        if let Vocabulary::Growing(growing) = &self.vocabulary {
            let count = growing
                .sealed
                .len()
                .checked_add(usize::from(vocabulary_added))
                .ok_or(Fault::Overflow)?;
            let result = budget::reserve(&mut vocabulary, count, &mut publication);
            record(
                &mut self.budget,
                extra + budget::capacity(&rows) + budget::capacity(&vocabulary),
            )?;
            result?;
        }
        // No fallible allocation remains after these envelope checks. Logical
        // payload and completed prefixes cannot change on an earlier refusal.
        self.budget.add(if rows_added {
            size_of::<RowSegment>() as u128
        } else {
            0
        })?;
        self.budget.add(if vocabulary_added {
            size_of::<VocabularySegment>() as u128
        } else {
            0
        })?;
        let vocabulary = seal_vocabulary(&mut self.vocabulary, vocabulary, vocabulary_added);
        rows.extend(self.sealed.iter().cloned());
        let atoms = self.tail.count();
        if rows_added {
            let tail = std::mem::replace(&mut self.tail, RowSegment::new(atoms));
            let segment = Arc::new(tail);
            rows.push(Arc::clone(&segment));
            self.sealed.push(segment);
        }
        let snapshot = Snapshot {
            atom_owner: Arc::clone(&self.atom_owner),
            data: Arc::new(SnapshotData {
                vocabulary,
                base: self.base.as_ref().map(|base| Arc::clone(&base.payload)),
                segments: rows,
                atoms,
            }),
        };
        record(&mut self.budget, extra + snapshot.metadata_bytes())?;
        Ok(snapshot)
    }

    /// Headers that coexist while the new immutable prefix is published.
    /// Directory capacities are reserved separately after this preflight.
    fn publication_capacity(
        &self,
        extra: u128,
        rows_added: bool,
        vocabulary_added: bool,
    ) -> Result<u128, Fault> {
        let vocabulary_header = match &self.vocabulary {
            Vocabulary::Growing(_) => size_of::<VocabularyData>(),
            Vocabulary::Frozen(_) => 0,
        };
        extra
            .checked_add(size_of::<SnapshotData>() as u128)
            .and_then(|bytes| bytes.checked_add(vocabulary_header as u128))
            .and_then(|bytes| {
                bytes.checked_add(if rows_added {
                    size_of::<RowSegment>() as u128
                } else {
                    0
                })
            })
            .and_then(|bytes| {
                bytes.checked_add(if vocabulary_added {
                    size_of::<VocabularySegment>() as u128
                } else {
                    0
                })
            })
            .ok_or(Fault::Overflow)
    }
}

fn seal_vocabulary(
    owner: &mut Vocabulary,
    mut segments: Vec<Arc<VocabularySegment>>,
    append: bool,
) -> Arc<VocabularyData> {
    match owner {
        Vocabulary::Frozen(base) => Arc::clone(&base.data),
        Vocabulary::Growing(growing) => {
            segments.extend(growing.sealed.iter().cloned());
            let counts = growing.tail.counts();
            if append {
                let tail = std::mem::replace(&mut growing.tail, VocabularySegment::new(counts));
                let segment = Arc::new(tail);
                segments.push(Arc::clone(&segment));
                growing.sealed.push(segment);
            }
            Arc::new(VocabularyData {
                owner: Arc::clone(&growing.owner),
                segments,
                counts,
                frozen: false,
            })
        }
    }
}

fn directory_steps<T>(segments: &Vec<Arc<T>>, append: bool) -> Result<usize, Fault> {
    let grows = append && segments.len() == segments.capacity();
    segments
        .len()
        .checked_mul(1 + usize::from(grows))
        .ok_or(Fault::Overflow)
}

fn reserve_writer<T>(
    segments: &mut Vec<Arc<T>>,
    owner: &mut budget::Budget,
    publication: &mut budget::Budget,
    pending: u128,
    extra: u128,
) -> Result<(), Fault> {
    let before = budget::capacity(segments);
    let used = owner.used;
    let result = budget::reserve(segments, 1, publication);
    owner.used = publication.used - pending;
    let after = budget::capacity(segments);
    if after > before {
        owner.peak = owner.peak.max(used + extra + after);
    }
    result
}

fn record(owner: &mut budget::Budget, extra: u128) -> Result<(), Fault> {
    owner.peak = owner
        .peak
        .max(owner.used.checked_add(extra).ok_or(Fault::Overflow)?);
    Ok(())
}
