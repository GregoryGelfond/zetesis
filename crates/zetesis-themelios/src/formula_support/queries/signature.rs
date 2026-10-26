//! A source relation's identity across the snapshots of one growing support.
//!
//! Each support round reads a new snapshot, so a value kept from an earlier
//! round cannot hold a borrow of its relation. It keeps the relation's
//! signature, its name, arity and classical sign, and a later snapshot's
//! query recognizes the relation by comparing that signature.

use zetesis_core::Sign;
use zetesis_core::catalog::PredicateRef;

use super::expression_keys::{buffer, tick};
use crate::FormulaFailure;
use crate::formula_support::{Buffer, Computation, Context};

/// A relation's name, arity and classical sign, in leased storage.
pub(super) struct Signature {
    name: Buffer<u8>,
    arity: usize,
    sign: Sign,
}

impl Signature {
    /// Test census of the leased name payload; the holder counts the header.
    #[cfg(test)]
    pub(super) fn name_bytes(&self) -> usize {
        self.name.capacity()
    }

    /// Whether `predicate` is this relation. Charges the name's length.
    pub(super) fn names(
        &self,
        predicate: PredicateRef<'_>,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        tick(context)?;
        if self.arity != predicate.arity() || self.sign != predicate.sign() {
            return Ok(false);
        }
        context.work.counters.charge_work(
            self.name.len() as u128,
            context.work.limits,
            context.work.location,
        )?;
        Ok(self.name.slice() == predicate.name().as_bytes())
    }
}

/// Record `predicate`'s signature. The buffer is embedded: the cell holding
/// the signature counts its header.
pub(super) fn signature(
    predicate: PredicateRef<'_>,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<Signature, FormulaFailure> {
    let text = predicate.name().as_bytes();
    let mut name = buffer(context)?;
    name.reserve(
        text.len(),
        context.computation,
        context.work.limits,
        context.work.counters,
        context.work.location,
    )?;
    for &byte in text {
        name.push(
            byte,
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
    }
    Ok(Signature {
        name: name.embedded(context.work.location)?,
        arity: predicate.arity(),
        sign: predicate.sign(),
    })
}
