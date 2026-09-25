//! Sparse complete candidates are instance-bound; results use canonical keys.

use crate::{Atom, Program, catalog::AtomRef};
use std::fmt;

mod selection;
pub use selection::{
    GateAtom, GateAtomError, GateAtoms, GateIndex, GateIndexError, GateIndexFailure, SeedAtom,
    SeedAtoms, SeedSelection, SeedSelectionError, SeedView,
};

/// A complete candidate represented only by its finite true atoms. Any atom
/// absent from this exact set is false; candidate construction never expands S.
#[derive(Clone, Debug)]
pub struct Seed {
    selection: SeedSelection,
}
impl Seed {
    /// Consume sparse true atoms and locate their coordinates in the Program's
    /// symbolic gate carrier. Accepted payloads are discarded; the result shares
    /// canonical Program values and retains only integer tuple coordinates.
    /// No full-carrier cardinality or enumeration is required. The input iterator
    /// must terminate. Coordinate and selection buffers reserve fallibly; Arc
    /// envelopes retain Rust's infallible allocation boundary.
    ///
    /// # Errors
    /// Refuses an outside-carrier atom or unavailable coordinate/selection storage.
    pub fn new(
        program: &Program,
        atoms: impl IntoIterator<Item = Atom>,
    ) -> Result<Self, SeedError> {
        SeedSelection::from_owned(program, atoms).map(|selection| Self { selection })
    }
    /// Exact true membership; absence is the complete candidate's false value.
    #[must_use]
    pub fn contains<'query>(&self, atom: impl Into<AtomRef<'query>>) -> bool {
        self.selection.view().contains(atom)
    }
    /// True atoms in semantic storage order, borrowing the canonical Program.
    #[must_use]
    pub fn atoms(&self) -> SeedAtoms<'_> {
        self.selection.view().atom_view()
    }
    /// The admitted instance whose gate carrier owns this seed.
    #[must_use]
    pub fn program(&self) -> &Program {
        self.selection.view().program()
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
    /// A validated symbolic carrier atom is missing from its complete graph.
    InvalidCarrierMapping,
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
            Self::InvalidCarrierMapping => {
                f.write_str("validated carrier atom is missing from the complete graph")
            }
            Self::WordCount { expected, actual } => {
                write!(f, "candidate requires {expected} words, received {actual}")
            }
            Self::Allocation => {
                f.write_str("candidate coordinate or selection storage could not be reserved")
            }
        }
    }
}
impl std::error::Error for SeedError {}
