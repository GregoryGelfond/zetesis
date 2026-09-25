//! Named canonical authority and relation-metadata capacity admission.
//!
//! Payload belongs to the one canonical authority. Relation, pending and dense
//! metadata are counted separately; no owned tuple copies are made here.

use std::mem::size_of;

use zetesis_core::atom_interner::{Failure, Limits};

use super::super::Work;
use crate::Stop;

pub(in crate::oracle) fn admit(work: &mut Work<'_>, bytes: u128) -> Result<(), Stop> {
    work.cancellation.poll()?;
    if bytes > work.limits.max_closure_bytes as u128 {
        return Err(Stop::StorageLimit);
    }
    Ok(())
}

/// Record admitted or acquired capacity, including allocator slack on refusal.
pub(in crate::oracle) fn record(work: &mut Work<'_>, bytes: u128) -> Result<(), Stop> {
    let bytes = usize::try_from(bytes).map_err(|_| Stop::StorageLimit)?;
    work.statistics.peak_closure_bytes = work.statistics.peak_closure_bytes.max(bytes);
    Ok(())
}

pub(in crate::oracle) fn after_reservation(work: &mut Work<'_>, bytes: u128) -> Result<(), Stop> {
    record(work, bytes)?;
    if bytes > work.limits.max_closure_bytes as u128 {
        Err(Stop::StorageLimit)
    } else {
        Ok(())
    }
}

/// Grow one metadata buffer, admitting old/replacement overlap before reserve.
/// `live` includes its old capacity; the return value replaces that charge with
/// actual new capacity. Existing contents remain whole after every refusal.
pub(super) fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    live: u128,
    work: &mut Work<'_>,
) -> Result<u128, Stop> {
    let required = values
        .len()
        .checked_add(additional)
        .ok_or(Stop::StorageLimit)?;
    if required <= values.capacity() {
        return Ok(live);
    }
    let target = required.max(values.capacity().saturating_mul(2));
    let replacement = (target as u128)
        .checked_mul(size_of::<T>() as u128)
        .ok_or(Stop::StorageLimit)?;
    let overlap = live.checked_add(replacement).ok_or(Stop::StorageLimit)?;
    admit(work, overlap)?;
    super::charge(work, values.len())?;
    let old = values.capacity() as u128 * size_of::<T>() as u128;
    values
        .try_reserve_exact(target - values.len())
        .map_err(|_| Stop::Allocation)?;
    let allocated = values.capacity() as u128 * size_of::<T>() as u128;
    record(work, live.checked_add(allocated).ok_or(Stop::StorageLimit)?)?;
    let current = live
        .checked_sub(old)
        .and_then(|bytes| bytes.checked_add(allocated))
        .ok_or(Stop::StorageLimit)?;
    after_reservation(work, current)?;
    Ok(current)
}

pub(in crate::oracle) fn atom_limits(work: &Work<'_>, other: u128) -> Result<Limits, Stop> {
    Ok(Limits {
        // Identity survives truth reset. Its cumulative population is bounded
        // by named storage; max_derived_atoms bounds current truth separately.
        max_atoms: usize::MAX,
        max_bytes: (work.limits.max_closure_bytes as u128)
            .checked_sub(other)
            .ok_or(Stop::StorageLimit)?,
    })
}

pub(in crate::oracle) fn atom_failure(failure: Failure<Stop>) -> Stop {
    match failure {
        Failure::Stopped(stop) => stop,
        Failure::Bytes { .. } | Failure::Overflow => Stop::StorageLimit,
        Failure::Atoms { .. } => Stop::DerivedAtomLimit,
        Failure::Allocation(_) => Stop::Allocation,
        Failure::Catalog(error) => match error {
            zetesis_core::catalog::Error::Allocation => Stop::Allocation,
            zetesis_core::catalog::Error::Storage { .. }
            | zetesis_core::catalog::Error::Overflow
            | zetesis_core::catalog::Error::IdExhausted => Stop::StorageLimit,
            _ => Stop::InvalidProgram,
        },
    }
}
