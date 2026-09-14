//! Located source admission over the shared authoritative atom interner.
//!
//! Only vacant entries copy a checked substitution. First insertion fixes each
//! dense ID; commits move suffix ownership without reordering or copying payload.
//! AVL comparisons, path planning, reservations and commit work all use the
//! enclosing formula counter. Nested payload remains under `ScalarBytes`, while
//! the finite index-capacity envelope is derived from the applicable atom bound.

use themelios_base::span::Location;
use zetesis_core::atom_interner::{AtomEntry, AtomInterner, Failure, Limits};
use zetesis_core::{Atom, AtomKey};

use crate::formula_support::Counters;
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

#[derive(Default)]
pub(super) struct Catalog(AtomInterner);

impl Catalog {
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
    pub(super) fn get(&self, id: usize) -> Option<&Atom> {
        self.0.get(id)
    }

    /// Callers commit before exposing a complete contiguous capture population.
    pub(super) fn atoms(&self) -> &[Atom] {
        let committed = self.0.committed();
        assert_eq!(committed.len(), self.len(), "complete atom capture prefix");
        committed.as_slice()
    }

    pub(super) fn entry<'owner, 'key>(
        &'owner mut self,
        key: AtomKey<'key>,
        bound: (FormulaResource, usize),
        counters: &mut Counters,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<AtomEntry<'owner, 'key>, FormulaFailure> {
        self.0
            .entry_key_with(key, Limits::for_atoms(bound.1), || {
                counters.work(limits, location)
            })
            .map_err(|error| failure(error, bound, location))
    }

    pub(super) fn find(
        &self,
        atom: &Atom,
        bound: (FormulaResource, usize),
        counters: &mut Counters,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<Option<usize>, FormulaFailure> {
        self.0
            .find_atom_with(atom, Limits::for_atoms(bound.1), || {
                counters.work(limits, location)
            })
            .map_err(|error| failure(error, bound, location))
    }

    pub(super) fn commit(
        &mut self,
        bound: (FormulaResource, usize),
        counters: &mut Counters,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        self.0
            .commit_with(Limits::for_atoms(bound.1), || {
                counters.work(limits, location)
            })
            .map_err(|error| failure(error, bound, location))
    }

    pub(super) fn into_atoms(
        self,
        bound: (FormulaResource, usize),
        counters: &mut Counters,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<Vec<Atom>, FormulaFailure> {
        self.0
            .into_atoms_with(Limits::for_atoms(bound.1), || {
                counters.work(limits, location)
            })
            .map_err(|error| failure(error, bound, location))
    }
}

pub(super) fn failure(
    error: Failure<FormulaFailure>,
    bound: (FormulaResource, usize),
    location: Location,
) -> FormulaFailure {
    match error {
        Failure::Stopped(error) => error,
        Failure::Allocation(error) => FormulaFailure::AtomAllocation { error, location },
        Failure::Atoms { required, limit } => FormulaFailure::Limit {
            resource: bound.0,
            observed: required as u128,
            limit: limit as u128,
            location,
        },
        Failure::Bytes { required, limit } => FormulaFailure::Limit {
            resource: FormulaResource::AtomStorageBytes,
            observed: required,
            limit,
            location,
        },
        // Interner Overflow denotes a next element count beyond usize: byte
        // envelopes themselves are computed in u128 from admitted capacities.
        Failure::Overflow => FormulaFailure::Limit {
            resource: bound.0,
            observed: usize::MAX as u128 + 1,
            limit: bound.1 as u128,
            location,
        },
    }
}

#[cfg(test)]
mod tests;
