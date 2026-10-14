//! Borrowed component checks and leased metadata for one partition attempt.

use crate::ProgramSite;
use zetesis_core::{PatternRef, TemplateComponentsRef};

use crate::formula_facts::LeasedReserve;
use crate::formula_support::{
    Counters, GroundingWork, StorageLease,
    components::{Admission, Pattern},
    reserve_exact_scoped,
};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

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

    /// A fact block's predicate.
    pub(super) fn predicate(
        &mut self,
        predicate: crate::formula_support::components::Predicate,
    ) -> Result<zetesis_core::catalog::PredicateRef<'a>, FormulaFailure> {
        predicate.get(self.components, self.limits, self.counters, self.location)
    }

    /// A fact cell's value.
    pub(super) fn scalar(
        &mut self,
        scalar: crate::formula_support::components::Scalar,
    ) -> Result<zetesis_core::catalog::TermRef<'a>, FormulaFailure> {
        scalar.get(self.components, self.limits, self.counters, self.location)
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

/// Exact reservations under the partition's leases, each admitted before it is
/// allocated.
impl LeasedReserve for Context<'_, '_> {
    fn reserve_leased<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        lease: &mut StorageLease,
    ) -> Result<(), FormulaFailure> {
        let others = lease
            .bytes()
            .checked_sub(values.capacity() * size_of::<T>())
            .expect("the lease holds each vector it reserves");
        self.reserve(values, additional, lease, others)
    }
}

/// A lease on storage the partition admits without holding it in a buffer of
/// its own: the vectors a split reserves, and the live buffers whose owner a
/// move changes.
pub(super) struct Lease(StorageLease);

impl Lease {
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
        Ok(Self(lease))
    }

    /// The lease, for reservations made under it.
    pub(super) fn get_mut(&mut self) -> &mut StorageLease {
        &mut self.0
    }

    /// Add `bytes` of buffers that are already live and whose owner the move
    /// changes. Nothing is allocated: the receipt grows by what changes owner,
    /// and the workspace's total must still fit.
    pub(super) fn retain_existing(
        &mut self,
        bytes: usize,
        context: &Context<'_, '_>,
    ) -> Result<(), FormulaFailure> {
        let total = self
            .0
            .bytes()
            .checked_add(bytes)
            .ok_or(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                observed: self.0.bytes() as u128 + bytes as u128,
                limit: context.limits.max_support_bytes as u128,
                location: context.location,
            })?;
        self.0.observe(total, context.location)?;
        context.admission.storage_observed(
            &self.0,
            0,
            0,
            context.limits,
            context.counters,
            context.location,
        )
    }

    pub(super) fn into_lease(self) -> StorageLease {
        self.0
    }
}
