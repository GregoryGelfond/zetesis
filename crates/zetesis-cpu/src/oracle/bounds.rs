//! Borrow the same must/may cube from owned atoms or a completed indexed root.

use std::{collections::BTreeSet, sync::Arc};

use zetesis_core::{Atom, AtomKey, GateAtom, Model, Program};

use super::Cube;
use crate::regions::Region;

/// A closure's immutable pre-pass gate bounds. The owned cube also represents
/// the symbolic, unbounded root; a region view exists only after root narrowing
/// completed and retained every possible gate atom.
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

    pub(super) fn must_hold(self, key: &AtomKey<'_>) -> bool {
        match self {
            Self::Owned(cube) => cube.must_hold(key),
            Self::Region(bounds) => {
                !bounds.lower_is_empty
                    && (key.get(bounds.held).is_some()
                        || bounds
                            .position(key)
                            .is_some_and(|at| bounds.region.is_held(at)))
            }
        }
    }

    pub(super) fn may_hold(self, key: &AtomKey<'_>) -> bool {
        match self {
            Self::Owned(cube) => cube.may_hold(key),
            Self::Region(bounds) => {
                key.get(bounds.held).is_some()
                    || bounds
                        .position(key)
                        .is_some_and(|at| !bounds.region.is_cut(at))
            }
        }
    }
}

/// A descendant's bounds borrow the root's existing atom owners and decisions.
/// `root` is distinct and canonical, disjoint from `held`, with one coordinate
/// per region cell. Both owners belong to the same admitted program and their
/// union is its completed root upper bound. Missing keys are therefore cut,
/// never an unknown symbolic tail. No atom, index or decision storage is copied.
pub(crate) struct RegionBounds<'a> {
    held: &'a BTreeSet<Atom>,
    root: &'a [Arc<GateAtom>],
    region: &'a Region,
    // Computed once per pass, preserving the empty-lower template shortcut
    // without repeatedly scanning region decisions for each source template.
    lower_is_empty: bool,
}

impl<'a> RegionBounds<'a> {
    pub(crate) fn new(
        held: &'a BTreeSet<Atom>,
        root: &'a [Arc<GateAtom>],
        region: &'a Region,
    ) -> Self {
        debug_assert_eq!(root.len(), region.len());
        Self {
            held,
            root,
            region,
            lower_is_empty: held.is_empty() && region.held().next().is_none(),
        }
    }

    fn position(&self, key: &AtomKey<'_>) -> Option<usize> {
        self.root
            .binary_search_by(|gate| key.compare(gate.atom()).reverse())
            .ok()
    }

    fn may_contain(&self, atom: &Atom) -> bool {
        self.held.contains(atom)
            || self
                .root
                .binary_search_by(|gate| gate.atom().cmp(atom))
                .is_ok_and(|at| !self.region.is_cut(at))
    }

    /// Test `(old_must ∪ lower_gate) ⊆ (old_may ∩ upper_gate)` before
    /// committing any decision. Existing must atoms already lie in old may;
    /// new lower atoms must pass both tests, including atoms outside this root.
    /// The upper closure's constraint flag is deliberately not a refutation.
    pub(crate) fn conflicts(&self, program: &Program, lower: &Model, upper: &Model) -> bool {
        self.held
            .iter()
            .chain(self.region.held().map(|at| self.root[at].atom()))
            .any(|atom| !upper.contains(atom))
            || lower.atoms().iter().any(|atom| {
                program.contains_gate_atom(atom)
                    && (!self.may_contain(atom) || !upper.contains(atom))
            })
    }
}

#[cfg(test)]
#[path = "../../tests/support/region_bound_readings.rs"]
mod tests;
