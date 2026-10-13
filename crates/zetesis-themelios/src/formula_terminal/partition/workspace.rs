//! Borrowed component checks and leased metadata for one partition attempt.

use crate::ProgramSite;
use zetesis_core::{PatternRef, TemplateComponentsRef};

use crate::formula_support::{
    Counters, GroundingWork, StorageLease,
    components::{Admission, Pattern},
    reserve_exact_scoped,
};
use crate::{FormulaFailure, FormulaLimits};

pub(super) struct Context<'a, 'source> {
    pub admission: &'a Admission<'source>,
    pub components: TemplateComponentsRef<'a>,
    pub limits: &'a FormulaLimits,
    pub counters: &'a mut Counters,
    pub location: ProgramSite,
}

impl<'a> Context<'a, '_> {
    pub(super) fn work(&mut self) -> Result<(), FormulaFailure> {
        self.counters.work(self.limits, self.location)
    }

    /// Charge `amount` steps of one bulk operation before performing it.
    pub(super) fn charge(&mut self, amount: u128) -> Result<(), FormulaFailure> {
        self.counters
            .charge_work(amount, self.limits, self.location)
    }

    pub(super) fn pattern(&mut self, pattern: Pattern) -> Result<PatternRef<'a>, FormulaFailure> {
        pattern.get(self.components, self.limits, self.counters, self.location)
    }

    pub(super) fn text(&mut self, left: &str, right: &str) -> Result<bool, FormulaFailure> {
        let mut left = left.bytes();
        let mut right = right.bytes();
        loop {
            self.work()?;
            match (left.next(), right.next()) {
                (None, None) => return Ok(true),
                (Some(a), Some(b)) if a == b => {}
                _ => return Ok(false),
            }
        }
    }

    pub(super) fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        lease: &mut StorageLease,
        header: usize,
    ) -> Result<(), FormulaFailure> {
        let external = self.admission.external_bytes(lease, self.location)?;
        reserve_exact_scoped(
            values,
            additional,
            lease,
            header,
            external as u128,
            GroundingWork::new(self.limits, self.counters, self.location),
        )
    }
}

pub(super) struct Scratch<T> {
    pub values: Vec<T>,
    lease: StorageLease,
}

impl<T> Scratch<T> {
    pub(super) fn new(context: &Context<'_, '_>) -> Result<Self, FormulaFailure> {
        let mut lease = context.admission.lease();
        lease.observe(size_of::<Self>(), context.location)?;
        context.admission.storage_observed(
            &lease,
            0,
            size_of::<Self>(),
            context.limits,
            context.counters,
            context.location,
        )?;
        Ok(Self {
            values: Vec::new(),
            lease,
        })
    }

    pub(super) fn reserve(
        &mut self,
        additional: usize,
        context: &mut Context<'_, '_>,
    ) -> Result<(), FormulaFailure> {
        context.reserve(
            &mut self.values,
            additional,
            &mut self.lease,
            size_of::<Self>(),
        )
    }

    pub(super) fn into_values(self) -> Vec<T> {
        self.values
    }

    pub(super) fn retain_existing(
        &mut self,
        bytes: usize,
        context: &Context<'_, '_>,
    ) -> Result<(), FormulaFailure> {
        // This is a change of ownership for buffers that are already live,
        // not an allocating replacement or a proposed allocation envelope.
        self.lease.observe(bytes, context.location)?;
        context.admission.storage_observed(
            &self.lease,
            0,
            0,
            context.limits,
            context.counters,
            context.location,
        )
    }

    pub(super) fn into_leased_values(self) -> (Vec<T>, StorageLease) {
        (self.values, self.lease)
    }

    pub(super) fn push(
        &mut self,
        value: T,
        context: &mut Context<'_, '_>,
    ) -> Result<(), FormulaFailure> {
        // Spare capacity is already leased; only growth needs a reservation.
        if self.values.len() == self.values.capacity() {
            context.reserve(&mut self.values, 1, &mut self.lease, size_of::<Self>())?;
        }
        context.work()?;
        self.values.push(value);
        Ok(())
    }
}
