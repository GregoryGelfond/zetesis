//! Retain the real base grounding continuation through theory validation.

use crate::ProgramSite;

use super::{Compiled, Schedule};
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_support::{
    Accounting, CompletedCatalog, Counters, StorageLease, SupportCatalog,
};
use crate::{FormulaFailure, FormulaLimits, FormulaResource, GroundingObserver};

/// No source payload is copied. These are the exact accepted owners and history
/// after emission and theory validation. The caller closes this catalog and
/// replaces the retained envelope receipt before retiring the original account.
pub(crate) struct RetainedGrounding {
    pub(crate) compiled: Compiled,
    pub(crate) catalog: CompletedCatalog,
    pub(crate) accounting: Accounting,
    pub(crate) budget: Budget,
    pub(crate) output_storage: StorageLease,
}

pub(super) struct RetainedState {
    pub(super) catalog: CompletedCatalog,
    pub(super) accounting: Accounting,
    pub(super) budget: Budget,
    pub(super) output_storage: StorageLease,
}

impl RetainedGrounding {
    /// Named fixed envelope charged by `output_storage`, excluding the source
    /// catalog header and generated-history header already charged elsewhere.
    /// Retire/replace this amount when the fields move into the final plan.
    pub(crate) fn envelope_bytes(&self) -> usize {
        envelope_bytes(&self.accounting)
    }

    pub(super) fn admit_envelope(
        lease: &mut StorageLease,
        external: usize,
        counters: &Counters,
        limits: &FormulaLimits,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let added = envelope_bytes(&counters.accounting);
        let total = external as u128 + counters.workspace_bytes() as u128 + added as u128;
        ceiling(
            FormulaResource::SupportBytes,
            total,
            limits.max_support_bytes as u128,
            location,
        )?;
        let actual = lease
            .bytes()
            .checked_add(added)
            .ok_or(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                observed: total,
                limit: limits.max_support_bytes as u128,
                location,
            })?;
        lease.observe(actual, location)?;
        counters.record(crate::grounding_observer::Event::SupportPeakBytes(total));
        Ok(())
    }
}

fn envelope_bytes(accounting: &Accounting) -> usize {
    size_of::<RetainedGrounding>() - size_of::<SupportCatalog>() - accounting.leased_header_bytes()
}

/// Ground an actual base Preparation while retaining its original canonical
/// owner, account, expansion budget and output receipts. This does not classify
/// terminal definitions or establish correspondence to another source program.
pub(crate) fn ground_retained(
    preparation: crate::formula::Preparation,
    observer: Option<&dyn GroundingObserver>,
) -> Result<RetainedGrounding, FormulaFailure> {
    let super::Grounded {
        compiled,
        constraints,
        retained,
    } = super::ground_with_schedule(preparation, observer, Schedule::Retained)?;
    debug_assert!(constraints.is_none());
    let RetainedState {
        catalog,
        accounting,
        budget,
        output_storage,
    } = retained.expect("retained schedule preserves its source owner");
    Ok(RetainedGrounding {
        compiled,
        catalog,
        accounting,
        budget,
        output_storage,
    })
}

#[cfg(test)]
mod tests;
