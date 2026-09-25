//! The same must/may cube read through retained carrier coordinates or a root.

use std::sync::Arc;

use zetesis_core::{AtomKey, CarrierAtom, GateAtom, Model, Program, catalog::AtomRef};

use super::{Cube, Work};
use crate::{Stop, regions::Region};

/// A sorted, distinct set of coordinate handles for one admitted Program.
/// Values and predicate spellings remain in that Program. Vector capacity is
/// fallibly admitted; cloned handles share their immutable tuple allocation.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub(crate) struct CarrierSet(Vec<CarrierAtom>);

impl CarrierSet {
    pub(crate) const fn new() -> Self {
        Self(Vec::new())
    }
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub(crate) fn iter(&self) -> std::slice::Iter<'_, CarrierAtom> {
        self.0.iter()
    }
    pub(crate) fn handle_bytes(&self) -> u128 {
        self.0.capacity() as u128 * size_of::<CarrierAtom>() as u128
    }
    /// Named tuple allocations, excluding the one shared Program owner.
    pub(crate) fn coordinate_bytes(&self) -> u128 {
        self.0.iter().map(CarrierAtom::coordinate_bytes).sum()
    }
    pub(crate) fn into_atoms(self) -> Vec<CarrierAtom> {
        self.0
    }
    #[cfg(test)]
    pub(crate) fn insert(&mut self, atom: CarrierAtom) {
        if let Err(at) = self.0.binary_search(&atom) {
            self.0.insert(at, atom);
        }
    }
    #[cfg(test)]
    pub(crate) fn contains(&self, atom: &CarrierAtom) -> bool {
        self.0.binary_search(atom).is_ok()
    }
    #[cfg(test)]
    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }

    pub(crate) fn contains_key(
        &self,
        key: &AtomKey<'_>,
        work: &mut Work<'_>,
    ) -> Result<bool, Stop> {
        self.search(|atom| {
            key.compare_ref_with(atom.atom(), || work.tick())
                .map(std::cmp::Ordering::reverse)
        })
    }
    pub(crate) fn contains_atom(
        &self,
        atom: AtomRef<'_>,
        work: &mut Work<'_>,
    ) -> Result<bool, Stop> {
        self.search(|candidate| candidate.atom().compare_ref_with(atom, || work.tick()))
    }
    fn search(
        &self,
        mut compare: impl FnMut(&CarrierAtom) -> Result<std::cmp::Ordering, Stop>,
    ) -> Result<bool, Stop> {
        let (mut low, mut high) = (0, self.len());
        while low < high {
            let middle = low + (high - low) / 2;
            match compare(&self.0[middle])? {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return Ok(true),
            }
        }
        Ok(false)
    }

    /// Reserve the replacement buffer while the current buffer remains live.
    /// `other_bytes` includes every other named transfer allocation. Tuple
    /// ownership itself is admitted by the caller before it is published here.
    pub(crate) fn reserve(
        &mut self,
        additional: usize,
        other_bytes: u128,
        limit: usize,
        peak: &mut usize,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        let required = self.len().checked_add(additional).ok_or(Stop::Allocation)?;
        if required <= self.0.capacity() {
            return Ok(());
        }
        let requested = required.max(self.0.capacity().saturating_mul(2)).max(4);
        let replacement = requested as u128 * size_of::<CarrierAtom>() as u128;
        let old = self.handle_bytes();
        let proposed = other_bytes + old + replacement;
        if proposed > limit as u128 {
            return Err(Stop::Allocation);
        }
        *peak = (*peak).max(usize::try_from(proposed).map_err(|_| Stop::Allocation)?);
        for _ in 0..self.0.len().max(1) {
            work.tick()?;
        }
        self.0
            .try_reserve_exact(requested - self.len())
            .map_err(|_| Stop::Allocation)?;
        let actual = other_bytes + old + self.handle_bytes();
        *peak = (*peak).max(usize::try_from(actual).map_err(|_| Stop::Allocation)?);
        if actual > limit as u128 {
            return Err(Stop::Allocation);
        }
        Ok(())
    }
    /// Input order is established by a completed Model or a checked set merge.
    pub(crate) fn push_ordered(
        &mut self,
        atom: CarrierAtom,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        work.tick()?;
        self.0.push(atom);
        Ok(())
    }
}

#[cfg(test)]
impl FromIterator<CarrierAtom> for CarrierSet {
    fn from_iter<T: IntoIterator<Item = CarrierAtom>>(iter: T) -> Self {
        let mut atoms: Vec<_> = iter.into_iter().collect();
        atoms.sort_unstable();
        atoms.dedup();
        Self(atoms)
    }
}

/// Immutable pre-pass gate bounds. `None` in an owned upper side denotes the
/// entire symbolic carrier; missing atoms in a completed root are instead cut.
#[derive(Clone, Copy)]
pub(crate) enum Bounds<'a> {
    Owned(&'a Cube),
    Region(&'a RegionBounds<'a>),
}
impl<'a> From<&'a Cube> for Bounds<'a> {
    fn from(cube: &'a Cube) -> Self {
        Self::Owned(cube)
    }
}
impl Bounds<'_> {
    pub(super) fn lower_is_empty(self) -> bool {
        match self {
            Self::Owned(cube) => cube.must.is_empty(),
            Self::Region(bounds) => bounds.lower_is_empty,
        }
    }
    pub(super) fn upper_is_bounded(self) -> bool {
        match self {
            Self::Owned(cube) => cube.may.is_some(),
            Self::Region(_) => true,
        }
    }
    pub(super) fn must_hold(self, key: &AtomKey<'_>, work: &mut Work<'_>) -> Result<bool, Stop> {
        match self {
            Self::Owned(cube) => cube.must_hold(key, work),
            Self::Region(bounds) => {
                if bounds.lower_is_empty {
                    return Ok(false);
                }
                Ok(bounds.held.contains_key(key, work)?
                    || bounds
                        .position(key, work)?
                        .is_some_and(|at| bounds.region.is_held(at)))
            }
        }
    }
    pub(super) fn may_hold(self, key: &AtomKey<'_>, work: &mut Work<'_>) -> Result<bool, Stop> {
        match self {
            Self::Owned(cube) => cube.may_hold(key, work),
            Self::Region(bounds) => Ok(bounds.held.contains_key(key, work)?
                || bounds
                    .position(key, work)?
                    .is_some_and(|at| !bounds.region.is_cut(at))),
        }
    }
}

/// The two owners are distinct, sorted, disjoint and belong to one Program.
/// Their union is the completed root upper bound. No tuple payload is copied.
pub(crate) struct RegionBounds<'a> {
    held: &'a CarrierSet,
    root: &'a [Arc<GateAtom>],
    region: &'a Region,
    lower_is_empty: bool,
}
impl<'a> RegionBounds<'a> {
    pub(crate) fn new(held: &'a CarrierSet, root: &'a [Arc<GateAtom>], region: &'a Region) -> Self {
        debug_assert_eq!(root.len(), region.len());
        Self {
            held,
            root,
            region,
            lower_is_empty: held.is_empty() && region.held().next().is_none(),
        }
    }
    fn position(&self, key: &AtomKey<'_>, work: &mut Work<'_>) -> Result<Option<usize>, Stop> {
        self.find(|gate| {
            key.compare_ref_with(gate.atom(), || work.tick())
                .map(std::cmp::Ordering::reverse)
        })
    }
    fn find(
        &self,
        mut compare: impl FnMut(&GateAtom) -> Result<std::cmp::Ordering, Stop>,
    ) -> Result<Option<usize>, Stop> {
        let (mut low, mut high) = (0, self.root.len());
        while low < high {
            let middle = low + (high - low) / 2;
            match compare(&self.root[middle])? {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return Ok(Some(middle)),
            }
        }
        Ok(None)
    }
    fn may_contain(&self, atom: AtomRef<'_>, work: &mut Work<'_>) -> Result<bool, Stop> {
        Ok(self.held.contains_atom(atom, work)?
            || self
                .find(|gate| gate.atom().compare_ref_with(atom, || work.tick()))?
                .is_some_and(|at| !self.region.is_cut(at)))
    }
    /// `(old_must ∪ lower_gate) ⊆ (old_may ∩ upper_gate)`, fully checked
    /// before any region decision changes. Upper constraints do not refute.
    pub(crate) fn conflicts(
        &self,
        program: &Program,
        lower: &Model,
        upper: &Model,
        work: &mut Work<'_>,
    ) -> Result<bool, Stop> {
        for atom in self
            .held
            .iter()
            .map(CarrierAtom::atom)
            .chain(self.region.held().map(|at| self.root[at].atom()))
        {
            if !model_contains(upper, atom, work)? {
                return Ok(true);
            }
        }
        for atom in lower.atoms() {
            work.tick()?;
            if program
                .gate_predicates()
                .binary_search_with(atom.predicate(), || work.tick())?
                .is_ok()
                && (!self.may_contain(atom, work)? || !model_contains(upper, atom, work)?)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Checked binary selection lookup; no alternate atom payload is constructed.
pub(crate) fn model_contains(
    model: &Model,
    atom: AtomRef<'_>,
    work: &mut Work<'_>,
) -> Result<bool, Stop> {
    let atoms = model.atoms();
    let (mut low, mut high) = (0, atoms.len());
    while low < high {
        let middle = low + (high - low) / 2;
        work.tick()?;
        let candidate = atoms.at(middle).ok_or(Stop::InvalidProgram)?;
        match candidate.compare_ref_with(atom, || work.tick())? {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return Ok(true),
        }
    }
    Ok(false)
}

#[cfg(test)]
#[path = "../../tests/support/region_bound_readings.rs"]
mod tests;
