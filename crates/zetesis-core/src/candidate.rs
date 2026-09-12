//! Sparse complete candidates are instance-bound; results use canonical keys.

use crate::{Atom, Program};
use std::collections::BTreeSet;
use std::fmt;

mod selection;
pub use selection::{SeedSelection, SeedSelectionError, SeedView};

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

/// A canonical set of supplied atoms, also named [`Interpretation`]. Construction
/// establishes neither derivation, program satisfaction, nor stable membership.
/// Cloning copies the tree and owned atom/string payload; structured values share
/// their immutable node storage. No program or oracle is retained.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Model {
    atoms: BTreeSet<Atom>,
}
impl Model {
    /// Coalesce supplied atoms into canonical set order.
    /// The iterator must terminate. Construction owns distinct atoms and uses
    /// `O(n log n)` comparisons for `n` input atoms; comparisons inspect tuple and
    /// text/node payload. Tree allocation is infallible, not a typed refusal.
    #[must_use]
    pub fn new(atoms: impl IntoIterator<Item = Atom>) -> Self {
        Self {
            atoms: atoms.into_iter().collect(),
        }
    }
    /// All supplied atoms in canonical order. Borrowing is constant time;
    /// traversing the set visits each distinct atom without cloning it.
    #[must_use]
    pub fn atoms(&self) -> &BTreeSet<Atom> {
        &self.atoms
    }
    /// Exact set membership with a logarithmic number of atom comparisons.
    /// A comparison can inspect tuple and text/node payload.
    #[must_use]
    pub fn contains(&self, atom: &Atom) -> bool {
        self.atoms.contains(atom)
    }
}

/// A finite total truth assignment represented by its true atoms. Every absent
/// atom is false. This raw value carries no program-satisfaction or stability
/// guarantee; [`Model`] remains available as the original compatible name.
pub type Interpretation = Model;

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
            Self::Allocation => f.write_str("candidate word storage could not be reserved"),
        }
    }
}
impl std::error::Error for SeedError {}
