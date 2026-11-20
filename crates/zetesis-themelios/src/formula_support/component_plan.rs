//! Immutable component ordering over one completed support and fixed-slot shape.
//!
//! The enclosing factor plan owns its copied literals. This separate owner borrows
//! those literals, so neither owner refers into its own allocation. Each slot is
//! prepared on its first traversal; later cursors borrow that immutable metadata.
//! Bindings, relation rows and cursor state are never retained here.

#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
mod tests;

use super::{
    Binding, Buffer, Computation, Context, FormulaFailure, GroundingWork, Join, JoinPlan,
    LiteralIr, StorageLease, Support, order,
};
use crate::ProgramSite;
use crate::expansion::Budget;

/// The caller's immutable component scope. `used` is sorted and duplicate-free.
#[derive(Clone, Copy)]
pub(crate) struct ComponentScope<'a> {
    pub(crate) literals: &'a [LiteralIr],
    pub(crate) used: &'a [usize],
}

struct Prepared<'a> {
    scope: ComponentScope<'a>,
    plan: order::Plan<'a>,
    // The enclosing slot array accounts the inline header, including this lease.
    // This receipt accounts only the plan's dynamically allocated vectors.
    _lease: StorageLease,
}

pub(crate) struct ComponentPlans<'a, 'source> {
    slots: Buffer<Option<Prepared<'a>>>,
    bound: Buffer<bool>,
    fixed_len: usize,
    support: &'a Support<'source>,
    lease: StorageLease,
}

impl<'a, 'source: 'a> ComponentPlans<'a, 'source> {
    /// Reserve optional plan slots after a fixed head has passed admission.
    /// No component is planned until its existence or emission cursor is needed.
    pub(crate) fn new(
        count: usize,
        variables: usize,
        fixed: &Binding<'_>,
        support: &'a Support<'source>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let mut lease = computation.lease();
        let header = size_of::<Self>()
            - size_of::<Buffer<Option<Prepared<'_>>>>()
            - size_of::<Buffer<bool>>();
        lease.observe(header, location)?;
        computation.storage_observed(&lease, 0, header, limits, counters, location)?;
        let mut slots = Buffer::new(computation, limits, counters, location)?;
        slots.reserve(count, computation, limits, counters, location)?;
        for _ in 0..count {
            slots.push(None, computation, limits, counters, location)?;
        }
        let mut bound = Buffer::new(computation, limits, counters, location)?;
        bound.reserve(variables, computation, limits, counters, location)?;
        for slot in 0..variables {
            bound.push(
                slot < fixed.len() && fixed.is_bound(slot, location)?,
                computation,
                limits,
                counters,
                location,
            )?;
        }
        Ok(Self {
            slots,
            bound,
            fixed_len: fixed.len(),
            support,
            lease,
        })
    }

    /// Borrow one exact component plan. Values may change; owner, component
    /// identity and the complete fixed-slot shape may not. Shape checks share
    /// the existing fixed-binding installation scan, before a cursor can run.
    pub(crate) fn join<'cursor>(
        &'cursor mut self,
        index: usize,
        scope: ComponentScope<'a>,
        fixed: &Binding<'_>,
        support: &'a Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Join<'cursor, 'source>, FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        counters.work(limits, location)?;
        if !std::ptr::eq(support, self.support)
            || fixed.len() != self.fixed_len
            || index >= self.slots.len()
        {
            return Err(owner(location));
        }
        computation.storage_observed(
            &self.lease,
            self.lease.bytes(),
            0,
            limits,
            counters,
            location,
        )?;
        if self.slots.slice()[index].is_none() {
            let prepared = Prepared::new(
                scope,
                self.bound.slice(),
                self.support,
                budget,
                Context::new(computation, limits, counters, location),
            )?;
            self.slots.slice_mut()[index] = Some(prepared);
        }
        self.cursor(
            index,
            scope,
            fixed,
            Context::new(computation, limits, counters, location),
        )
    }

    /// Plan component `index` if it has no plan yet, as [`Self::join`] would
    /// on its first traversal; a prepared component costs nothing more.
    pub(crate) fn prepare(
        &mut self,
        index: usize,
        scope: ComponentScope<'a>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        if index >= self.slots.len() {
            return Err(owner(location));
        }
        if self.slots.slice()[index].is_none() {
            let prepared = Prepared::new(
                scope,
                self.bound.slice(),
                self.support,
                budget,
                Context::new(computation, limits, counters, location),
            )?;
            self.slots.slice_mut()[index] = Some(prepared);
        }
        Ok(())
    }

    /// Whether component `index` already has its plan.
    pub(crate) fn is_prepared(&self, index: usize) -> bool {
        self.slots.slice().get(index).is_some_and(Option::is_some)
    }

    /// [`Self::join`] for a component whose plan is already prepared, so
    /// several cursors may borrow this owner at once. The same entry checks
    /// and charges apply; an unprepared component is refused as foreign.
    pub(crate) fn join_prepared<'cursor>(
        &'cursor self,
        index: usize,
        scope: ComponentScope<'a>,
        fixed: &Binding<'_>,
        support: &'a Support<'source>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Join<'cursor, 'source>, FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        counters.work(limits, location)?;
        if !std::ptr::eq(support, self.support)
            || fixed.len() != self.fixed_len
            || !self.is_prepared(index)
        {
            return Err(owner(location));
        }
        computation.storage_observed(
            &self.lease,
            self.lease.bytes(),
            0,
            limits,
            counters,
            location,
        )?;
        self.cursor(
            index,
            scope,
            fixed,
            Context::new(computation, limits, counters, location),
        )
    }

    fn cursor<'cursor>(
        &'cursor self,
        index: usize,
        scope: ComponentScope<'a>,
        fixed: &Binding<'_>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Join<'cursor, 'source>, FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let prepared = self.slots.slice()[index]
            .as_ref()
            .expect("prepared component");
        if !std::ptr::eq(scope.literals, prepared.scope.literals)
            || !std::ptr::eq(scope.used, prepared.scope.used)
        {
            return Err(owner(location));
        }
        let mut join = Join::with_plan(
            scope.literals,
            &Binding::new(computation, limits, counters, location)?,
            self.bound.len(),
            self.support,
            JoinPlan::Borrowed(&prepared.plan),
            None,
            Context::new(computation, limits, counters, location),
        )?;
        join.fix_component(
            fixed,
            scope.used,
            Some(self.bound.slice()),
            Context::new(computation, limits, counters, location),
        )?;
        #[cfg(test)]
        testing::reused();
        Ok(join)
    }
}

impl<'a> Prepared<'a> {
    fn new<'source: 'a>(
        scope: ComponentScope<'a>,
        bound: &[bool],
        support: &Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let empty = Binding::new(computation, limits, counters, location)?;
        let mut plan = order::Plan::with_fixed(
            scope.literals,
            (&empty, bound),
            bound.len(),
            order::SourceRows {
                relations: support,
                pivot: None,
            },
            budget,
            location,
            Some(&mut |capacity| {
                computation.preparation_capacity(capacity, limits, counters, location)
            }),
        )?;
        // Fixed head slots are bound before the first row is read, so they
        // order the component as well as its comparison schedule.
        let mut lease = computation.lease();
        let bytes = usize::try_from(plan.retained_bytes()).map_err(|_| overflow(location))?;
        lease.observe(bytes, location)?;
        computation.storage_observed(&lease, 0, 0, limits, counters, location)?;
        let decisions = order::Decisions::checked(
            scope.literals,
            &plan.patterns,
            bound,
            budget,
            location,
            &mut |capacity| computation.preparation_capacity(capacity, limits, counters, location),
        )?;
        plan.decisions = decisions;
        lease.observe(
            usize::try_from(plan.retained_bytes()).map_err(|_| overflow(location))?,
            location,
        )?;
        computation.storage_observed(&lease, lease.bytes(), 0, limits, counters, location)?;
        #[cfg(test)]
        testing::prepared();
        Ok(Self {
            scope,
            plan,
            _lease: lease,
        })
    }
}

fn owner(location: ProgramSite) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Owner,
        location,
    }
}

fn overflow(location: ProgramSite) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Overflow,
        location,
    }
}
