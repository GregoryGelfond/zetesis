//! Sparse complete candidates are instance-bound; results use canonical keys.

use crate::{Atom, Program};
use std::collections::BTreeSet;
use std::fmt;

mod selection;
pub use selection::{
    GateAtom, GateAtomError, GateAtoms, SeedAtom, SeedSelection, SeedSelectionError, SeedView,
};

/// A complete candidate represented only by its finite true atoms. Any atom
/// absent from this exact set is false; candidate construction never expands S.
#[derive(Clone, Debug)]
pub struct Seed {
    program: Program,
    atoms: BTreeSet<Atom>,
}
impl Seed {
    /// Validate true atoms against the program's symbolic gate carrier.
    /// The input iterator must terminate. Construction consumes its atoms, checks
    /// signature/domain membership, and canonicalizes them in a tree set; it does
    /// not enumerate the carrier or ground the program. Comparisons can inspect
    /// atom/value payload. The program handle is shared; distinct atoms are owned.
    /// Tree-set allocation is infallible and is not a typed allocation refusal.
    ///
    /// # Errors
    /// Returns [`SeedError::OutsideCarrier`] for a foreign predicate or value.
    pub fn new(
        program: &Program,
        atoms: impl IntoIterator<Item = Atom>,
    ) -> Result<Self, SeedError> {
        let mut true_atoms = BTreeSet::new();
        for atom in atoms {
            if !program.contains_gate_atom(&atom) {
                return Err(SeedError::OutsideCarrier { atom });
            }
            true_atoms.insert(atom);
        }
        Ok(Self {
            program: program.clone(),
            atoms: true_atoms,
        })
    }
    /// Exact true membership; absence is the complete candidate's false value.
    #[must_use]
    pub fn contains(&self, atom: &Atom) -> bool {
        self.atoms.contains(atom)
    }
    /// True atoms in canonical order; no false tuples are stored.
    #[must_use]
    pub fn atoms(&self) -> &BTreeSet<Atom> {
        &self.atoms
    }
    /// The admitted instance whose gate carrier owns this seed.
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }
}

/// A candidate construction or representation refusal, never logical rejection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeedError {
    /// A true atom lies outside the instance's symbolic gate carrier.
    OutsideCarrier {
        /// The offending atom.
        atom: Atom,
    },
    /// The seed belongs to a separately admitted instance.
    WrongProgram,
    /// A core-minted position is missing from the graph's complete gate carrier.
    InvalidGatePosition,
    /// Caller-provided word storage has a different graph width.
    WordCount {
        /// Required complete interpretation words.
        expected: usize,
        /// Supplied output words.
        actual: usize,
    },
    /// A requested dense representation could not reserve its storage.
    Allocation,
}
impl fmt::Display for SeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideCarrier { atom } => {
                write!(f, "candidate atom lies outside the gate carrier: {atom:?}")
            }
            Self::WrongProgram => f.write_str("candidate belongs to a different program instance"),
            Self::InvalidGatePosition => {
                f.write_str("candidate gate position violates the complete carrier")
            }
            Self::WordCount { expected, actual } => {
                write!(f, "candidate requires {expected} words, received {actual}")
            }
            Self::Allocation => f.write_str("candidate word storage could not be reserved"),
        }
    }
}
impl std::error::Error for SeedError {}
