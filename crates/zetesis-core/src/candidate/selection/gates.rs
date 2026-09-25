//! Mint inseparable program, carrier-position and coordinate witnesses lazily.

use crate::{AtomIter, CarrierAtom, CarrierError, Program, catalog::AtomRef};
use std::iter::FusedIterator;
use std::num::NonZeroUsize;

mod index;
pub use index::{GateIndex, GateIndexError, GateIndexFailure};

/// An atom minted at its position in this program's canonical gate carrier.
/// Its private position and Program-bound coordinates cannot be independently
/// changed. Sharing the whole token preserves both without copying payload.
/// Both [`Program::indexed_gate_atoms`] and [`GateIndex::locate`] establish the
/// same position; the latter does not enumerate preceding tuples.
#[derive(Debug)]
pub struct GateAtom {
    pub(super) carrier: CarrierAtom,
    pub(super) position: NonZeroUsize,
}
impl GateAtom {
    /// Borrow the minted logical atom without changing its positional witness.
    #[must_use]
    pub fn atom(&self) -> AtomRef<'_> {
        self.carrier.atom()
    }

    /// The immutable admitted instance establishing this positional witness.
    #[must_use]
    pub fn program(&self) -> &Program {
        self.carrier.program()
    }

    /// Share the Program-bound tuple coordinates without its optional fast
    /// graph-position lookup. No logical payload or coordinate vector is copied.
    #[must_use]
    pub fn carrier(&self) -> CarrierAtom {
        self.carrier.clone()
    }
}

/// Failure to produce a complete indexed gate atom; never carrier exhaustion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateAtomError {
    /// Existing tuple-coordinate/value storage failure.
    Carrier(CarrierError),
    /// The next successful atom's positive position cannot fit usize.
    OrdinalOverflow,
}
impl std::fmt::Display for GateAtomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Carrier(error) => error.fmt(f),
            Self::OrdinalOverflow => f.write_str("gate-carrier position cannot be represented"),
        }
    }
}
impl std::error::Error for GateAtomError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Carrier(error) => Some(error),
            Self::OrdinalOverflow => None,
        }
    }
}

/// Lazy canonical gate atoms with checked, program-bound carrier positions.
/// No carrier tuple is requested at construction. Coordinate/value reservations
/// remain fallible; sharing a returned token is an explicit caller operation.
/// A carrier or position error is returned once and fuses the iterator.
pub struct GateAtoms<'a> {
    atoms: AtomIter<'a>,
    position: Option<NonZeroUsize>,
    failed: bool,
}
impl Program {
    /// Iterate opaque gate atoms in the same order as [`Self::gate_atoms`].
    /// Any complete graph of this immutable program has the same order in its
    /// gate-ID subsequence. Core keeps the checked position with its payload.
    #[must_use]
    pub fn indexed_gate_atoms(&self) -> GateAtoms<'_> {
        GateAtoms {
            atoms: self.gate_atoms(),
            position: NonZeroUsize::new(1),
            failed: false,
        }
    }
}
impl Iterator for GateAtoms<'_> {
    type Item = Result<GateAtom, GateAtomError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let atom = match self.atoms.next()? {
            Ok(atom) => atom,
            Err(error) => {
                self.failed = true;
                return Some(Err(GateAtomError::Carrier(error)));
            }
        };
        let Some(position) = self.position else {
            self.failed = true;
            return Some(Err(GateAtomError::OrdinalOverflow));
        };
        self.position = position.get().checked_add(1).and_then(NonZeroUsize::new);
        Some(Ok(GateAtom {
            carrier: atom,
            position,
        }))
    }
}
impl FusedIterator for GateAtoms<'_> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdmissionLimits, AtomPattern, Predicate, Template};

    fn program(count: usize) -> Program {
        Program::new(
            (0..count)
                .map(|index| {
                    let atom =
                        AtomPattern::new(Predicate::new(format!("p{index}"), 0).unwrap(), vec![])
                            .unwrap();
                    Template::new(Some(atom.clone()), vec![], vec![atom], vec![], vec![])
                })
                .collect(),
            AdmissionLimits::default(),
        )
        .unwrap()
    }

    #[test]
    fn position_overflow_is_explicit_and_fused() {
        let program = program(2);
        let mut atoms = program.indexed_gate_atoms();
        atoms.position = NonZeroUsize::new(usize::MAX);
        assert!(atoms.next().unwrap().is_ok());
        assert!(matches!(
            atoms.next(),
            Some(Err(GateAtomError::OrdinalOverflow))
        ));
        assert!(atoms.next().is_none());
        assert!(atoms.next().is_none());
    }

    #[test]
    fn exhausted_carrier_needs_no_next_position() {
        let program = program(1);
        let mut atoms = program.indexed_gate_atoms();
        atoms.position = NonZeroUsize::new(usize::MAX);
        assert!(atoms.next().unwrap().is_ok());
        assert!(atoms.next().is_none());
    }

    // Coordinate allocation is exercised at the Carrier construction boundary.
    // A private-corruption test cannot supply an impossible admitted predicate
    // through the real immutable Program vocabulary.
}
