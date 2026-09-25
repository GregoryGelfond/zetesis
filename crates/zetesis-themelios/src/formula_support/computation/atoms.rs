//! Assigned source atoms carry identity without claiming any membership.

use crate::formula_support::Context;
use zetesis_core::catalog::AtomRef;
use zetesis_core::{AtomKey, PatternRef, TemplateTerm};

use super::{Computation, Counters, FormulaFailure, FormulaLimits, Location, StorageLease, Terms};
use crate::formula_binding::Binding;
use crate::formula_support::relations::{AssignedAtom, SourceAtom, SourceScope};

impl Computation<'_, '_> {
    pub(crate) fn signed(
        &self,
        atom: &SourceAtom,
        sign: zetesis_core::Sign,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<SourceAtom>, FormulaFailure> {
        match &self.terms {
            Terms::Append(append) => append.signed(
                atom,
                sign,
                self.support.workspace_bytes(),
                limits,
                counters,
                location,
            ),
            Terms::Frozen(_) => Err(owner_failure(location)),
        }
    }

    /// Source rows can enter the same authority directly. Same-owner rows reuse
    /// their payload; other borrowed input follows the canonical importer.
    pub(crate) fn atom_ref(
        &mut self,
        atom: AtomRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<SourceAtom, FormulaFailure> {
        match &mut self.terms {
            Terms::Append(append) => append.discover_ref(
                atom,
                self.support.workspace_bytes(),
                limits,
                counters,
                location,
            ),
            Terms::Frozen(_) => Err(owner_failure(location)),
        }
    }

    /// Resolve borrowed constants and copy variable IDs into one leased argument
    /// frame. Repeated arguments preserve their original positions. No borrowed
    /// resolver remains live across admission into the same source authority.
    pub(crate) fn atom(
        &mut self,
        pattern: PatternRef<'_>,
        binding: &Binding<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<SourceAtom, FormulaFailure> {
        if matches!(&self.terms, Terms::Frozen(_)) {
            return Err(owner_failure(location));
        }
        let mut values = Binding::new(self, limits, counters, location)?;
        values.extend_scope(pattern.terms().len(), self, limits, counters, location)?;
        let mut arguments = Vec::new();
        let mut lease = self.lease();
        let header = size_of::<Vec<usize>>() + size_of::<StorageLease>();
        lease.observe(header, location)?;
        self.storage_observed(&lease, 0, header, limits, counters, location)?;
        crate::formula_support::reserve(
            &mut arguments,
            pattern.terms().len(),
            &mut lease,
            header,
            Context::new(self, limits, counters, location),
        )?;
        let terms = pattern.terms();
        for position in 0..terms.len() {
            counters.work(limits, location)?;
            let term = terms.at(position).expect("checked pattern arity");
            let key = match term {
                TemplateTerm::Constant(value) => self
                    .read()
                    .term_key(value)
                    .map_err(|error| crate::formula_binding::assignment(error.into(), location))?,
                TemplateTerm::Variable(variable) => binding.key(variable, location)?,
            };
            values.set(position, &key, limits, counters, location)?;
            counters.work(limits, location)?;
            arguments.push(position);
        }
        let Terms::Append(append) = &mut self.terms else {
            return Err(owner_failure(location));
        };
        let workspace = self.support.workspace_bytes();
        counters.work(limits, location)?;
        let predicate = append
            .read()
            .declare_existing(pattern.predicate())
            .map_err(|error| crate::formula_binding::assignment(error.into(), location))?;
        append.assigned(
            AssignedAtom {
                predicate: &predicate,
                values: values.slots(),
                arguments: &arguments,
            },
            workspace,
            limits,
            counters,
            location,
        )
    }

    /// Producer heads explicitly select discovered identities into support.
    /// Final formula emission and objective/output selection do not call this.
    pub(crate) fn support(
        &mut self,
        atom: &SourceAtom,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        match &mut self.terms {
            Terms::Append(append) => append.retain(
                atom,
                self.support.workspace_bytes(),
                limits,
                counters,
                location,
            ),
            Terms::Frozen(_) => Err(owner_failure(location)),
        }
    }

    /// Test published or pending support, rather than mere identity admission.
    pub(crate) fn contains(
        &self,
        key: AtomKey<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        match &self.terms {
            Terms::Append(append) => append.contains(
                key,
                self.support.workspace_bytes(),
                limits,
                counters,
                location,
            ),
            Terms::Frozen(_) => self.support.contains(&key, limits, counters, location),
        }
    }

    pub(crate) fn source_scope(&self, location: Location) -> Result<SourceScope, FormulaFailure> {
        match &self.terms {
            Terms::Append(append) => Ok(append.source_scope()),
            Terms::Frozen(_) => Err(owner_failure(location)),
        }
    }

    pub(crate) fn source_atom(
        &self,
        atom: &SourceAtom,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<AtomRef<'_>, FormulaFailure> {
        self.source_at(&atom.scope, atom.position, limits, counters, location)
    }

    pub(in crate::formula_support) fn source_at(
        &self,
        scope: &SourceScope,
        position: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<AtomRef<'_>, FormulaFailure> {
        match &self.terms {
            Terms::Append(append) => append.source_at(scope, position, limits, counters, location),
            Terms::Frozen(_) => Err(owner_failure(location)),
        }
    }
}

fn owner_failure(location: Location) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Owner,
        location,
    }
}

#[cfg(test)]
mod tests;
