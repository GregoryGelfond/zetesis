//! Reusable expression metadata charged to one source workspace lease.

use crate::ProgramSite;
use crate::formula_support::Context as GroundingContext;
use zetesis_core::catalog::{AssignmentError, AssignmentFailure, Error, TermAssignment, TermKey};

use super::{Context, Evaluation, Input};
use crate::formula_support::{Computation, Counters, StorageLease, storage};
use crate::{FormulaFailure, FormulaLimits};

const RETAINED_CELLS: usize = 32;

#[derive(Default)]
pub(super) struct Scratch {
    terms: Option<TermAssignment>,
    pub(super) integers: Vec<i32>,
    pub(super) missing: Vec<bool>,
    lease: Option<StorageLease>,
}

impl Scratch {
    fn bytes(&self) -> usize {
        size_of::<Evaluation>()
            + self.integers.capacity() * size_of::<i32>()
            + self.missing.capacity() * size_of::<bool>()
            + self.terms.as_ref().map_or(0, |terms| {
                terms.retained_bytes() - size_of::<TermAssignment>()
            })
    }

    pub(super) fn begin(
        &mut self,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        if self.terms.is_none() {
            self.terms = Some(computation.read().assignment());
        }
        if self.lease.is_none() {
            self.lease = Some(computation.lease());
        }
        let actual = self.bytes();
        let lease = self.lease.as_mut().expect("created above");
        let previous = lease.bytes();
        lease.observe(actual, location)?;
        computation.storage_observed(
            lease,
            previous,
            size_of::<Evaluation>(),
            limits,
            counters,
            location,
        )?;
        self.terms
            .as_ref()
            .expect("created above")
            .as_slice()
            .bind_with(computation.read(), || counters.work(limits, location))
            .map_err(|error| crate::formula_binding::failure(error, location))?;
        Ok(())
    }

    pub(super) fn terms(&self) -> &TermAssignment {
        self.terms.as_ref().expect("evaluation began")
    }

    pub(super) fn mask<V: Fn(usize) -> Input>(
        &mut self,
        len: usize,
        context: &mut Context<'_, '_, '_, V>,
    ) -> Result<(), FormulaFailure> {
        let other = self.bytes() - self.missing.capacity() * size_of::<bool>();
        storage::reserve(
            &mut self.missing,
            len,
            self.lease.as_mut().expect("evaluation began"),
            other,
            GroundingContext::new(
                context.computation,
                context.limits,
                context.counters,
                context.location,
            ),
        )?;
        for _ in 0..len {
            context.counters.work(context.limits, context.location)?;
        }
        self.missing.resize(len, false);
        Ok(())
    }

    pub(super) fn integer<V: Fn(usize) -> Input>(
        &mut self,
        value: i32,
        context: &mut Context<'_, '_, '_, V>,
    ) -> Result<(), FormulaFailure> {
        let other = self.bytes() - self.integers.capacity() * size_of::<i32>();
        storage::reserve(
            &mut self.integers,
            1,
            self.lease.as_mut().expect("evaluation began"),
            other,
            GroundingContext::new(
                context.computation,
                context.limits,
                context.counters,
                context.location,
            ),
        )?;
        context.counters.work(context.limits, context.location)?;
        self.integers.push(value);
        Ok(())
    }

    pub(super) fn push<V: Fn(usize) -> Input>(
        &mut self,
        key: Option<&TermKey>,
        context: &mut Context<'_, '_, '_, V>,
    ) -> Result<(), FormulaFailure> {
        let other = self.bytes() - self.terms().retained_bytes();
        let lease = self.lease.as_mut().expect("evaluation began");
        let previous = lease.bytes();
        let maximum = context
            .computation
            .allowance(lease, context.limits, context.location)?;
        let terms = self.terms.as_mut().expect("evaluation began");
        let position = terms.len();
        let end = position.checked_add(1).ok_or_else(|| {
            crate::formula_binding::assignment(
                AssignmentError::Storage(Error::Overflow),
                context.location,
            )
        })?;
        let result = terms
            .resize_with(end, maximum.saturating_sub(other), || {
                context.counters.work(context.limits, context.location)
            })
            .map_err(|error| offset(error, other));
        lease.observe(other + terms.retained_bytes(), context.location)?;
        let observed = context.computation.storage_observed(
            lease,
            previous,
            other + size_of::<TermAssignment>(),
            context.limits,
            context.counters,
            context.location,
        );
        context
            .computation
            .storage_result(result, lease, context.limits, context.location)?;
        observed?;
        if let Some(key) = key {
            terms
                .set_with(position, key, || {
                    context.counters.work(context.limits, context.location)
                })
                .map_err(|error| crate::formula_binding::failure(error, context.location))?;
        }
        Ok(())
    }

    pub(super) fn materialize<V: Fn(usize) -> Input>(
        &mut self,
        context: &mut Context<'_, '_, '_, V>,
    ) -> Result<(), FormulaFailure> {
        if !self.terms().is_empty() {
            return Ok(());
        }
        for index in 0..self.integers.len() {
            context.counters.work(context.limits, context.location)?;
            let key = if self.missing.get(index).copied().unwrap_or(false) {
                None
            } else {
                Some(context.computation.number(
                    self.integers[index],
                    context.limits,
                    context.counters,
                    context.location,
                )?)
            };
            self.push(key.as_ref(), context)?;
        }
        self.integers.clear();
        Ok(())
    }

    pub(super) fn reset(&mut self, location: ProgramSite) {
        self.integers.clear();
        self.missing.clear();
        if self.integers.capacity() > RETAINED_CELLS {
            self.integers = Vec::new();
        }
        if self.missing.capacity() > RETAINED_CELLS {
            self.missing = Vec::new();
        }
        if let Some(terms) = &mut self.terms {
            terms.reset();
            if terms.capacity() > RETAINED_CELLS {
                self.terms = None;
            }
        }
        let actual = self.bytes();
        if let Some(lease) = &mut self.lease {
            lease
                .observe(actual, location)
                .expect("releasing already charged capacity cannot overflow");
        }
    }
}

fn offset(
    error: AssignmentFailure<FormulaFailure>,
    other: usize,
) -> AssignmentFailure<FormulaFailure> {
    match error {
        AssignmentFailure::Assignment(AssignmentError::Storage(Error::Storage {
            required,
            limit,
        })) => AssignmentFailure::Assignment(AssignmentError::Storage(Error::Storage {
            required: required + other as u128,
            limit: limit + other,
        })),
        error => error,
    }
}

pub(super) struct Frame<'a> {
    pub(super) scratch: &'a mut Scratch,
    pub(super) location: ProgramSite,
}
impl Drop for Frame<'_> {
    fn drop(&mut self) {
        self.scratch.reset(self.location);
    }
}

#[cfg(test)]
mod tests;
