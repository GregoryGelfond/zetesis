//! A normal-rule traversal borrows one checked source template across its bindings.

mod rows;
pub(crate) use rows::RowHead;

use zetesis_core::PatternRef;
use zetesis_core::atom_interner::PreparedPattern;

use super::{AtomPattern, Binding, Computation, Counters, StorageLease};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

pub(super) struct Head<'source> {
    pattern: Pattern<'source>,
    _lease: StorageLease,
}

enum Pattern<'source> {
    Prepared(PreparedPattern<'source>),
    General(PatternRef<'source>),
}

impl<'source> Head<'source> {
    pub(super) fn new(
        pattern: AtomPattern,
        computation: &Computation<'_, 'source>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let pattern = computation.static_pattern(pattern, limits, counters, location)?;
        let mut lease = computation.lease();
        lease.observe(size_of::<Self>(), location)?;
        computation.storage_observed(&lease, 0, size_of::<Self>(), limits, counters, location)?;
        let pattern = computation
            .prepare_pattern(pattern, limits, counters, location)?
            .map_or(Pattern::General(pattern), Pattern::Prepared);
        Ok(Self {
            pattern,
            _lease: lease,
        })
    }

    pub(super) fn derive(
        &self,
        binding: &Binding<'_>,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let atom = match &self.pattern {
            Pattern::Prepared(pattern) => {
                computation.prepared_atom(pattern, binding, limits, counters, location)?
            }
            Pattern::General(pattern) => {
                computation.atom(*pattern, binding, limits, counters, location)?
            }
        };
        computation.support(&atom, limits, counters, location)
    }
}

#[cfg(test)]
mod tests;
