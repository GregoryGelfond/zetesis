//! Assigned source atoms carry identity without claiming any membership.

use zetesis_core::PatternRef;
use zetesis_core::atom_interner::{PreparedPattern, PreparedRows, RowColumn};
use zetesis_core::catalog::AtomRef;
use zetesis_core::relation::{Relation, Row};

use super::{Computation, Counters, FormulaFailure, FormulaLimits, ProgramSite, Terms};
use crate::formula_binding::Binding;
use crate::formula_support::GroundingWork;
use crate::formula_support::relations::{SourceAtom, SourceScope};

impl Computation<'_, '_> {
    pub(crate) fn signed(
        &self,
        atom: &SourceAtom,
        sign: zetesis_core::Sign,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
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
        location: ProgramSite,
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

    /// Project constants and assigned variables directly into canonical atom
    /// arguments. Admission reuses that projection for lookup and row interning;
    /// no replacement assignment or argument-position vector is constructed.
    pub(crate) fn atom(
        &mut self,
        pattern: PatternRef<'_>,
        binding: &Binding<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<SourceAtom, FormulaFailure> {
        let Terms::Append(append) = &mut self.terms else {
            return Err(owner_failure(location));
        };
        append.assigned(
            pattern,
            binding.slots(),
            self.support.workspace_bytes(),
            GroundingWork::new(limits, counters, location),
        )
    }

    pub(crate) fn prepare_pattern<'pattern>(
        &self,
        pattern: PatternRef<'pattern>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<PreparedPattern<'pattern>>, FormulaFailure> {
        let Terms::Append(append) = &self.terms else {
            return Err(owner_failure(location));
        };
        append.prepare_pattern(
            pattern,
            self.support.workspace_bytes(),
            GroundingWork::new(limits, counters, location),
        )
    }

    pub(crate) fn prepare_rows<'rows, 'source>(
        &self,
        pattern: PatternRef<'source>,
        sources: Vec<&'rows Relation<'source>>,
        columns: Vec<Option<RowColumn>>,
        work: GroundingWork<'_>,
    ) -> Result<Option<PreparedRows<'rows, 'source>>, FormulaFailure> {
        let Terms::Append(append) = &self.terms else {
            return Err(owner_failure(work.location));
        };
        append.prepare_rows(
            pattern,
            sources,
            columns,
            self.support.workspace_bytes(),
            work,
        )
    }

    pub(crate) fn row_atom(
        &mut self,
        pattern: &PreparedRows<'_, '_>,
        rows: &[Row<'_, '_>],
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<SourceAtom, FormulaFailure> {
        let Terms::Append(append) = &mut self.terms else {
            return Err(owner_failure(location));
        };
        append.rows_assigned(
            pattern,
            rows,
            self.support.workspace_bytes(),
            GroundingWork::new(limits, counters, location),
        )
    }

    pub(crate) fn prepared_atom(
        &mut self,
        pattern: &PreparedPattern<'_>,
        binding: &Binding<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<SourceAtom, FormulaFailure> {
        let Terms::Append(append) = &mut self.terms else {
            return Err(owner_failure(location));
        };
        append.prepared_assigned(
            pattern,
            binding.slots(),
            self.support.workspace_bytes(),
            GroundingWork::new(limits, counters, location),
        )
    }

    /// Producer heads explicitly select discovered identities into support.
    /// Final formula emission and objective/output selection do not call this.
    pub(crate) fn support(
        &mut self,
        atom: &SourceAtom,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
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
    pub(crate) fn contains_pattern(
        &self,
        pattern: PatternRef<'_>,
        binding: &Binding<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        match &self.terms {
            Terms::Append(append) => append.contains_pattern(
                pattern,
                binding.slots(),
                self.support.workspace_bytes(),
                GroundingWork::new(limits, counters, location),
            ),
            Terms::Frozen(_) => {
                let view = binding.view(self.read(), limits, counters, location)?;
                let key = pattern
                    .key(view)
                    .map_err(|error| FormulaFailure::UnsafeVariable {
                        variable: error.variable,
                        location,
                    })?;
                self.support.contains(&key, limits, counters, location)
            }
        }
    }

    pub(crate) fn source_scope(
        &self,
        location: ProgramSite,
    ) -> Result<SourceScope, FormulaFailure> {
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
        location: ProgramSite,
    ) -> Result<AtomRef<'_>, FormulaFailure> {
        self.source_at(&atom.scope, atom.position, limits, counters, location)
    }

    pub(in crate::formula_support) fn source_at(
        &self,
        scope: &SourceScope,
        position: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<AtomRef<'_>, FormulaFailure> {
        match &self.terms {
            Terms::Append(append) => append.source_at(scope, position, limits, counters, location),
            Terms::Frozen(_) => Err(owner_failure(location)),
        }
    }
}

fn owner_failure(location: ProgramSite) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Owner,
        location,
    }
}

#[cfg(test)]
mod tests;
