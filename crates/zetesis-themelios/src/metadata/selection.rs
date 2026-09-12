//! Canonical signed-signature selection with one charged membership operation.

use zetesis_core::{Atom, Predicate};

/// Atom-channel selection only. Term observations are independent. Models that display the same atoms remain distinct
/// underlying stable models and must be counted/enumerated independently.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutputSelection {
    explicit: bool,
    signatures: Vec<Predicate>,
}

/// Truthful atom-channel name; `OutputSelection` remains the compatibility name.
pub type AtomSelection = OutputSelection;

/// Source collection owns an unordered occurrence buffer. Only `finish`
/// publishes the immutable canonical selection used by membership queries.
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

    pub(super) fn finish(mut self) -> OutputSelection {
        self.signatures.sort_unstable();
        self.signatures.dedup();
        OutputSelection {
            explicit: self.explicit,
            signatures: self.signatures,
        }
    }
}

/// Inclusive admission limits for an explicit signed-signature slice.
#[derive(Clone, Copy, Debug)]
pub struct AtomSelectionLimits {
    /// Input occurrences, including duplicates, before set construction.
    pub max_signatures: usize,
    /// Sum of input UTF-8 predicate-name lengths, including duplicates.
    /// The resulting set holds at most this text plus one Predicate per input;
    /// allocator overhead and vector bookkeeping are excluded.
    pub max_name_bytes: usize,
}
impl Default for AtomSelectionLimits {
    fn default() -> Self {
        Self {
            max_signatures: 1_024,
            max_name_bytes: 1_048_576,
        }
    }
}

/// Explicit atom selection was refused before cloning any signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomSelectionError {
    /// Too many input occurrences.
    Signatures {
        /// Inclusive ceiling.
        limit: usize,
        /// Required count.
        observed: usize,
    },
    /// Cumulative predicate-name text exceeds its inclusive ceiling.
    NameBytes {
        /// Inclusive ceiling.
        limit: usize,
        /// Required byte count.
        observed: u128,
    },
}
impl std::fmt::Display for AtomSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "atom selection refused: {self:?}")
    }
}
impl std::error::Error for AtomSelectionError {}

impl OutputSelection {
    /// Select all original atoms. No allocation or observation evaluation occurs.
    #[must_use]
    pub fn all() -> Self {
        Self::default()
    }
    /// Select no original atoms. Term observations remain independent.
    #[must_use]
    pub fn none() -> Self {
        Self {
            explicit: true,
            signatures: Vec::new(),
        }
    }
    /// Select the union of the supplied signed predicate signatures.
    /// Input occurrences and text are checked before cloning and sorting; equal signatures
    /// deduplicate without changing the limits charged for their input. An empty
    /// slice selects no atoms. Time is O(N log N) predicate comparisons, including
    /// name bytes; retained text/cells are bounded by the admitted slice.
    ///
    /// # Errors
    /// Returns a typed inclusive count/text refusal before constructing the set.
    pub fn from_signatures(
        signatures: &[Predicate],
        limits: AtomSelectionLimits,
    ) -> Result<Self, AtomSelectionError> {
        if signatures.len() > limits.max_signatures {
            return Err(AtomSelectionError::Signatures {
                limit: limits.max_signatures,
                observed: signatures.len(),
            });
        }
        let mut bytes = 0_u128;
        for signature in signatures {
            bytes += signature.name().len() as u128;
            if bytes > limits.max_name_bytes as u128 {
                return Err(AtomSelectionError::NameBytes {
                    limit: limits.max_name_bytes,
                    observed: bytes,
                });
            }
        }
        let mut signatures = signatures.to_vec();
        signatures.sort_unstable();
        signatures.dedup();
        Ok(Self {
            explicit: true,
            signatures,
        })
    }

    /// Whether a signature or empty `#show` directive was present. When false,
    /// every atom is displayed; when true, only the selected signatures are.
    #[must_use]
    pub fn is_explicit(&self) -> bool {
        self.explicit
    }

    /// The union of signed predicate signatures selected explicitly. An empty
    /// slice displays no atoms when explicit, and all atoms otherwise. The slice
    /// is strictly ordered by signed predicate identity and contains no duplicates.
    #[must_use]
    pub fn signatures(&self) -> &[Predicate] {
        &self.signatures
    }
    /// Whether an atom is selected for display. This must not be used to prune
    /// candidates, reduct closure, stable-model identity, or model counts.
    #[must_use]
    pub fn includes(&self, atom: &Atom) -> bool {
        match self.try_includes(atom, |_| Ok::<(), std::convert::Infallible>(())) {
            Ok(selected) => selected,
            Err(impossible) => match impossible {},
        }
    }

    /// Whether an atom is selected, charging before each signature comparison.
    ///
    /// Shares the exact membership operation with [`Self::includes`]. A probe
    /// charges one unit plus both UTF-8 predicate-name lengths, a conservative
    /// bound on compared text. Sorted signatures require at most
    /// `floor(log2(S)) + 1` probes for nonempty `S`; implicit and empty explicit
    /// selections require no probes. No allocation or atom cloning occurs.
    ///
    /// # Errors
    /// Returns the first error from `charge` before performing that comparison.
    /// A refused lookup supplies no selection result.
    pub fn try_includes<E>(
        &self,
        atom: &Atom,
        mut charge: impl FnMut(u128) -> Result<(), E>,
    ) -> Result<bool, E> {
        if !self.explicit {
            return Ok(true);
        }
        let predicate = atom.predicate();
        let mut remaining = self.signatures.as_slice();
        while !remaining.is_empty() {
            let middle = remaining.len() / 2;
            let signature = &remaining[middle];
            charge(1 + predicate.name().len() as u128 + signature.name().len() as u128)?;
            match predicate.cmp(signature) {
                std::cmp::Ordering::Less => remaining = &remaining[..middle],
                std::cmp::Ordering::Equal => return Ok(true),
                std::cmp::Ordering::Greater => remaining = &remaining[middle + 1..],
            }
        }
        Ok(false)
    }
}
