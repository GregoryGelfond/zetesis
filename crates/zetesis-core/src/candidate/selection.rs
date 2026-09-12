//! Shared true-atom ownership with an allocation-free candidate view.

use std::collections::{BTreeSet, btree_set};
use std::iter::FusedIterator;
use std::sync::Arc;

use super::Seed;
use crate::{Atom, AtomKey, Program};

/// A complete candidate whose canonical true atoms share immutable payloads.
/// Missing atoms are false. This establishes carrier membership and program
/// identity, not satisfaction, reduct closure or stable membership.
/// Cloning copies selected handles, not atom payloads; its vector allocation is
/// infallible. Use [`Self::to_seed`] for explicit owned tree materialization.
#[derive(Clone, Debug)]
pub struct SeedSelection {
    program: Program,
    atoms: Vec<Arc<Atom>>,
}

impl SeedSelection {
    /// Validate and canonicalize existing shared true-atom handles.
    /// The iterator must terminate. Each input is checked against the symbolic
    /// gate carrier; construction never expands that carrier or grounds source.
    /// Vector growth is fallible; sorting/deduplication moves handles without
    /// cloning atom payloads. Comparisons inspect typed predicate/value payload.
    /// The caller owns any allocation used to create the supplied Arc handles.
    ///
    /// # Errors
    /// Refuses an outside-carrier atom or unavailable selection-vector storage.
    pub fn new(
        program: &Program,
        atoms: impl IntoIterator<Item = Arc<Atom>>,
    ) -> Result<Self, SeedSelectionError> {
        let mut selected = Vec::new();
        for atom in atoms {
            if !program.contains_gate_atom(&atom) {
                return Err(SeedSelectionError::OutsideCarrier { atom });
            }
            selected
                .try_reserve(1)
                .map_err(|_| SeedSelectionError::Allocation)?;
            selected.push(atom);
        }
        selected.sort_unstable();
        selected.dedup();
        Ok(Self {
            program: program.clone(),
            atoms: selected,
        })
    }

    /// Borrow exact true membership and canonical iteration without allocation.
    #[must_use]
    pub fn view(&self) -> SeedView<'_> {
        SeedView {
            program: &self.program,
            atoms: Atoms::Selected(&self.atoms),
        }
    }

    /// Materialize the compatible owned Seed explicitly. This clones each true
    /// atom's payload and constructs a canonical tree using infallible allocation.
    /// The already validated program identity and true set are unchanged.
    #[must_use]
    pub fn to_seed(&self) -> Seed {
        Seed {
            program: self.program.clone(),
            atoms: self.view().atoms().cloned().collect(),
        }
    }
}

impl Seed {
    /// Borrow this exact candidate without copying its tree or atom payloads.
    #[must_use]
    pub fn view(&self) -> SeedView<'_> {
        SeedView {
            program: self.program(),
            atoms: Atoms::Owned(self.atoms()),
        }
    }
}

/// Shared selection construction failed before any candidate was returned.
/// The refused handle remains shared; even this error copies no atom payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeedSelectionError {
    /// The supplied atom is outside the selected program's gate carrier.
    OutsideCarrier {
        /// The exact offending immutable shared atom.
        atom: Arc<Atom>,
    },
    /// The selection's handle vector could not reserve its storage.
    Allocation,
}

impl std::fmt::Display for SeedSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutsideCarrier { atom } => {
                write!(f, "candidate atom lies outside the gate carrier: {atom:?}")
            }
            Self::Allocation => f.write_str("candidate selection storage could not be reserved"),
        }
    }
}
impl std::error::Error for SeedSelectionError {}

/// An allocation-free borrow of one instance-bound complete candidate.
/// Both owned Seeds and shared selections have the same exact true-set contract.
/// A view cannot outlive its owner; it never materializes a tree or copies atoms.
#[derive(Clone, Copy, Debug)]
pub struct SeedView<'a> {
    program: &'a Program,
    atoms: Atoms<'a>,
}

#[derive(Clone, Copy, Debug)]
enum Atoms<'a> {
    Owned(&'a BTreeSet<Atom>),
    Selected(&'a [Arc<Atom>]),
}

impl<'a> SeedView<'a> {
    /// The immutable program instance owning the candidate's gate carrier.
    #[must_use]
    pub const fn program(self) -> &'a Program {
        self.program
    }

    /// True atoms in canonical storage order, with no cloning or allocation.
    #[must_use]
    pub fn atoms(
        self,
    ) -> impl ExactSizeIterator<Item = &'a Atom> + DoubleEndedIterator + FusedIterator + Clone {
        match self.atoms {
            Atoms::Owned(atoms) => TrueAtoms::Owned(atoms.iter()),
            Atoms::Selected(atoms) => TrueAtoms::Selected(atoms.iter()),
        }
    }

    /// Exact true membership; comparisons inspect canonical typed atom identity.
    #[must_use]
    pub fn contains(self, atom: &Atom) -> bool {
        match self.atoms {
            Atoms::Owned(atoms) => atoms.contains(atom),
            Atoms::Selected(atoms) => atoms
                .binary_search_by(|stored| stored.as_ref().cmp(atom))
                .is_ok(),
        }
    }

    /// Check a complete borrowed substitution without constructing an Atom.
    /// The key's sign, arity and every typed argument participate in identity.
    #[must_use]
    pub fn contains_key(self, key: &AtomKey<'_>) -> bool {
        match self.atoms {
            Atoms::Owned(atoms) => key.get(atoms).is_some(),
            Atoms::Selected(atoms) => atoms
                .binary_search_by(|stored| key.compare(stored).reverse())
                .is_ok(),
        }
    }
}

#[derive(Clone)]
enum TrueAtoms<'a> {
    Owned(btree_set::Iter<'a, Atom>),
    Selected(std::slice::Iter<'a, Arc<Atom>>),
}

impl<'a> Iterator for TrueAtoms<'a> {
    type Item = &'a Atom;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Owned(atoms) => atoms.next(),
            Self::Selected(atoms) => atoms.next().map(Arc::as_ref),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}
impl DoubleEndedIterator for TrueAtoms<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::Owned(atoms) => atoms.next_back(),
            Self::Selected(atoms) => atoms.next_back().map(Arc::as_ref),
        }
    }
}
impl ExactSizeIterator for TrueAtoms<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Owned(atoms) => atoms.len(),
            Self::Selected(atoms) => atoms.len(),
        }
    }
}
impl FusedIterator for TrueAtoms<'_> {}
