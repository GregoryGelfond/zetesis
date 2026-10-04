//! Checked output occurrence publication from the completed source authority.

use super::{
    Counters, Event, FormulaFailure, FormulaLimits, FormulaResource, ProgramSite, SourceScope,
    SupportCatalog, atom_failure, owner_limits,
};
use zetesis_core::AtomCatalog;

impl SupportCatalog {
    pub(in crate::formula_support) fn publish_selection(
        &mut self,
        scope: &SourceScope,
        positions: &[usize],
        workspace: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<AtomCatalog, FormulaFailure> {
        counters.work(limits, location)?;
        if !self.scope.same(scope) {
            return Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location,
            });
        }
        let outer = self.bytes(location)? as u128 - self.owner.storage_bytes() + workspace as u128;
        let mut checked = owner_limits(limits, outer, location)?;
        let atom_limit = limits.max_atom_storage_bytes as u128;
        let atom_bound = atom_limit < checked.max_bytes;
        checked.max_bytes = checked.max_bytes.min(atom_limit);
        self.owner.restart_storage_peak();
        let result = self
            .owner
            .publish_selection_with(positions, checked, || counters.work(limits, location));
        counters.record(Event::SupportPeakBytes(
            outer + self.owner.storage_peak_bytes(),
        ));
        result.map_err(|error| match error {
            zetesis_core::atom_interner::Failure::Bytes { required, .. } if atom_bound => {
                FormulaFailure::Limit {
                    resource: FormulaResource::AtomStorageBytes,
                    observed: required,
                    limit: atom_limit,
                    location,
                }
            }
            error => atom_failure(error, limits, outer, location),
        })
    }
}
