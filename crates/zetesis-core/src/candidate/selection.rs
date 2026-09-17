//! Shared true-atom ownership with an allocation-free candidate view.

use std::collections::{BTreeSet, btree_set};
use std::iter::FusedIterator;
use std::num::NonZeroUsize;
use std::sync::Arc;

use super::Seed;
use crate::{Atom, AtomId, AtomKey, GroundProgram, Program, SeedError};

mod gates;
pub use gates::{GateAtom, GateAtomError, GateAtoms};

#[cfg(test)]
mod tests;

// One selected owner, retaining either an arbitrary shared atom or the entire
// core-minted positional witness. Neither variant recopies the atom payload.
#[derive(Clone, Debug)]
enum Entry {
    Manual(Arc<Atom>),
    Indexed(Arc<GateAtom>),
}
impl Entry {
    fn atom(&self) -> &Atom {
        match self {
            Self::Manual(atom) => atom,
            Self::Indexed(atom) => atom.atom(),
        }
    }
    fn position(&self) -> Option<NonZeroUsize> {
        match self {
            Self::Manual(_) => None,
            Self::Indexed(atom) => Some(atom.position),
        }
    }
}

/// A complete candidate whose canonical true atoms share immutable payloads.
/// Missing atoms are false. This establishes carrier membership and program
/// identity, not satisfaction, reduct closure or stable membership.
/// Cloning copies selected handles, not atom payloads; its vector allocation is
/// infallible. Use [`Self::to_seed`] for explicit owned tree materialization.
#[derive(Clone, Debug)]
pub struct SeedSelection {
    program: Program,
    atoms: Vec<Entry>,
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
            selected.push(Entry::Manual(atom));
        }
        selected.sort_unstable_by(|left, right| left.atom().cmp(right.atom()));
        selected.dedup_by(|left, right| left.atom() == right.atom());
        Ok(Self {
            program: program.clone(),
            atoms: selected,
        })
    }

    /// Retain core-minted gate atoms from this same admitted program instance.
    /// Their private checked positions survive sorting and deduplication with
    /// their original payloads. Integer position order is the canonical atom
    /// order for this carrier; no payload copying or carrier expansion occurs.
    /// Vector growth is fallible. The caller owns the Arc allocation for supplied
    /// tokens; cloning those handles shares their program and atom payload.
    ///
    /// # Errors
    /// Refuses foreign program identity or unavailable selection storage.
    pub fn from_gate_atoms(
        program: &Program,
        atoms: impl IntoIterator<Item = Arc<GateAtom>>,
    ) -> Result<Self, SeedSelectionError> {
        let mut selected = Vec::new();
        for atom in atoms {
            if !program.same_instance(&atom.program) {
                return Err(SeedSelectionError::WrongProgram);
            }
            selected
                .try_reserve(1)
                .map_err(|_| SeedSelectionError::Allocation)?;
            selected.push(Entry::Indexed(atom));
        }
        selected.sort_unstable_by_key(Entry::position);
        selected.dedup_by_key(|entry| entry.position());
        Ok(Self {
            program: program.clone(),
            atoms: selected,
        })
    }

    /// Retain gate atoms every candidate of an enumeration holds beside the
    /// core-minted atoms the counter selected. The necessary atoms are checked
    /// against the symbolic gate carrier and the minted ones against program
    /// identity; both sort in the canonical atom order, which is also the
    /// minted position order, and a payload present in both is kept once.
    /// Vector growth is fallible; no payload is copied.
    ///
    /// # Errors
    /// Refuses an outside-carrier necessary atom, foreign program identity or
    /// unavailable selection storage.
    pub fn necessary_and_selected(
        program: &Program,
        necessary: impl IntoIterator<Item = Arc<Atom>>,
        selected: impl IntoIterator<Item = Arc<GateAtom>>,
    ) -> Result<Self, SeedSelectionError> {
        let mut entries = Vec::new();
        for atom in necessary {
            if !program.contains_gate_atom(&atom) {
                return Err(SeedSelectionError::OutsideCarrier { atom });
            }
            entries
                .try_reserve(1)
                .map_err(|_| SeedSelectionError::Allocation)?;
            entries.push(Entry::Manual(atom));
        }
        for atom in selected {
            if !program.same_instance(&atom.program) {
                return Err(SeedSelectionError::WrongProgram);
            }
            entries
                .try_reserve(1)
                .map_err(|_| SeedSelectionError::Allocation)?;
            entries.push(Entry::Indexed(atom));
        }
        entries.sort_unstable_by(|left, right| left.atom().cmp(right.atom()));
        entries.dedup_by(|left, right| left.atom() == right.atom());
        Ok(Self {
            program: program.clone(),
            atoms: entries,
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
/// `OutsideCarrier` retains the offending shared atom; constructing these errors
/// does not clone atom payloads. Other failures may drop consumed handles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeedSelectionError {
    /// A core-minted gate atom belongs to a separately admitted program.
    WrongProgram,
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
            Self::WrongProgram => f.write_str("gate atom belongs to a different program instance"),
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
    Selected(&'a [Entry]),
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
        self.entries().map(SeedAtom::atom)
    }

    /// Exact true membership; comparisons inspect canonical typed atom identity.
    #[must_use]
    pub fn contains(self, atom: &Atom) -> bool {
        match self.atoms {
            Atoms::Owned(atoms) => atoms.contains(atom),
            Atoms::Selected(atoms) => atoms
                .binary_search_by(|stored| stored.atom().cmp(atom))
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
                .binary_search_by(|stored| key.compare(stored.atom()).reverse())
                .is_ok(),
        }
    }

    /// Borrow complete atom entries, retaining any core-minted carrier position.
    /// Resolving an entry remains explicit, so consumers can poll and charge
    /// before each lookup. This iterator itself performs no graph lookup.
    #[must_use]
    pub fn entries(
        self,
    ) -> impl ExactSizeIterator<Item = SeedAtom<'a>> + DoubleEndedIterator + FusedIterator + Clone
    {
        let program = self.program;
        match self.atoms {
            Atoms::Owned(atoms) => Entries::Owned(atoms.iter()),
            Atoms::Selected(atoms) => Entries::Selected(atoms.iter()),
        }
        .map(move |(atom, position)| SeedAtom {
            program,
            atom,
            position,
        })
    }
}

/// A borrowed true atom and optional inseparable gate-carrier position.
/// Fields are private; callers cannot associate an index with another payload.
#[derive(Clone, Copy, Debug)]
pub struct SeedAtom<'a> {
    program: &'a Program,
    atom: &'a Atom,
    position: Option<NonZeroUsize>,
}

impl<'a> SeedAtom<'a> {
    /// The unchanged logical atom; borrowing does not resolve or copy it.
    #[must_use]
    pub const fn atom(self) -> &'a Atom {
        self.atom
    }

    /// Resolve this atom in a complete graph of the same immutable program.
    /// Core-minted entries use one gate-ID array lookup, without payload hashing
    /// or comparison. Manual entries use the graph's canonical binary lookup.
    ///
    /// # Errors
    /// Refuses foreign program identity first. An invalid indexed position is
    /// an explicit invariant failure and never falls back to symbolic lookup.
    pub fn resolve_in(self, graph: &GroundProgram) -> Result<AtomId, SeedError> {
        self.resolve_with(graph, |atom| graph.atom_id(atom))
    }

    fn resolve_with(
        self,
        graph: &GroundProgram,
        symbolic: impl FnOnce(&Atom) -> Option<AtomId>,
    ) -> Result<AtomId, SeedError> {
        if !self.program.same_instance(graph.program()) {
            return Err(SeedError::WrongProgram);
        }
        match self.position {
            Some(position) => graph
                .gate_atom_ids()
                .get(position.get() - 1)
                .copied()
                .ok_or(SeedError::InvalidGatePosition),
            None => symbolic(self.atom).ok_or_else(|| SeedError::OutsideCarrier {
                atom: self.atom.clone(),
            }),
        }
    }
}

#[derive(Clone)]
enum Entries<'a> {
    Owned(btree_set::Iter<'a, Atom>),
    Selected(std::slice::Iter<'a, Entry>),
}
impl<'a> Iterator for Entries<'a> {
    type Item = (&'a Atom, Option<NonZeroUsize>);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Owned(atoms) => atoms.next().map(|atom| (atom, None)),
            Self::Selected(atoms) => atoms.next().map(|entry| (entry.atom(), entry.position())),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}
impl ExactSizeIterator for Entries<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Owned(atoms) => atoms.len(),
            Self::Selected(atoms) => atoms.len(),
        }
    }
}

impl DoubleEndedIterator for Entries<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::Owned(atoms) => atoms.next_back().map(|atom| (atom, None)),
            Self::Selected(atoms) => atoms
                .next_back()
                .map(|entry| (entry.atom(), entry.position())),
        }
    }
}
impl FusedIterator for Entries<'_> {}
