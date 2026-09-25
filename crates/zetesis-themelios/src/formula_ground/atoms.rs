//! Theory and condition coordinates select the shared source authority.
//!
//! First emission fixes a local dense coordinate. Source identity discovery and
//! support membership are separate; only sparse integer maps live in this owner.

use themelios_base::span::Location;
use zetesis_core::Sign;
use zetesis_core::catalog::AtomRef;

use crate::formula_support::{Computation, Counters, SourceAtom, SourceSelection};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

pub(super) struct Catalog(SourceSelection);

impl Catalog {
    pub(super) fn new(
        computation: &Computation<'_, '_>,
        counters: &Counters,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        SourceSelection::new(computation, limits, counters, location).map(Self)
    }

    pub(super) fn len(&self) -> usize {
        self.0.len()
    }

    pub(super) fn source(
        &self,
        local: usize,
        counters: &mut Counters,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<SourceAtom, FormulaFailure> {
        self.0.source(local, limits, counters, location)
    }

    pub(super) fn get<'read>(
        &self,
        id: usize,
        computation: &'read Computation<'_, '_>,
        counters: &mut Counters,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<AtomRef<'read>, FormulaFailure> {
        self.0.atom(id, computation, limits, counters, location)
    }

    pub(super) fn position(
        &self,
        atom: &SourceAtom,
        counters: &mut Counters,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<Option<usize>, FormulaFailure> {
        self.0.position(atom, limits, counters, location)
    }

    pub(super) fn insert(
        &mut self,
        atom: &SourceAtom,
        bound: (FormulaResource, usize),
        computation: &Computation<'_, '_>,
        counters: &mut Counters,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<(usize, bool), FormulaFailure> {
        self.0
            .insert(atom, bound, computation, limits, counters, location)
    }

    pub(super) fn find(
        &self,
        local: usize,
        sign: Sign,
        computation: &Computation<'_, '_>,
        counters: &mut Counters,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<Option<usize>, FormulaFailure> {
        let source = self.0.source(local, limits, counters, location)?;
        let Some(other) = computation.signed(&source, sign, limits, counters, location)? else {
            return Ok(None);
        };
        self.0.position(&other, limits, counters, location)
    }

    pub(super) fn into_selection(self) -> SourceSelection {
        self.0
    }
}

#[cfg(test)]
mod tests;
