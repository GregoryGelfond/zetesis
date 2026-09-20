//! Checked ranks in the full symbolic gate carrier, without tuple enumeration.

use super::GateAtom;
use crate::{Atom, Program};
use std::num::NonZeroUsize;

/// Canonical gate-carrier positions for one immutable admitted program.
///
/// The index retains one shared program handle and one offset per gate
/// signature, not the Cartesian atoms. Construction takes O(p log(a + 1)) checked
/// arithmetic operations and O(p) words for p signatures of maximum arity a.
/// Looking up a supplied atom takes O(log(p + 1) + a log(d + 1)) typed comparisons for
/// domain size d; a comparison can inspect structured value payload.
/// The index establishes identity and carrier membership, not possible support
/// or answer-set membership. Its positions also apply to any complete graph of
/// the same program instance.
pub struct GateIndex {
    program: Program,
    offsets: Vec<usize>,
    cardinality: usize,
}

impl GateIndex {
    /// Index an admitted program's full gate carrier symbolically.
    ///
    /// Offset storage is reserved fallibly under the admitted signature count.
    /// No atom payload is copied and no carrier tuple is requested.
    ///
    /// # Errors
    /// Refuses unavailable offset storage or a full carrier cardinality that
    /// cannot fit usize. The latter is the same position boundary as exhausting
    /// [`Program::indexed_gate_atoms`], established without walking the tuples.
    pub fn new(program: &Program) -> Result<Self, GateIndexError> {
        let mut offsets = Vec::new();
        offsets
            .try_reserve_exact(program.gate_predicates().len())
            .map_err(|_| GateIndexError::Allocation)?;
        let mut cardinality = 0_usize;
        for predicate in program.gate_predicates() {
            offsets.push(cardinality);
            cardinality = cardinality
                .checked_add(tuple_count(program.domain().len(), predicate.arity())?)
                .ok_or(GateIndexError::OrdinalOverflow)?;
        }
        Ok(Self {
            program: program.clone(),
            offsets,
            cardinality,
        })
    }

    /// Exact number of atoms in the full symbolic gate carrier.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.cardinality
    }

    /// Whether the full symbolic gate carrier has no atom.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.cardinality == 0
    }

    /// Bind an owned logical atom to its original canonical carrier position.
    ///
    /// The returned token retains this index's program instance and moves the
    /// supplied payload without copying it. No allocation or enumeration occurs.
    /// Signature order is the admitted gate order. Within a signature,
    /// `AtomIter` advances the last coordinate fastest, so the mixed-radix fold
    /// over domain ranks is exactly that tuple's zero-based position. Adding the
    /// preceding signatures' cardinalities and one gives the full gate position;
    /// filtering to a supported subset never renumbers it. Nullary signatures
    /// have one empty tuple, including when the domain is empty.
    ///
    /// # Errors
    /// Refuses a signature or value outside this program's gate carrier, or
    /// checked position arithmetic that cannot fit usize.
    pub fn locate(&self, atom: Atom) -> Result<GateAtom, GateIndexError> {
        let signature = self
            .program
            .gate_predicates()
            .binary_search(atom.predicate())
            .map_err(|_| GateIndexError::OutsideCarrier)?;
        let domain = self.program.domain();
        let mut tuple = 0_usize;
        for value in atom.values() {
            let digit = domain
                .binary_search(value)
                .map_err(|_| GateIndexError::OutsideCarrier)?;
            tuple = tuple
                .checked_mul(domain.len())
                .and_then(|rank| rank.checked_add(digit))
                .ok_or(GateIndexError::OrdinalOverflow)?;
        }
        let position = self.offsets[signature]
            .checked_add(tuple)
            .and_then(|rank| rank.checked_add(1))
            .and_then(NonZeroUsize::new)
            .ok_or(GateIndexError::OrdinalOverflow)?;
        Ok(GateAtom {
            program: self.program.clone(),
            position,
            atom,
        })
    }
}

fn tuple_count(radix: usize, arity: usize) -> Result<usize, GateIndexError> {
    match (radix, arity) {
        (_, 0) | (1, _) => Ok(1),
        (0, _) => Ok(0),
        _ => u32::try_from(arity)
            .ok()
            .and_then(|exponent| radix.checked_pow(exponent))
            .ok_or(GateIndexError::OrdinalOverflow),
    }
}

/// Symbolic gate indexing failed before a positional token was returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateIndexError {
    /// The per-signature offset vector could not reserve its storage.
    Allocation,
    /// The supplied atom's signature or a value is outside the gate carrier.
    OutsideCarrier,
    /// The full carrier cardinality or a positive position cannot fit usize.
    OrdinalOverflow,
}

impl std::fmt::Display for GateIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Allocation => "gate-position index storage could not be reserved",
            Self::OutsideCarrier => "atom lies outside the program's gate carrier",
            Self::OrdinalOverflow => "gate-carrier position cannot be represented",
        })
    }
}

impl std::error::Error for GateIndexError {}
