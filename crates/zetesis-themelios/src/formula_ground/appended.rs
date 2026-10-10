//! Account and detach an aggregate append before canonical suffix remapping.

use crate::formula::ceiling;
use crate::formula_support::{Computation, Counters, StorageLease};
use crate::{FormulaFailure, FormulaLimits, FormulaResource, ProgramSite};
use zetesis_ferraris::{FormulaSuffix, FormulaTransaction};

pub(super) struct Detached {
    pub(super) suffix: FormulaSuffix,
    _storage: StorageLease,
}

pub(super) fn detach(
    transaction: FormulaTransaction<'_>,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Detached, FormulaFailure> {
    let mut storage = computation.lease();
    let header = size_of::<FormulaSuffix>();
    let requested = header as u128 + transaction.appended_payload_bytes();
    let allowance = computation.allowance(&storage, limits, location)?;
    ceiling(
        FormulaResource::SupportBytes,
        (limits.max_support_bytes - allowance) as u128 + requested,
        limits.max_support_bytes as u128,
        location,
    )?;
    counters.charge_work(transaction.appended_cells(), limits, location)?;
    let suffix = transaction
        .detach()
        .map_err(|error| FormulaFailure::Theory { error, location })?;
    let actual =
        usize::try_from(header as u128 + suffix.parts().retained_bytes()).map_err(|_| {
            FormulaFailure::Theory {
                error: zetesis_ferraris::AdmissionError::Allocation,
                location,
            }
        })?;
    storage.observe(actual, location)?;
    computation.storage_observed(&storage, 0, header, limits, counters, location)?;
    Ok(Detached {
        suffix,
        _storage: storage,
    })
}
