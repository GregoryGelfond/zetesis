//! One live, fallibly reserved numeric or coordinate buffer.
use crate::ProgramSite;
use crate::formula_support::{Computation, Counters, StorageLease, reserve};
use crate::{FormulaFailure, FormulaLimits};

use crate::formula_support::Context;
pub(crate) struct Buffer<T> {
    values: Vec<T>,
    lease: StorageLease,
}
impl<T> Buffer<T> {
    pub(crate) fn new(
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let mut lease = computation.lease();
        lease.observe(size_of::<Self>(), location)?;
        computation.storage_observed(&lease, 0, size_of::<Self>(), limits, counters, location)?;
        Ok(Self {
            values: Vec::new(),
            lease,
        })
    }
    pub(crate) fn push(
        &mut self,
        value: T,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        self.reserve(1, computation, limits, counters, location)?;
        counters.work(limits, location)?;
        self.values.push(value);
        Ok(())
    }
    /// Reserve a known additional population without publishing placeholder
    /// elements. The ordinary lease records capacity even on refusal.
    pub(crate) fn reserve(
        &mut self,
        additional: usize,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        reserve(
            &mut self.values,
            additional,
            &mut self.lease,
            size_of::<Self>(),
            Context::new(computation, limits, counters, location),
        )
    }
    pub(crate) fn clear(&mut self) {
        self.values.clear();
    }
    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }
    pub(crate) fn slice(&self) -> &[T] {
        &self.values
    }
    pub(crate) fn slice_mut(&mut self) -> &mut [T] {
        &mut self.values
    }
    pub(crate) fn iter(&self) -> std::slice::Iter<'_, T> {
        self.values.iter()
    }
}

impl<T: Copy> Buffer<T> {
    /// Admit one replacement capacity and all added metadata cells before
    /// initialization. Capacity remains charged if the caller stops afterward.
    pub(crate) fn resize(
        &mut self,
        len: usize,
        value: T,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let additional = len.saturating_sub(self.values.len());
        reserve(
            &mut self.values,
            additional,
            &mut self.lease,
            size_of::<Self>(),
            Context::new(computation, limits, counters, location),
        )?;
        counters.charge_work(additional as u128 + 1, limits, location)?;
        self.values.resize(len, value);
        Ok(())
    }
}
