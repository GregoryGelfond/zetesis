//! One mutable vocabulary or an exact closed base. Frozen payload and lookup
//! indexes have separate shared owners: immutable result reads retain no index.

use std::{mem::size_of, sync::Arc};

use super::control::Work;
use super::{Counts, Failure, Fault, Index, Owner, Store, VocabularySegment, budget, locate};

#[derive(Debug, Default)]
pub(super) struct Indexes {
    pub(super) texts: Index,
    pub(super) terms: Index,
    pub(super) predicates: Index,
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    use crate::catalog::Limits;
    use crate::{Value, ValueLimits, ValueNode};

    #[test]
    fn empty_publication_reports_allocated_envelopes_and_external_metadata() {
        let owner = Store::new(1 << 20);
        let external = 137;
        let expected = owner.current_bytes()
            + external
            + size_of::<VocabularyData>() as u128
            + size_of::<Indexes>() as u128;
        let frozen = owner
            .freeze_vocabulary_with(external, || Ok::<_, ()>(()))
            .unwrap();
        assert_eq!(frozen.publication_peak(), expected);
    }

    #[test]
    fn publication_receipt_excludes_earlier_import_scratch() {
        let depth = 2_000;
        let mut nodes = vec![ValueNode::Tuple { arity: 1 }; depth - 1];
        nodes.push(ValueNode::Number(1));
        let value = Value::from_nodes(
            nodes,
            ValueLimits {
                max_nodes: depth,
                max_depth: depth,
                max_bytes: 1 << 20,
            },
        )
        .unwrap();
        let mut owner = Store::new(1 << 20);
        owner
            .import_value(
                &value,
                Limits {
                    max_nodes: depth,
                    max_depth: depth,
                    max_bytes: 1 << 20,
                },
            )
            .unwrap();
        let import_peak = owner.peak_bytes();
        let frozen = owner.freeze_vocabulary_with(0, || Ok::<_, ()>(())).unwrap();
        assert!(frozen.publication_peak() < import_peak);
        assert!(frozen.publication_peak() >= frozen.retained_bytes());
    }
}
impl Indexes {
    fn bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + self.texts.buffer_bytes()
            + self.terms.buffer_bytes()
            + self.predicates.buffer_bytes()
    }
}

#[derive(Debug)]
pub(super) struct Growing {
    pub(super) owner: Arc<Owner>,
    pub(super) sealed: Vec<Arc<VocabularySegment>>,
    pub(super) tail: VocabularySegment,
    pub(super) indexes: Indexes,
}

#[derive(Debug)]
pub(super) enum Vocabulary {
    Growing(Box<Growing>),
    Frozen(FrozenVocabulary),
}
impl Vocabulary {
    pub(super) fn new() -> Self {
        Self::Growing(Box::new(Growing {
            owner: Arc::new(Owner),
            sealed: Vec::new(),
            tail: VocabularySegment::new(Counts::default()),
            indexes: Indexes::default(),
        }))
    }
    pub(super) fn owner(&self) -> &Arc<Owner> {
        match self {
            Self::Growing(growing) => &growing.owner,
            Self::Frozen(base) => &base.data.owner,
        }
    }
    pub(super) fn counts(&self) -> Counts {
        match self {
            Self::Growing(growing) => growing.tail.counts(),
            Self::Frozen(base) => base.data.counts,
        }
    }
    pub(super) fn indexes(&self) -> &Indexes {
        match self {
            Self::Growing(growing) => &growing.indexes,
            Self::Frozen(base) => &base.indexes,
        }
    }
    pub(super) fn growing(&mut self) -> Result<&mut Growing, Fault> {
        match self {
            Self::Growing(growing) => Ok(growing),
            Self::Frozen(_) => Err(Fault::FrozenVocabulary),
        }
    }
    /// Used only after growth admission, while the exclusive writer keeps mode fixed.
    pub(super) fn admitted_growing(&mut self) -> &mut Growing {
        self.growing()
            .expect("growing vocabulary admitted before publication")
    }
    pub(super) fn has_unpublished(&self) -> bool {
        matches!(self, Self::Growing(growing) if !growing.tail.is_empty())
    }
    pub(super) fn empty_prefix(&self) -> Arc<VocabularyData> {
        match self {
            Self::Growing(growing) => Arc::new(VocabularyData::empty(Arc::clone(&growing.owner))),
            Self::Frozen(base) => Arc::clone(&base.data),
        }
    }
    pub(super) fn segment(
        &self,
        id: usize,
        start: impl Fn(Counts) -> usize,
    ) -> Option<&VocabularySegment> {
        match self {
            Self::Growing(growing) if id >= start(growing.tail.start) => Some(&growing.tail),
            Self::Growing(growing) => locate(&growing.sealed, id, |segment| start(segment.start)),
            Self::Frozen(base) => locate(&base.data.segments, id, |segment| start(segment.start)),
        }
    }
}

#[derive(Debug)]
pub(super) struct VocabularyData {
    pub(super) owner: Arc<Owner>,
    pub(super) segments: Vec<Arc<VocabularySegment>>,
    pub(super) counts: Counts,
    pub(super) frozen: bool,
}
impl VocabularyData {
    pub(super) fn empty(owner: Arc<Owner>) -> Self {
        Self {
            owner,
            segments: Vec::new(),
            counts: Counts::default(),
            frozen: false,
        }
    }
    pub(super) fn metadata_bytes(&self) -> u128 {
        size_of::<Self>() as u128 + budget::capacity(&self.segments)
    }
    pub(super) fn retained_bytes(&self) -> u128 {
        self.metadata_bytes()
            + self
                .segments
                .iter()
                .map(|segment| segment.bytes())
                .sum::<u128>()
    }
}

/// Closed shared vocabulary. Cloning shares payload and immutable lookup indexes;
/// no writer can append a term, text or predicate to this identity scope.
#[derive(Clone, Debug)]
pub(crate) struct FrozenVocabulary {
    pub(super) data: Arc<VocabularyData>,
    pub(super) indexes: Arc<Indexes>,
    bytes: u128,
    publication_peak: u128,
}
impl FrozenVocabulary {
    /// Named shared allocations, excluding the inline handle and Arc bookkeeping.
    pub(crate) const fn retained_bytes(&self) -> u128 {
        self.bytes
    }
    /// Actual freeze-attempt peak including caller metadata, independently of
    /// the writer's earlier import history. No rejected reservation is observed.
    pub(crate) const fn publication_peak(&self) -> u128 {
        self.publication_peak
    }
    /// Shared immutable payload, excluding the separate lookup indexes.
    pub(crate) fn payload_bytes(&self) -> u128 {
        self.data.retained_bytes()
    }
}

impl Store {
    /// Consume a vocabulary-only writer. Existing immutable prefixes stay valid;
    /// payload buffers, segment references and index allocations are moved once.
    /// `extra` is simultaneously held caller metadata, admitted before reserve.
    pub(crate) fn freeze_vocabulary_with<E>(
        mut self,
        extra: u128,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<FrozenVocabulary, Failure<E>> {
        let mut work = Work::new(&mut before);
        self.budget.peak = self.budget.used;
        work.step()?;
        self.budget.check()?;
        if self.tail.count() != 0 {
            return Err(Fault::VocabularyHasAtoms.into());
        }
        self.budget.check_extra(extra)?;
        if let Vocabulary::Frozen(mut base) = self.vocabulary {
            base.publication_peak = self.budget.used + extra;
            return Ok(base);
        }
        let envelopes = extra
            .checked_add(self.vocabulary.freeze_envelopes())
            .ok_or(Fault::Overflow)?;
        self.budget.check_extra(envelopes)?;
        let mut admission = self.budget.clone();
        admission.used += envelopes;
        self.vocabulary.prepare_freeze(&mut admission, &mut work)?;
        // All remaining envelopes are allocated below after the final permit.
        // The reserved directory's actual replacement overlap is already in peak.
        let publication_peak = admission.peak.max(admission.used);
        self.vocabulary
            .finish_freeze(publication_peak)
            .map_err(Failure::Storage)
    }
}

impl Vocabulary {
    pub(super) fn freeze_envelopes(&self) -> u128 {
        match self {
            Self::Frozen(_) => 0,
            Self::Growing(growing) => {
                size_of::<VocabularyData>() as u128
                    + size_of::<Indexes>() as u128
                    + if growing.tail.is_empty() {
                        0
                    } else {
                        size_of::<VocabularySegment>() as u128
                    }
            }
        }
    }

    /// Reserve the moved directory and admit all subsequent metadata visits.
    pub(super) fn prepare_freeze<E>(
        &mut self,
        admission: &mut super::Budget,
        work: &mut Work<'_, E>,
    ) -> Result<(), Failure<E>> {
        if let Self::Growing(growing) = self {
            let append = !growing.tail.is_empty();
            if append {
                work.reserve(&mut growing.sealed, 1, admission)?;
            }
            work.steps(
                growing
                    .sealed
                    .len()
                    .checked_add(usize::from(append))
                    .and_then(|count| count.checked_add(6))
                    .ok_or(Fault::Overflow)?,
            )?;
        }
        Ok(())
    }

    /// The caller reserved the directory and admitted every remaining move.
    /// Fixed Arc envelopes retain the established infallible allocation boundary.
    pub(super) fn finish_freeze(self, publication_peak: u128) -> Result<FrozenVocabulary, Fault> {
        let growing = match self {
            Self::Growing(growing) => growing,
            Self::Frozen(mut base) => {
                base.publication_peak = publication_peak;
                return Ok(base);
            }
        };
        let Growing {
            owner,
            mut sealed,
            tail,
            indexes,
        } = *growing;
        let counts = tail.counts();
        if !tail.is_empty() {
            sealed.push(Arc::new(tail));
        }
        let data = Arc::new(VocabularyData {
            owner,
            segments: sealed,
            counts,
            frozen: true,
        });
        let indexes = Arc::new(indexes);
        let bytes = data
            .retained_bytes()
            .checked_add(indexes.bytes())
            .ok_or(Fault::Overflow)?;
        Ok(FrozenVocabulary {
            data,
            indexes,
            bytes,
            publication_peak,
        })
    }
}
