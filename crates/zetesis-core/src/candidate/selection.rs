//! One immutable sparse coordinate selection for owned and shared candidates.

use std::{fmt, iter::FusedIterator, num::NonZeroUsize, slice, sync::Arc};

use super::Seed;
use crate::catalog::AtomRef;
use crate::{Atom, AtomId, AtomKey, CarrierAtom, GroundProgram, Program, SeedError};

mod gates;
pub use gates::{GateAtom, GateAtomError, GateAtoms, GateIndex, GateIndexError, GateIndexFailure};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
enum Entry {
    Manual(CarrierAtom),
    Indexed(Arc<GateAtom>),
}
impl Entry {
    fn carrier(&self) -> &CarrierAtom {
        match self {
            Self::Manual(atom) => atom,
            Self::Indexed(atom) => &atom.carrier,
        }
    }
    fn atom(&self) -> AtomRef<'_> {
        self.carrier().atom()
    }
    fn position(&self) -> Option<NonZeroUsize> {
        match self {
            Self::Manual(_) => None,
            Self::Indexed(atom) => Some(atom.position),
        }
    }
}

/// A complete candidate retaining sparse coordinates in one immutable Program.
/// Missing atoms are false. This establishes carrier membership and applicability,
/// not satisfaction, reduct closure or stable membership. Cloning shares both the
/// selection and its coordinate allocations; it copies no atom/value payload.
#[derive(Clone, Debug)]
pub struct SeedSelection(Arc<SelectionData>);
#[derive(Debug)]
struct SelectionData {
    program: Program,
    atoms: Vec<Entry>,
}

impl SeedSelection {
    /// Consume shared ingress atoms into Program-bound coordinate tokens.
    /// Accepted ingress payloads are not retained. The input iterator must
    /// terminate; construction neither counts nor expands the whole carrier.
    /// Vector/coordinate reservations are fallible. Arc envelopes use Rust's
    /// infallible allocation boundary.
    ///
    /// # Errors
    /// Refuses outside-carrier input or unavailable coordinate/selection storage.
    pub fn new(
        program: &Program,
        atoms: impl IntoIterator<Item = Arc<Atom>>,
    ) -> Result<Self, SeedSelectionError> {
        let mut selected = SelectionBuilder::new(program);
        for atom in atoms {
            match selected.manual(atom.as_ref().into()) {
                Ok(()) => {}
                Err(ManualError::OutsideCarrier) => {
                    return Err(SeedSelectionError::OutsideCarrier { atom });
                }
                Err(ManualError::Allocation) => return Err(SeedSelectionError::Allocation),
            }
        }
        Ok(selected.finish())
    }

    pub(super) fn from_owned(
        program: &Program,
        atoms: impl IntoIterator<Item = Atom>,
    ) -> Result<Self, SeedError> {
        let mut selected = SelectionBuilder::new(program);
        for atom in atoms {
            match selected.manual((&atom).into()) {
                Ok(()) => {}
                Err(ManualError::OutsideCarrier) => return Err(SeedError::OutsideCarrier { atom }),
                Err(ManualError::Allocation) => return Err(SeedError::Allocation),
            }
        }
        Ok(selected.finish())
    }

    /// Share existing carrier-coordinate tokens from this exact Program.
    /// A full-carrier token whose signature is not a gate is refused. No payload
    /// is imported and no global carrier ordinal is required.
    ///
    /// # Errors
    /// Refuses wrong Program identity, a nongate token or unavailable vector storage.
    pub fn from_carrier_atoms(
        program: &Program,
        atoms: impl IntoIterator<Item = CarrierAtom>,
    ) -> Result<Self, SeedSelectionError> {
        let mut selected = SelectionBuilder::new(program);
        for atom in atoms {
            selected.carrier(atom)?;
        }
        Ok(selected.finish())
    }

    /// Retain core-minted gate atoms from this same immutable Program.
    /// Their original checked positions remain attached to their coordinates.
    /// Sorting and deduplication preserve the symbolic carrier's storage order.
    ///
    /// # Errors
    /// Refuses wrong Program identity or unavailable selection storage.
    pub fn from_gate_atoms(
        program: &Program,
        atoms: impl IntoIterator<Item = Arc<GateAtom>>,
    ) -> Result<Self, SeedSelectionError> {
        let mut selected = SelectionBuilder::new(program);
        for atom in atoms {
            selected.indexed(atom)?;
        }
        Ok(selected.finish())
    }

    /// Combine held coordinate tokens and selected indexed gates into one set.
    /// Duplicate atoms remain true once; when a duplicate has an indexed witness,
    /// that witness is retained. No copied atom payload or Cartesian expansion
    /// is required.
    ///
    /// # Errors
    /// Refuses wrong Program identity, nongate held tokens or unavailable storage.
    pub fn held_and_selected(
        program: &Program,
        held: impl IntoIterator<Item = CarrierAtom>,
        selected: impl IntoIterator<Item = Arc<GateAtom>>,
    ) -> Result<Self, SeedSelectionError> {
        let mut selection = SelectionBuilder::new(program);
        for atom in held {
            selection.carrier(atom)?;
        }
        for atom in selected {
            selection.indexed(atom)?;
        }
        Ok(selection.finish())
    }

    /// Borrow the exact true set without cloning coordinate tokens.
    #[must_use]
    pub fn view(&self) -> SeedView<'_> {
        SeedView {
            program: &self.0.program,
            atoms: &self.0.atoms,
        }
    }

    /// Share this exact selection as the compatible Seed wrapper.
    /// This is constant-time and copies no coordinate vector or logical payload.
    #[must_use]
    pub fn to_seed(&self) -> Seed {
        Seed {
            selection: self.clone(),
        }
    }
}

struct SelectionBuilder<'a> {
    program: &'a Program,
    atoms: Vec<Entry>,
}
enum ManualError {
    OutsideCarrier,
    Allocation,
}
impl<'a> SelectionBuilder<'a> {
    fn new(program: &'a Program) -> Self {
        Self {
            program,
            atoms: Vec::new(),
        }
    }
    fn push(&mut self, entry: Entry) -> Result<(), SeedSelectionError> {
        self.atoms
            .try_reserve(1)
            .map_err(|_| SeedSelectionError::Allocation)?;
        self.atoms.push(entry);
        Ok(())
    }
    fn manual(&mut self, atom: AtomRef<'_>) -> Result<(), ManualError> {
        let carrier = self
            .program
            .locate_atom(atom, true)
            .map_err(|_| ManualError::Allocation)?
            .ok_or(ManualError::OutsideCarrier)?;
        self.push(Entry::Manual(carrier))
            .map_err(|_| ManualError::Allocation)
    }
    fn carrier(&mut self, atom: CarrierAtom) -> Result<(), SeedSelectionError> {
        if !self.program.same_instance(atom.program()) {
            return Err(SeedSelectionError::WrongProgram);
        }
        if self
            .program
            .gate_predicates()
            .binary_search(atom.predicate())
            .is_err()
        {
            return Err(SeedSelectionError::OutsideGateCarrier { atom });
        }
        self.push(Entry::Manual(atom))
    }
    fn indexed(&mut self, atom: Arc<GateAtom>) -> Result<(), SeedSelectionError> {
        if !self.program.same_instance(atom.program()) {
            return Err(SeedSelectionError::WrongProgram);
        }
        self.push(Entry::Indexed(atom))
    }
    fn finish(mut self) -> SeedSelection {
        // The common Program witness permits coordinate ordering. Prefer the
        // existing indexed witness for equal tuples rather than discarding it.
        self.atoms.sort_unstable_by(|left, right| {
            left.carrier()
                .cmp(right.carrier())
                .then_with(|| right.position().is_some().cmp(&left.position().is_some()))
        });
        self.atoms
            .dedup_by(|left, right| left.carrier() == right.carrier());
        SeedSelection(Arc::new(SelectionData {
            program: self.program.clone(),
            atoms: self.atoms,
        }))
    }
}

impl Seed {
    /// Borrow the same immutable true set as its shared selection.
    #[must_use]
    pub fn view(&self) -> SeedView<'_> {
        self.selection.view()
    }
}

/// Construction stopped before returning a candidate. Rejected ingress remains
/// available for diagnosis; accepted inputs are retained only as coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeedSelectionError {
    /// A token belongs to a separately admitted Program.
    WrongProgram,
    /// Supplied ingress lies outside the Program's symbolic gate carrier.
    OutsideCarrier {
        /// The actual rejected caller-owned atom.
        atom: Arc<Atom>,
    },
    /// A validated full-carrier token has a signature not used by any gate.
    OutsideGateCarrier {
        /// The rejected coordinate token, without payload copying.
        atom: CarrierAtom,
    },
    /// A coordinate or selected-handle vector could not reserve its storage.
    Allocation,
}
impl fmt::Display for SeedSelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongProgram => f.write_str("gate atom belongs to a different program instance"),
            Self::OutsideCarrier { atom } => {
                write!(f, "candidate atom lies outside the gate carrier: {atom:?}")
            }
            Self::OutsideGateCarrier { atom } => write!(
                f,
                "candidate atom lies outside the gate carrier: {:?}",
                atom.atom()
            ),
            Self::Allocation => {
                f.write_str("candidate coordinate or selection storage could not be reserved")
            }
        }
    }
}
impl std::error::Error for SeedSelectionError {}

/// An allocation-free borrow of an instance-bound complete candidate.
#[derive(Clone, Copy, Debug)]
pub struct SeedView<'a> {
    program: &'a Program,
    atoms: &'a [Entry],
}
impl<'a> SeedView<'a> {
    /// The immutable Program whose carrier contains this candidate.
    #[must_use]
    pub const fn program(self) -> &'a Program {
        self.program
    }
    /// True atoms in semantic storage order, with exact borrowed traversal.
    #[must_use]
    pub fn atoms(self) -> SeedIter<'a> {
        self.atom_view().iter()
    }
    pub(super) fn atom_view(self) -> SeedAtoms<'a> {
        SeedAtoms {
            entries: self.atoms,
        }
    }
    /// Exact true membership using the shared typed atom comparator.
    #[must_use]
    pub fn contains<'query>(self, atom: impl Into<AtomRef<'query>>) -> bool {
        let atom = atom.into();
        self.atoms
            .binary_search_by(|stored| stored.atom().cmp(&atom))
            .is_ok()
    }
    /// Compare a checked substitution directly with selected coordinate rows.
    #[must_use]
    pub fn contains_key(self, key: &AtomKey<'_>) -> bool {
        self.atoms
            .binary_search_by(|stored| key.compare_ref(stored.atom()).reverse())
            .is_ok()
    }
    /// Checked true membership, admitting each probe and typed comparison.
    /// No coordinate or logical payload is allocated.
    ///
    /// # Errors
    /// Returns the first callback refusal before the corresponding operation.
    pub fn contains_key_with<E>(
        self,
        key: &AtomKey<'_>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<bool, E> {
        let (mut low, mut high) = (0, self.atoms.len());
        while low < high {
            let middle = low + (high - low) / 2;
            before()?;
            match key
                .compare_ref_with(self.atoms[middle].atom(), &mut before)?
                .reverse()
            {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return Ok(true),
            }
        }
        Ok(false)
    }
    /// Borrow complete entries with optional inseparable gate-position evidence.
    /// No complete-graph lookup happens until `resolve_in` is requested.
    #[must_use]
    pub fn entries(
        self,
    ) -> impl ExactSizeIterator<Item = SeedAtom<'a>> + DoubleEndedIterator + FusedIterator + Clone
    {
        self.atoms.iter().map(move |entry| SeedAtom {
            program: self.program,
            carrier: entry.carrier(),
            position: entry.position(),
        })
    }
}

/// Exact borrowed atom iteration over an immutable sparse selection.
#[derive(Clone, Copy, Debug)]
pub struct SeedAtoms<'a> {
    entries: &'a [Entry],
}
impl<'a> SeedAtoms<'a> {
    /// Number of true logical atoms.
    #[must_use]
    pub fn len(self) -> usize {
        self.entries.len()
    }
    /// Whether every carrier atom is false in this candidate.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.entries.is_empty()
    }
    /// Borrow a true atom by its semantic-storage-order position.
    #[must_use]
    pub fn at(self, index: usize) -> Option<AtomRef<'a>> {
        self.entries.get(index).map(Entry::atom)
    }
    /// Exact-size double-ended iteration without copying payload or coordinates.
    #[must_use]
    pub fn iter(self) -> SeedIter<'a> {
        SeedIter {
            entries: self.entries.iter(),
        }
    }
}
impl<'a> IntoIterator for SeedAtoms<'a> {
    type Item = AtomRef<'a>;
    type IntoIter = SeedIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
/// Exact true-atom cursor; cloning copies only its borrowed slice state.
#[derive(Clone, Debug)]
pub struct SeedIter<'a> {
    entries: slice::Iter<'a, Entry>,
}
impl<'a> Iterator for SeedIter<'a> {
    type Item = AtomRef<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        self.entries.next().map(Entry::atom)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.entries.size_hint()
    }
}
impl DoubleEndedIterator for SeedIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.entries.next_back().map(Entry::atom)
    }
}
impl ExactSizeIterator for SeedIter<'_> {}
impl FusedIterator for SeedIter<'_> {}

/// A true carrier atom plus optional checked full-carrier position.
/// Fields remain private so clients cannot replace its logical row independently.
#[derive(Clone, Copy, Debug)]
pub struct SeedAtom<'a> {
    program: &'a Program,
    carrier: &'a CarrierAtom,
    position: Option<NonZeroUsize>,
}
impl<'a> SeedAtom<'a> {
    /// Borrow the logical row from the original canonical Program vocabulary.
    #[must_use]
    pub fn atom(self) -> AtomRef<'a> {
        self.carrier.atom()
    }
    /// Resolve in a complete graph of this exact immutable Program. Indexed
    /// witnesses use one array lookup; manual coordinates use symbolic lookup.
    ///
    /// # Errors
    /// Refuses wrong Program identity or a missing complete-graph mapping.
    /// An invalid indexed position never falls back to symbolic lookup.
    pub fn resolve_in(self, graph: &GroundProgram) -> Result<AtomId, SeedError> {
        self.resolve_with(graph, |atom| graph.atom_id(atom))
    }
    fn resolve_with(
        self,
        graph: &GroundProgram,
        symbolic: impl FnOnce(AtomRef<'_>) -> Option<AtomId>,
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
            None => symbolic(self.atom()).ok_or(SeedError::InvalidCarrierMapping),
        }
    }
}
