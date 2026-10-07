//! Account and detach an aggregate append before canonical suffix remapping.

use crate::formula::ceiling;
use crate::formula_support::{Computation, Counters, StorageLease};
use crate::{FormulaFailure, FormulaLimits, FormulaResource, ProgramSite};
use zetesis_ferraris::{FormulaSuffix, FormulaTransaction, Node};

pub(super) struct Detached {
    pub(super) suffix: FormulaSuffix,
    _storage: StorageLease,
}

pub(super) fn detach(
    transaction: FormulaTransaction<'_>,
    previous_operands: usize,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Detached, FormulaFailure> {
    let count = transaction.view().len() - transaction.first();
    let operands = transaction.parts().operands().len() - previous_operands;
    let mut storage = computation.lease();
    let header = size_of::<FormulaSuffix>();
    let requested = header as u128
        + count as u128 * size_of::<Node>() as u128
        + operands as u128 * size_of::<usize>() as u128;
    let allowance = computation.allowance(&storage, limits, location)?;
    ceiling(
        FormulaResource::SupportBytes,
        (limits.max_support_bytes - allowance) as u128 + requested,
        limits.max_support_bytes as u128,
        location,
    )?;
    counters.charge_work(count as u128 + operands as u128, limits, location)?;
    let suffix = transaction
        .detach()
        .map_err(|error| FormulaFailure::Theory { error, location })?;
    let actual = header
        + suffix.parts().node_capacity() * size_of::<Node>()
        + suffix.parts().operand_capacity() * size_of::<usize>();
    storage.observe(actual, location)?;
    computation.storage_observed(&storage, 0, header, limits, counters, location)?;
    Ok(Detached {
        suffix,
        _storage: storage,
    })
}
