//! Prepared per-predicate decisions over one vocabulary's accessible prefix.

use std::{fmt, mem::size_of};

use super::{CatalogRead, Error, PredicateRef, storage};

/// One prepared decision per predicate of a vocabulary's accessible prefix.
///
/// Bits denote predicate identities of one vocabulary owner, low bit first,
/// with zero tail padding; identity order has no semantic role. Owned ingress,
/// another vocabulary and a predicate admitted after preparation have no
/// decision here, so a caller keeps its general procedure for them.
#[derive(Debug)]
pub struct PredicateMask {
    scope: storage::VocabularyScope,
    len: usize,
    words: Vec<u64>,
}

/// Preparation stopped at the caller or could not reserve its words; no
/// partial mask escapes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PredicateMaskFailure<E> {
    /// The word vector could not be reserved.
    Storage(Error),
    /// The caller refused a decision.
    Stopped(E),
}

impl<E: fmt::Display> fmt::Display for PredicateMaskFailure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => error.fmt(f),
            Self::Stopped(error) => error.fmt(f),
        }
    }
}
impl<E: fmt::Debug + fmt::Display> std::error::Error for PredicateMaskFailure<E> {}

impl<'a> CatalogRead<'a> {
    /// Decide every predicate of this prefix once, in identity order.
    ///
    /// # Cost
    /// One `decide` call per admitted predicate; the mask retains one word per
    /// 64 predicates and a handle on the vocabulary's identity.
    ///
    /// # Errors
    /// Returns the first refusal of `decide`, or the word reservation's
    /// refusal, without a mask.
    ///
    /// # Panics
    /// Panics if an identity below the prefix count fails to resolve, which
    /// violates the canonical storage coverage invariant.
    pub fn predicate_mask_with<E>(
        self,
        mut decide: impl FnMut(PredicateRef<'a>) -> Result<bool, E>,
    ) -> Result<PredicateMask, PredicateMaskFailure<E>> {
        let ids = self.0.predicate_ids();
        let len = ids.len();
        let mut words = Vec::new();
        words
            .try_reserve_exact(len.div_ceil(64))
            .map_err(|_| PredicateMaskFailure::Storage(Error::Allocation))?;
        words.resize(len.div_ceil(64), 0);
        for id in ids {
            let predicate =
                PredicateRef::new(self.0, id).expect("an identity below the prefix count resolves");
            if decide(predicate).map_err(PredicateMaskFailure::Stopped)? {
                words[id.position() / 64] |= 1 << (id.position() % 64);
            }
        }
        Ok(PredicateMask {
            scope: self.0.vocabulary_scope(),
            len,
            words,
        })
    }
}

impl PredicateMask {
    /// The prepared decision for a canonical predicate of this vocabulary's
    /// prepared prefix, or `None` for any other predicate.
    #[must_use]
    pub fn decision(&self, predicate: PredicateRef<'_>) -> Option<bool> {
        let (read, id) = predicate.canonical()?;
        let position = id.position();
        (position < self.len && read.accepts_vocabulary_scope(&self.scope))
            .then(|| self.words[position / 64] & (1 << (position % 64)) != 0)
    }

    /// Number of predicates decided at preparation.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the prepared prefix had no predicates.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Mask header and word capacity; the vocabulary itself is shared.
    #[must_use]
    pub fn retained_bytes(&self) -> usize {
        size_of::<Self>() + self.words.capacity() * size_of::<u64>()
    }
}

#[cfg(test)]
mod tests;
