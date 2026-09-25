//! Signed-signature selection over the bundle's canonical metadata vocabulary.

use std::sync::Arc;

use super::{
    Admission, MetadataStorageError, MetadataStorageLimits, MetadataVocabulary, Predicate, Read,
};
use zetesis_core::{
    Predicate as OwnedPredicate,
    catalog::{AtomRef, PredicateRef},
};

/// Borrowed, semantically ordered predicate signatures. Coordinates remain
/// private to the immutable metadata owner; iteration copies only read views.
#[derive(Clone, Copy)]
pub struct Signatures<'a> {
    read: Option<Read<'a>>,
    positions: &'a [Predicate],
}
impl<'a> Signatures<'a> {
    /// Number of distinct signatures.
    #[must_use]
    pub fn len(self) -> usize {
        self.positions.len()
    }
    /// Whether the selection has no signatures.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.positions.is_empty()
    }
    /// Read a signature by its semantic-order position.
    #[must_use]
    pub fn at(self, position: usize) -> Option<PredicateRef<'a>> {
        self.read?.predicate(*self.positions.get(position)?)
    }
    /// Iterate in signed-predicate identity order, without materializing names.
    ///
    /// # Panics
    /// Panics if an internal publication invariant is broken: every selected
    /// predicate must belong to the paired immutable vocabulary prefix.
    #[must_use]
    pub fn iter(self) -> impl ExactSizeIterator<Item = PredicateRef<'a>> + DoubleEndedIterator {
        self.positions.iter().map(move |position| {
            self.read
                .expect("nonempty published signatures have a vocabulary")
                .predicate(*position)
                .expect("published predicate occurrence belongs to the paired prefix")
        })
    }
}
impl std::fmt::Debug for Signatures<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
impl PartialEq for Signatures<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
impl Eq for Signatures<'_> {}

#[derive(Clone, Debug, Default)]
pub(super) struct SignatureSet {
    vocabulary: Option<Arc<MetadataVocabulary>>,
    positions: Vec<Predicate>,
}
impl SignatureSet {
    pub(super) fn publish(
        vocabulary: Option<Arc<MetadataVocabulary>>,
        mut positions: Vec<Predicate>,
    ) -> Self {
        if let Some(owner) = vocabulary.as_ref() {
            let read = owner
                .read_with(|| Ok::<_, std::convert::Infallible>(()))
                .expect("publication checked component prefix");
            positions.sort_unstable_by(|a, b| {
                read.predicate(*a)
                    .expect("admitted predicate")
                    .cmp(&read.predicate(*b).expect("admitted predicate"))
            });
            positions.dedup_by(|a, b| read.predicate(*a) == read.predicate(*b));
        } else {
            assert!(
                positions.is_empty(),
                "nonempty coordinates require their authority"
            );
        }
        Self {
            vocabulary,
            positions,
        }
    }
    pub(super) fn view(&self) -> Signatures<'_> {
        Signatures {
            read: self.vocabulary.as_ref().map(|owner| {
                owner
                    .read_with(|| Ok::<_, std::convert::Infallible>(()))
                    .expect("immutable published metadata retains its checked prefix")
            }),
            positions: &self.positions,
        }
    }
}
impl PartialEq for SignatureSet {
    fn eq(&self, other: &Self) -> bool {
        self.view() == other.view()
    }
}
impl Eq for SignatureSet {}

/// Atom-channel selection only. Term observations are independent; display
/// equality never changes underlying stable-model identity or model counts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutputSelection {
    explicit: bool,
    signatures: SignatureSet,
}
/// Atom-channel name retained alongside `OutputSelection`.
pub type AtomSelection = OutputSelection;

#[derive(Default)]
pub(super) struct Builder {
    explicit: bool,
    signatures: Vec<Predicate>,
}
impl Builder {
    pub(super) fn mark_explicit(&mut self) {
        self.explicit = true;
    }
    pub(super) fn include(&mut self, signature: Predicate) {
        self.mark_explicit();
        self.signatures.push(signature);
    }
    pub(super) fn finish(self, vocabulary: Option<Arc<MetadataVocabulary>>) -> OutputSelection {
        OutputSelection {
            explicit: self.explicit,
            signatures: SignatureSet::publish(vocabulary, self.signatures),
        }
    }
}

/// Independent admission limits for explicit signature input and its canonical owner.
#[derive(Clone, Copy, Debug)]
pub struct AtomSelectionLimits {
    /// Input occurrences, including duplicates.
    pub max_signatures: usize,
    /// Cumulative input UTF-8 name lengths, including duplicates.
    pub max_name_bytes: usize,
    /// Named canonical vocabulary/component capacities and publication overlap.
    pub storage: MetadataStorageLimits,
}
impl Default for AtomSelectionLimits {
    fn default() -> Self {
        Self {
            max_signatures: 1_024,
            max_name_bytes: 1_048_576,
            storage: MetadataStorageLimits::default(),
        }
    }
}
/// Explicit atom selection admission failed; no partial policy is returned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AtomSelectionError {
    /// Too many input occurrences.
    Signatures {
        /// Inclusive ceiling.
        limit: usize,
        /// Required count.
        observed: usize,
    },
    /// Too many cumulative input name bytes.
    NameBytes {
        /// Inclusive ceiling.
        limit: usize,
        /// Required count.
        observed: u128,
    },
    /// Canonical payload/component admission or publication refused.
    Storage(MetadataStorageError),
}
impl std::fmt::Display for AtomSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "atom selection refused: {self:?}")
    }
}
impl std::error::Error for AtomSelectionError {}
impl OutputSelection {
    /// Select all original atoms without allocating.
    #[must_use]
    pub fn all() -> Self {
        Self::default()
    }
    /// Select no original atoms without changing term observations.
    #[must_use]
    pub fn none() -> Self {
        Self {
            explicit: true,
            ..Self::default()
        }
    }
    /// Admit and select the semantic union of signed signatures. Input occurrence
    /// and text limits apply before canonical admission, even for duplicates.
    ///
    /// # Errors
    /// Returns an inclusive count/text or typed canonical storage refusal.
    pub fn from_signatures(
        signatures: &[OwnedPredicate],
        limits: AtomSelectionLimits,
    ) -> Result<Self, AtomSelectionError> {
        if signatures.len() > limits.max_signatures {
            return Err(AtomSelectionError::Signatures {
                limit: limits.max_signatures,
                observed: signatures.len(),
            });
        }
        let mut bytes = 0u128;
        for signature in signatures {
            bytes += signature.name().len() as u128;
            if bytes > limits.max_name_bytes as u128 {
                return Err(AtomSelectionError::NameBytes {
                    limit: limits.max_name_bytes,
                    observed: bytes,
                });
            }
        }
        if signatures.is_empty() {
            return Ok(Self::none());
        }
        let mut authority =
            Admission::new(limits.storage, 0).map_err(AtomSelectionError::Storage)?;
        let mut builder = Builder::default();
        for signature in signatures {
            builder.include(
                authority
                    .predicate(signature.into(), 0)
                    .map_err(AtomSelectionError::Storage)?,
            );
        }
        Ok(builder.finish(Some(
            authority.finish(0).map_err(AtomSelectionError::Storage)?,
        )))
    }
    /// Whether explicit signature or empty-show selection was authored.
    #[must_use]
    pub fn is_explicit(&self) -> bool {
        self.explicit
    }
    /// Distinct signed signatures in semantic order, never coordinate order.
    #[must_use]
    pub fn signatures(&self) -> Signatures<'_> {
        self.signatures.view()
    }
    /// Whether an atom is displayed; this never prunes logical candidates.
    #[must_use]
    pub fn includes<'a>(&self, atom: impl Into<AtomRef<'a>>) -> bool {
        match self.try_includes(atom, |_| Ok::<_, std::convert::Infallible>(())) {
            Ok(value) => value,
            Err(impossible) => match impossible {},
        }
    }
    /// Charge before each binary-search comparison: one plus both UTF-8 name
    /// lengths, exactly as for the previous owned input representation. Implicit
    /// or explicit-empty policies require no probes.
    ///
    /// # Errors
    /// Returns the first caller refusal, without a selection result.
    ///
    /// # Panics
    /// Panics if a selected predicate is absent from its published vocabulary,
    /// which violates the selection's internal publication invariant.
    pub fn try_includes<'a, E>(
        &self,
        atom: impl Into<AtomRef<'a>>,
        mut charge: impl FnMut(u128) -> Result<(), E>,
    ) -> Result<bool, E> {
        if !self.explicit {
            return Ok(true);
        }
        let predicate = atom.into().predicate();
        let signatures = self.signatures();
        let (mut start, mut end) = (0, signatures.len());
        while start < end {
            let middle = start + (end - start) / 2;
            let signature = signatures.at(middle).expect("bounded signature position");
            charge(1 + predicate.name().len() as u128 + signature.name().len() as u128)?;
            match predicate.cmp(&signature) {
                std::cmp::Ordering::Less => end = middle,
                std::cmp::Ordering::Equal => return Ok(true),
                std::cmp::Ordering::Greater => start = middle + 1,
            }
        }
        Ok(false)
    }
}
